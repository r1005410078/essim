//! RTU 串口从站安全封装；仅支持当前明确的 Unix 参数子集。
use super::{checked, invalid, library_version, sys, Exception, Mapping};
use std::{ffi::CString, io, marker::PhantomData, ptr::NonNull, rc::Rc};
/// RTU ADU 最大长度，包含站号和 CRC，不同于 TCP 的 260。
pub const RTU_MAX_ADU_LENGTH: usize = sys::RTU_MAX_ADU_LENGTH;

/// 已校验的串口配置；首版支持 9600–115200 的五种常用速率、8 数据位、N/E/O、1/2 停止位。
///
/// 接收等待限制 10–100ms，字节间等待 1–2ms，限制最大 256 字节请求造成的 worker 阻塞。
/// 参数被接受不表示硬件已验证；PTY 不验证校验位、电气或帧间时序。
///
/// # Examples
/// ```
/// use libmodbus_rs::RtuConfig;
/// // 仅校验教学配置；不会打开示例路径。
/// let config = RtuConfig::new("/dev/serial-example", 19200, 'N', 8, 2, 7, 50, 2)?;
/// # Ok::<(), std::io::Error>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtuConfig {
    device: CString,
    baud: i32,
    parity: u8,
    data_bits: u8,
    stop_bits: u8,
    slave: u8,
    receive_timeout_ms: u32,
    byte_timeout_ms: u32,
}
impl RtuConfig {
    pub(super) fn device_path(&self) -> &str {
        self.device.to_str().expect("路径来自UTF8字符串")
    }

    /// 校验设备路径、串口参数、1–247 站号及毫秒超时，不打开设备。
    ///
    /// # Errors
    /// 路径为空/含 NUL，参数不在首版支持范围内时返回 InvalidInput。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: &str,
        baud: u32,
        parity: char,
        data_bits: u8,
        stop_bits: u8,
        slave: u16,
        receive_timeout_ms: u32,
        byte_timeout_ms: u32,
    ) -> io::Result<Self> {
        if device.trim().is_empty() {
            return Err(invalid("串口路径不能为空"));
        }
        let device = CString::new(device).map_err(|_| invalid("串口路径不能含 NUL"))?;
        if ![9600, 19200, 38400, 57600, 115200].contains(&baud)
            || !['N', 'E', 'O'].contains(&parity)
            || data_bits != 8
            || ![1, 2].contains(&stop_bits)
        {
            return Err(invalid(
                "串口支持 9600/19200/38400/57600/115200、8 数据位、N/E/O 校验及 1/2 停止位",
            ));
        }
        if !(1..=247).contains(&slave)
            || !(10..=100).contains(&receive_timeout_ms)
            || !(1..=2).contains(&byte_timeout_ms)
        {
            return Err(invalid(
                "站号须为 1–247，接收超时 10–100ms，字节间超时 1–2ms",
            ));
        }
        Ok(Self {
            device,
            baud: baud as i32,
            parity: parity as u8,
            data_bits,
            stop_bits,
            slave: slave as u8,
            receive_timeout_ms,
            byte_timeout_ms,
        })
    }
}

/// 成功接收且检查结构的 RTU 请求；私有构造，不能用于 TCP 应答。
#[derive(Debug)]
pub struct RtuRequest {
    bytes: [u8; RTU_MAX_ADU_LENGTH],
    length: usize,
}
impl RtuRequest {
    /// 返回实际接收的 ADU，包含站号、PDU 和 CRC。
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
    /// 请求是否使用广播站号 0；广播的任何应答入口均保持静默。
    pub fn is_broadcast(&self) -> bool {
        self.bytes[0] == 0
    }
}

