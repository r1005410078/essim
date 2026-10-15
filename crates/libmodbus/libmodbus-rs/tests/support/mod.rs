#![allow(dead_code)]
use std::{
    ffi::CStr,
    fs::File,
    io::{Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    time::Duration,
};
/// 仅测试使用的 PTY；保留 slave guard，避免两端初始化前 master 返回 EIO。
pub struct Pty {
    pub wire: File,
    pub path: String,
    _slave: File,
}
impl Pty {
    pub fn new() -> Self {
        let (mut master, mut slave) = (-1, -1);
        let mut name = [0i8; 256];
        // SAFETY: 输出指针有效，名称缓冲足够容纳系统 PTY 路径；不传入 termios/winsize。
        assert_eq!(
            unsafe {
                libc::openpty(
                    &mut master,
                    &mut slave,
                    name.as_mut_ptr(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                )
            },
            0
        );
        // SAFETY: openpty 成功创建且移交两个唯一 fd；名称由系统写入并以 NUL 终止。
        unsafe {
            Self {
                wire: File::from_raw_fd(master),
                path: CStr::from_ptr(name.as_ptr()).to_str().unwrap().into(),
                _slave: File::from_raw_fd(slave),
            }
        }
    }
    pub fn send(&mut self, body: &[u8]) {
        self.wire.write_all(&adu(body)).unwrap();
    }
    pub fn read(&mut self, n: usize) -> Vec<u8> {
        let mut bytes = vec![0; n];
        for byte in &mut bytes {
            assert!(self.ready(Duration::from_secs(1)), "串口响应超时");
            self.wire.read_exact(std::slice::from_mut(byte)).unwrap();
        }
        bytes
    }
    pub fn ready(&self, timeout: Duration) -> bool {
        let mut fd = libc::pollfd {
            fd: self.wire.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: pollfd 有效，仅借用存活 fd；测试超时在 i32 范围内。
        unsafe {
            libc::poll(&mut fd, 1, timeout.as_millis() as i32) > 0 && fd.revents & libc::POLLIN != 0
        }
    }
}
/// 仅测试构造报文；产品 CRC 由 libmodbus 实现。
pub fn adu(body: &[u8]) -> Vec<u8> {
    let mut crc = 0xffffu16;
    for byte in body {
        crc ^= *byte as u16;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xa001
            } else {
                crc >> 1
            };
        }
    }
    let mut bytes = body.to_vec();
    bytes.extend(crc.to_le_bytes());
    bytes
}
