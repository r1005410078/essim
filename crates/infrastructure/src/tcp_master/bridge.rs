//! 私有通道与真实TCP连接之间透传；仅做有界MBAP观测，编码／确认校验由libmodbus负责。
use super::{publish, Shared};
use essim_core::application::events::master::MasterFact;
use libmodbus_rs::ModbusClient;
use std::{
    io::{self, Read, Write},
    net::{Shutdown, SocketAddr, TcpStream},
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::Duration,
};
#[derive(Default)]
struct Frames {
    bytes: Vec<u8>,
}
impl Frames {
    fn feed(&mut self, chunk: &[u8]) -> io::Result<Vec<Vec<u8>>> {
        self.bytes.extend_from_slice(chunk);
        let mut frames = vec![];
        while self.bytes.len() >= 6 {
            let len = usize::from(u16::from_be_bytes([self.bytes[4], self.bytes[5]])) + 6;
            if !(8..=260).contains(&len) || self.bytes[2..4] != [0, 0] {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "非法MBAP长度／协议标识",
                ));
            }
            if self.bytes.len() < len {
                break;
            }
            frames.push(self.bytes.drain(..len).collect());
        }
        if self.bytes.len() > 260 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "未完成报文超过260字节",
            ));
        }
        Ok(frames)
    }
}
pub(super) struct Connection {
    pub client: ModbusClient,
    stop: Arc<AtomicBool>,
    interrupt: TcpStream,
    join: Option<JoinHandle<()>>,
}
impl Connection {
    pub fn open(
        address: SocketAddr,
        settings: &essim_core::domain::master::MasterSettings,
        shared: Shared,
    ) -> io::Result<Self> {
        let remote = TcpStream::connect_timeout(
            &address,
            Duration::from_millis(settings.connect_timeout_ms),
        )?;
        remote.set_nodelay(true)?;
        remote.set_nonblocking(true)?;
        let interrupt = remote.try_clone()?;
        let (inside, mut local) = UnixStream::pair()?;
        local.set_nonblocking(true)?;
        let client = ModbusClient::tcp_channel(
            inside,
            1,
            Duration::from_millis(settings.request_timeout_ms),
        )?;
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let join = std::thread::Builder::new()
            .name("master-wire".into())
            .spawn(move || {
                let mut remote = remote;
                let mut tx = Frames::default();
                let mut rx = Frames::default();
                let mut to_remote: Vec<u8> = vec![];
                let mut to_local: Vec<u8> = vec![];
                let mut buffer = [0; 512];
                let result = (|| -> io::Result<()> {
                    while !done.load(Ordering::Acquire) {
                        for outgoing in [true, false] {
                            let result = if outgoing {
                                local.read(&mut buffer)
                            } else {
                                remote.read(&mut buffer)
                            };
                            match result {
                                Ok(0) => return Ok(()),
                                Ok(n) => {
                                    // 单在途协议保证流量有界；超限关闭，绝不静默丢弃TCP字节。
                                    let queue = if outgoing {
                                        &mut to_remote
                                    } else {
                                        &mut to_local
                                    };
                                    if queue.len() + n > 4096 {
                                        return Err(io::Error::other("透传队列超过4096字节"));
                                    }
                                    queue.extend_from_slice(&buffer[..n]);
                                    for frame in if outgoing {
                                        tx.feed(&buffer[..n])?
                                    } else {
                                        rx.feed(&buffer[..n])?
                                    } {
                                        publish(
                                            &shared,
                                            MasterFact::Frame {
                                                direction: if outgoing { "TX" } else { "RX" }
                                                    .into(),
                                                hex: frame
                                                    .iter()
                                                    .map(|b| format!("{b:02X}"))
                                                    .collect::<Vec<_>>()
                                                    .join(" "),
                                            },
                                        );
                                    }
                                }
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                                Err(e) => return Err(e),
                            }
                        }
                        if shared.lock().map(|p| p.record_failed).unwrap_or(true) {
                            return Err(io::Error::other("可靠记录失败，停止透传"));
                        }
                        for outgoing in [true, false] {
                            let queue = if outgoing {
                                &mut to_remote
                            } else {
                                &mut to_local
                            };
                            if !queue.is_empty() {
                                let result = if outgoing {
                                    remote.write(queue)
                                } else {
                                    local.write(queue)
                                };
                                match result {
                                    Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                                    Ok(n) => {
                                        queue.drain(..n);
                                    }
                                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                                    Err(e) => return Err(e),
                                }
                            }
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Ok(())
                })();
                if let Err(e) = result {
                    publish(
                        &shared,
                        MasterFact::Failure {
                            message: format!("传输关闭：{e}"),
                        },
                    );
                }
                let _ = remote.shutdown(Shutdown::Both);
                let _ = local.shutdown(Shutdown::Both);
            })?;
        Ok(Self {
            client,
            stop,
            interrupt,
            join: Some(join),
        })
    }
}
impl Drop for Connection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.interrupt.shutdown(Shutdown::Both);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn split_join_and_invalid_frames() {
        let a = [0, 1, 0, 0, 0, 6, 1, 3, 0, 0, 0, 1];
        let mut f = Frames::default();
        assert!(f.feed(&a[..4]).unwrap().is_empty());
        let mut tail = a[4..].to_vec();
        tail.extend(a);
        let out = f.feed(&tail).unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0], a);
        assert!(f.feed(&[0, 0, 0, 0, 255, 255]).is_err());
    }
}