/// 独占串口的 RTU context，Drop 时先关闭串口再释放 C 对象，禁止跨线程。
///
/// 不使用 TCP 的 socket 借用路径，不开启自动重连、RTS 回调或广播响应 quirk。
/// ```compile_fail
/// use libmodbus_rs::{RtuContext, Request, Mapping};
/// fn mixed(ctx: &mut RtuContext, tcp: &Request, map: &mut Mapping) {
///     ctx.reply(tcp, map); // TCP 请求不能传给 RTU
/// }
/// ```
/// ```compile_fail
/// use libmodbus_rs::{TcpContext, RtuRequest, Mapping};
/// use std::os::fd::AsFd;
/// fn mixed<S: AsFd>(ctx: &mut TcpContext<S>, rtu: &RtuRequest, map: &mut Mapping) {
///     ctx.reply(rtu, map); // RTU 请求不能传给 TCP
/// }
/// ```
/// ```compile_fail
/// use libmodbus_rs::RtuContext;
/// fn moved(ctx: RtuContext) { std::thread::spawn(move || drop(ctx)); }
/// ```
pub struct RtuContext {
    pub(super) handle: NonNull<sys::modbus_t>,
    opened: bool,
    _thread_bound: PhantomData<Rc<()>>,
}
impl RtuContext {
    pub(super) fn channel(
        config: &RtuConfig,
        socket: &std::os::unix::net::UnixStream,
    ) -> io::Result<Self> {
        use std::os::fd::AsRawFd;
        if library_version() != (3, 2, 0) {
            return Err(invalid("运行库必须为libmodbus3.2.0"));
        }
        // SAFETY: config已校验且CString存活；返回指针检查非空，由此对象唯一释放。
        let handle = NonNull::new(unsafe {
            sys::modbus_new_rtu(
                config.device.as_ptr(),
                config.baud,
                config.parity as _,
                config.data_bits.into(),
                config.stop_bits.into(),
            )
        })
        .ok_or_else(io::Error::last_os_error)?;
        let ctx = Self {
            handle,
            opened: false,
            _thread_bound: PhantomData,
        };
        // SAFETY: 私有socket存活，C仅借用；opened=false确保Drop只free而不关闭Rust描述符。
        checked(unsafe { sys::modbus_set_socket(ctx.handle.as_ptr(), socket.as_raw_fd()) })?;
        Ok(ctx)
    }

