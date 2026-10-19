//! RTU私有通道与独占串口透传；完整实际帧和清理范围分开记录。
use super::{publish, Shared};
use essim_core::{
    application::events::master::MasterFact,
    domain::endpoint::{EndpointConfig, Parity, Transport},
};
use libmodbus_rs::{rtu_crc, ModbusClient, RtuConfig, SerialPort};
use std::{
    io::{self, Read, Write},
    os::unix::net::UnixStream,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
pub(super) struct RtuConnection {
    pub client: ModbusClient,
    stop: Arc<AtomicBool>,
    recovery: mpsc::SyncSender<mpsc::SyncSender<io::Result<()>>>,
    join: Option<JoinHandle<()>>,
    timeout: Duration,
}
fn config(e: &EndpointConfig) -> io::Result<RtuConfig> {
    let Transport::Rtu {
        path,
        baud,
        parity,
        data_bits,
        stop_bits,
    } = &e.transport
    else {
        return Err(io::ErrorKind::InvalidInput.into());
    };
    RtuConfig::new(
        path,
        *baud,
        match parity {
            Parity::None => 'N',
            Parity::Even => 'E',
            Parity::Odd => 'O',
        },
        *data_bits,
        *stop_bits,
        1,
        50,
        2,
    )
}
fn length(bytes: &[u8], tx: bool) -> Option<usize> {
    if bytes.len() < 2 {
        return None;
    }
    let fc = bytes[1];
    if fc >= 128 {
        return Some(5);
    }
    if tx {
        match fc {
            1..=6 => Some(8),
            8 => Some(8),
            7 | 11 | 12 | 17 => Some(4),
            15 | 16 if bytes.len() >= 7 => Some(9 + bytes[6] as usize),
            _ => None,
        }
    } else {
        match fc {
            5 | 6 | 8 | 11 | 15 | 16 => Some(8),
            7 => Some(5),
            1..=4 | 12 | 17 if bytes.len() >= 3 => Some(5 + bytes[2] as usize),
            _ => None,
        }
    }
}
fn record(shared: &Shared, bytes: &[u8], tx: bool, complete: bool, discarded: bool) {
    publish(
        shared,
        MasterFact::RtuFrame {
            direction: if tx { "TX" } else { "RX" }.into(),
            hex: bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
            complete,
            crc_valid: complete && rtu_crc(bytes) == 0,
            discarded,
        },
    );
}
impl RtuConnection {
    pub fn open(endpoint: &EndpointConfig, shared: Shared) -> io::Result<Self> {
        let cfg = config(endpoint)?;
        let cfg_for_wire = cfg.clone();
        let Transport::Rtu {
            path,
            baud,
            parity,
            stop_bits,
            ..
        } = &endpoint.transport
        else {
            unreachable!()
        };
        let path = path.clone();
        let bits = 1 + 8 + u64::from(*parity != Parity::None) + u64::from(*stop_bits);
        let gap = if *baud > 19200 {
            Duration::from_micros(1750)
        } else {
            Duration::from_micros((bits * 3_500_000).div_ceil(u64::from(*baud)))
        };
        let timeout = Duration::from_millis(endpoint.master.request_timeout_ms);
        let (inside, mut local) = UnixStream::pair()?;
        local.set_nonblocking(true)?;
        let client = ModbusClient::rtu_channel(&cfg, inside, timeout)?;
        let stop = Arc::new(AtomicBool::new(false));
        let done = stop.clone();
        let (recovery, rx) = mpsc::sync_channel::<mpsc::SyncSender<io::Result<()>>>(1);
        let (ready, started) = mpsc::sync_channel(1);
        let join = std::thread::Builder::new()
            .name("master-serial".into())
            .spawn(move || {
                let mut port = match SerialPort::open(&path, &cfg_for_wire) {
                    Ok(p) => {
                        let _ = ready.send(Ok(()));
                        p
                    }
                    Err(e) => {
                        let _ = ready.send(Err(e));
                        return;
                    }
                };
                let mut tx = Vec::new();
                let mut incoming = Vec::new();
                let mut out = Vec::new();
                let mut returned = Vec::new();
                let mut buffer = [0; 256];
                let mut last_rx = Instant::now();
                let mut last_tx = Instant::now() - gap;
                let mut awaiting = false;
                let mut recovery: Option<(Instant, Instant, mpsc::SyncSender<io::Result<()>>)> =
                    None;
                let result = (|| -> io::Result<()> {
                    while !done.load(Ordering::Acquire) {
                        if let Ok(reply) = rx.try_recv() {
                            awaiting = false;
                            returned.clear();
                            if !incoming.is_empty() {
                                record(&shared, &incoming, false, false, true);
                                incoming.clear();
                            }
                            recovery = Some((
                                Instant::now() + timeout,
                                Instant::now() + timeout * 2 + Duration::from_millis(100),
                                reply,
                            ));
                        }
                        match local.read(&mut buffer) {
                            Ok(0) => break,
                            Ok(n) => {
                                tx.extend_from_slice(&buffer[..n]);
                                if tx.len() > 256 {
                                    return Err(io::Error::other("RTU请求超过256字节"));
                                }
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(e) => return Err(e),
                        }
                        if let Some(n) = length(&tx, true) {
                            if n > 256 {
                                return Err(io::Error::other("RTU请求长度非法"));
                            }
                            if tx.len() >= n
                                && recovery.is_none()
                                && out.is_empty()
                                && last_tx.elapsed() >= gap
                            {
                                let bytes: Vec<_> = tx.drain(..n).collect();
                                record(&shared, &bytes, true, true, false);
                                out.extend(bytes);
                                awaiting = true;
                            }
                        }
                        match port.read(&mut buffer) {
                            Ok(0) => {}
                            Ok(n) => {
                                last_rx = Instant::now();
                                incoming.extend_from_slice(&buffer[..n]);
                                if incoming.len() > 512 {
                                    return Err(io::Error::other("RTU接收缓冲超过512字节"));
                                }
                            }
                            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                            Err(e) => return Err(e),
                        }
                        while let Some(n) = length(&incoming, false) {
                            if n > 256 {
                                break;
                            }
                            if incoming.len() < n {
                                break;
                            }
                            let bytes: Vec<_> = incoming.drain(..n).collect();
                            let discard = recovery.is_some() || !awaiting;
                            record(&shared, &bytes, false, true, discard);
                            if !discard {
                                returned.extend(bytes);
                                awaiting = false;
                            }
                        }
                        if !incoming.is_empty()
                            && last_rx.elapsed() >= gap + Duration::from_millis(3)
                        {
                            let discarded = recovery.is_some() || !awaiting;
                            record(&shared, &incoming, false, false, discarded);
                            if !discarded {
                                returned.extend(&incoming);
                            }
                            incoming.clear();
                        }
                        if let Some((minimum, maximum, _)) = &recovery {
                            if Instant::now() >= *minimum && last_rx.elapsed() >= gap {
                                if let Some((_, _, reply)) = recovery.take() {
                                    let _ = reply.send(Ok(()));
                                }
                            } else if Instant::now() >= *maximum {
                                if let Some((_, _, reply)) = recovery.take() {
                                    let _ = reply.send(Err(io::Error::other(
                                        "串口持续接收，无法确认安静边界",
                                    )));
                                }
                                return Err(io::Error::other("RTU恢复超时"));
                            }
                        }
                        if shared.lock().map(|p| p.record_failed).unwrap_or(true) {
                            return Err(io::Error::other("可靠记录失败，停止串行通信"));
                        }
                        if !out.is_empty() {
                            match port.write(&out) {
                                Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
                                Ok(n) => {
                                    out.drain(..n);
                                    if out.is_empty() {
                                        last_tx = Instant::now();
                                    }
                                }
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                                Err(e) => return Err(e),
                            }
                        }
                        if !returned.is_empty() {
                            match local.write(&returned) {
                                Ok(0) => break,
                                Ok(n) => {
                                    returned.drain(..n);
                                }
                                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                                Err(e) => return Err(e),
                            }
                        }
                        std::thread::sleep(Duration::from_millis(1));
                    }
                    Ok(())
                })();
                if !incoming.is_empty() {
                    record(&shared, &incoming, false, false, true);
                }
                if let Err(e) = result {
                    publish(
                        &shared,
                        MasterFact::Failure {
                            message: format!("串口通道关闭：{e}"),
                        },
                    );
                }
            })?;
        match started.recv_timeout(Duration::from_secs(3)) {
            Ok(Ok(())) => Ok(Self {
                client,
                stop,
                recovery,
                join: Some(join),
                timeout,
            }),
            result => {
                stop.store(true, Ordering::Release);
                let _ = join.join();
                match result {
                    Ok(Err(e)) => Err(e),
                    _ => Err(io::Error::new(io::ErrorKind::TimedOut, "串口打开未确认")),
                }
            }
        }
    }
    pub fn recover(&mut self, cancel: &AtomicBool, shutdown: &AtomicBool) -> io::Result<()> {
        let (tx, rx) = mpsc::sync_channel(1);
        self.recovery.try_send(tx).map_err(io::Error::other)?;
        let deadline = Instant::now() + self.timeout * 2 + Duration::from_millis(200);
        loop {
            if cancel.load(Ordering::Acquire) || shutdown.load(Ordering::Acquire) {
                return Err(io::ErrorKind::Interrupted.into());
            }
            match rx.recv_timeout(Duration::from_millis(5)) {
                Ok(result) => {
                    result?;
                    break;
                }
                Err(mpsc::RecvTimeoutError::Timeout) if Instant::now() < deadline => {}
                Err(e) => return Err(io::Error::other(e)),
            }
        }
        self.client.drain_rtu_channel()
    }
}
impl Drop for RtuConnection {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lengths_cover_diagnostics_and_basic_fragments() {
        assert_eq!(length(&[1, 8], false), Some(8));
        assert_eq!(length(&[1, 12, 6], false), Some(11));
        assert_eq!(length(&[1, 0x83], false), Some(5));
        assert_eq!(length(&[1, 15, 0, 0, 0, 4, 1], true), Some(10));
        assert_eq!(length(&[1], false), None);
    }
}