    /// 创建、配置并打开串口，成功后串口由本对象唯一管理。
    ///
    /// # Errors
    /// 运行库版本错误、C 分配/配置失败或串口打开失败时返回错误。
    pub fn open(config: &RtuConfig) -> io::Result<Self> {
        Self::open_inner(config, 0)
    }
    fn open_inner(config: &RtuConfig, fail_stage: u8) -> io::Result<Self> {
        if library_version() != (3, 2, 0) {
            return Err(invalid("运行库必须为 libmodbus 3.2.0"));
        }
        // SAFETY: 配置已校验，CString 在调用期间存活；返回值检查非空且独占持有。
        let handle = NonNull::new(unsafe {
            sys::modbus_new_rtu(
                config.device.as_ptr(),
                config.baud,
                config.parity as _,
                config.data_bits.into(),
                config.stop_bits.into(),
            )
        })
        .ok_or_else(io::Error::last_os_error)?;
        let mut ctx = Self {
            handle,
            opened: false,
            _thread_bound: PhantomData,
        };
        if fail_stage == 1 {
            return Err(io::Error::other("注入：RTU 分配后失败"));
        }
        // SAFETY: context 存活且独占，所有参数已通过范围校验，尚无并发收发。
        unsafe {
            checked(sys::modbus_set_slave(handle.as_ptr(), config.slave.into()))?;
            checked(sys::modbus_set_indication_timeout(
                handle.as_ptr(),
                0,
                config.receive_timeout_ms * 1000,
            ))?;
            checked(sys::modbus_set_byte_timeout(
                handle.as_ptr(),
                0,
                config.byte_timeout_ms * 1000,
            ))?;
            // 非本站帧触发的 confirmation-to-ignore 同样必须有限等待。
            checked(sys::modbus_set_response_timeout(
                handle.as_ptr(),
                0,
                config.receive_timeout_ms * 1000,
            ))?;
            checked(sys::modbus_connect(handle.as_ptr()))?;
        }
        ctx.opened = true;
        if fail_stage == 2 {
            return Err(io::Error::other("注入：RTU 打开后失败"));
        }
        Ok(ctx)
    }
    /// 接收一条 RTU 请求；被上游忽略时返回 None，不等同于超时或错误。
    ///
    /// # Errors
    /// 超时、I/O 失败、CRC 或请求结构错误时返回错误；协议错误映射为 InvalidData。
    pub fn receive(&mut self) -> io::Result<Option<RtuRequest>> {
        let mut req = RtuRequest {
            bytes: [0; RTU_MAX_ADU_LENGTH],
            length: 0,
        };
        // SAFETY: 本对象独占打开的 context，缓冲区容量等于 RTU ADU 上限，调用期间可写。
        let n = unsafe { sys::modbus_receive(self.handle.as_ptr(), req.bytes.as_mut_ptr()) };
        if n < 0 {
            let e = io::Error::last_os_error();
            return Err(
                if e.raw_os_error().is_some_and(|v| v >= sys::MODBUS_ENOBASE) {
                    io::Error::new(io::ErrorKind::InvalidData, e)
                } else {
                    e
                },
            );
        }
        if n == 0 {
            return Ok(None);
        }
        let n = n as usize;
        if !(4..=RTU_MAX_ADU_LENGTH).contains(&n) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "RTU 长度异常"));
        }
        let expected = match req.bytes[1] {
            0..=6 => 8,
            15 | 16 if n >= 9 => 9 + req.bytes[6] as usize,
            22 => 10,
            23 if n >= 13 => 13 + req.bytes[10] as usize,
            15 | 16 | 23 => 0,
            _ => 4,
        };
        if n != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "RTU 请求结构异常",
            ));
        }
        req.length = n;
        Ok(Some(req))
    }
    /// 按四区映射应答，返回写入驱动的字节数；广播写入可更新映射但返回 0，不发送响应。
    ///
    /// # Errors
    /// C 应答或串口非阻塞写入失败时返回错误，不自动重试或无限等待。
    pub fn reply(&mut self, req: &RtuRequest, mapping: &mut Mapping) -> io::Result<usize> {
        // SAFETY: 私有构造请求已检查协议与结构；context 和映射均存活且独占借用。
        checked(unsafe {
            sys::modbus_reply(
                self.handle.as_ptr(),
                req.bytes.as_ptr(),
                req.length as i32,
                mapping.handle.as_ptr(),
            )
        })
    }
    /// 发送异常响应；广播直接返回 0，不调用上游会发送广播异常的函数。
    ///
    /// # Errors
    /// C 应答或串口写入失败时返回错误；返回字节数不表示主站已收到。
    pub fn reply_exception(&mut self, req: &RtuRequest, exception: Exception) -> io::Result<usize> {
        if req.is_broadcast() {
            return Ok(0);
        }
        // SAFETY: 请求来自成功接收，context 存活且独占，异常码来自受限枚举。
        checked(unsafe {
            sys::modbus_reply_exception(self.handle.as_ptr(), req.bytes.as_ptr(), exception as u32)
        })
    }
}
impl Drop for RtuContext {
    fn drop(&mut self) {
        // SAFETY: 唯一句柄，不存在未结束的安全借用；仅成功打开后 close 一次，再 free 一次。
        unsafe {
            if self.opened {
                sys::modbus_close(self.handle.as_ptr());
            }
            sys::modbus_free(self.handle.as_ptr());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn opened_failure_and_normal_drop_restore_the_serial_and_do_not_close_reused_fds() {
        use std::{
            io::{Read, Write},
            os::{fd::AsRawFd, unix::net::UnixStream},
        };
        let pty = crate::test_pty::Pty::new();
        let cfg = RtuConfig::new(&pty.path, 19200, 'N', 8, 2, 1, 50, 2).unwrap();
        let flags = || {
            let mut t = std::mem::MaybeUninit::<libc::termios>::uninit();
            // SAFETY: master fd 存活且输出 termios 空间有效，成功后才能读取初始化值。
            assert_eq!(
                unsafe { libc::tcgetattr(pty.wire.as_raw_fd(), t.as_mut_ptr()) },
                0
            );
            // SAFETY: 上述 tcgetattr 成功初始化完整 termios。
            unsafe { t.assume_init().c_lflag & (libc::ICANON | libc::ECHO | libc::ISIG) }
        };
        let original = flags();
        for _ in 0..30 {
            assert!(RtuContext::open_inner(&cfg, 2).is_err());
            assert_eq!(flags(), original);
            let ctx = RtuContext::open(&cfg).unwrap();
            assert_eq!(flags() & libc::ICANON, 0);
            drop(ctx);
            assert_eq!(flags(), original);
            let (mut a, mut b) = UnixStream::pair().unwrap();
            a.write_all(&[42]).unwrap();
            let mut byte = [0];
            b.read_exact(&mut byte).unwrap();
            assert_eq!(byte, [42]);
        }
    }
    #[test]
    fn allocation_failure_keeps_original_error() {
        let cfg = RtuConfig::new("/missing", 19200, 'N', 8, 2, 1, 50, 2).unwrap();
        let err = RtuContext::open_inner(&cfg, 1).err().unwrap();
        assert_eq!(err.to_string(), "注入：RTU 分配后失败");
    }
}
