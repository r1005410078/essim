//! 同步 TCP/RTU 主站；串行请求、有限响应等待，无自动重试。
use crate::{checked, invalid, sys, RtuConfig, RtuContext, TcpContext};
use std::{
    io,
    net::{SocketAddr, TcpStream},
    time::Duration,
};

/// 从保留的 libmodbus errno 中提取异常码（1–11）；I/O 或无效响应返回 None。
/// 异常码表示远端拒绝/报告状态，不表示写入成功。
pub fn exception_code(error: &io::Error) -> Option<u8> {
    error.raw_os_error().and_then(|v| {
        let code = v.checked_sub(sys::MODBUS_ENOBASE)?;
        (1..=11).contains(&code).then_some(code as u8)
    })
}
enum Channel {
    Tcp(TcpContext<TcpStream>),
    Rtu(RtuContext),
    Local(TcpContext<std::os::unix::net::UnixStream>),
    RtuLocal {
        context: RtuContext,
        socket: std::os::unix::net::UnixStream,
    },
}

/// 主动访问 TCP 或 RTU 从站的同步主站，独占通道且不实现 Send/Sync。
///
/// 地址均为零基协议地址；读取返回 bool/u16 原始值，无倍率或字序转换。
/// 支持 FC01/02/03/04/05/06/0F/10，不包含轮询调度、广播或自动重连。
/// TCP 响应校验沿用上游：不严格校验响应 Unit ID 和 MBAP 长度，不作为不可信对端的协议防火墙。
/// response_timeout 为上游等待语义，依赖平台 select 行为，不承诺墙钟整次操作截止时间。
/// 一次只允许一个在途请求；合法异常保留连接，直接TCP/RTU通道的I/O/超时/无效响应关闭后须重新构造。
/// 私有RTU通道遇协议错误保留句柄，由运行所有者隔离真实串口并drain后才允许下一请求。
/// 写入超时可能已在远端生效，禁止据此自动重试控制命令。
///
/// ```compile_fail
/// fn send(client: libmodbus_rs::ModbusClient) {
///     std::thread::spawn(move || drop(client));
/// }
/// ```
pub struct ModbusClient {
    channel: Option<Channel>,
}
fn timeout_parts(timeout: Duration) -> io::Result<(u32, u32)> {
    let seconds = u32::try_from(timeout.as_secs()).map_err(|_| invalid("超时秒数超过 u32"))?;
    if seconds == 0 && timeout.subsec_micros() == 0 {
        return Err(invalid("超时至少为 1 微秒"));
    }
    Ok((seconds, timeout.subsec_micros()))
}
impl ModbusClient {
    /// 主动连接数值 TCP 地址；连接与响应分别使用有限超时，Unit ID 为 0–255。
    /// 设置上游 response_timeout 并禁用 byte_timeout；socket 由 Rust 持有，C 仅借用，Drop 不重复关闭。
    ///
    /// # Errors
    /// 超时无效、连接/配置失败或运行库版本不符时返回错误。
    pub fn tcp(
        address: SocketAddr,
        unit: u8,
        connect_timeout: Duration,
        response_timeout: Duration,
    ) -> io::Result<Self> {
        timeout_parts(connect_timeout)?;
        timeout_parts(response_timeout)?;
        let socket = TcpStream::connect_timeout(&address, connect_timeout)?;
        socket.set_write_timeout(Some(response_timeout))?;
        let mut client = Self {
            channel: Some(Channel::Tcp(TcpContext::with_socket(
                socket,
                response_timeout,
            )?)),
        };
        client.configure(unit, response_timeout)?;
        Ok(client)
    }
    /// 在调用方提供的私有Unix通道上执行TCP协议，供真实传输观测桥使用。
    /// socket所有权移入上下文；C仅借用句柄，Drop由Rust释放，不跨线程。
    /// # Errors
    /// 超时、上下文分配或配置失败时返回错误并释放socket。
    pub fn tcp_channel(
        socket: std::os::unix::net::UnixStream,
        unit: u8,
        timeout: Duration,
    ) -> io::Result<Self> {
        timeout_parts(timeout)?;
        socket.set_write_timeout(Some(timeout))?;
        let mut client = Self {
            channel: Some(Channel::Local(TcpContext::with_socket(socket, timeout)?)),
        };
        client.configure(unit, timeout)?;
        Ok(client)
    }
    /// 通过私有通道执行RTU协议；实际串口由调用方透传所有者管理。
    /// C仅借用socket，所有资源仍绑定当前线程。
    /// # Errors
    /// 参数、句柄创建或超时配置失败时拒绝。
    pub fn rtu_channel(
        config: &RtuConfig,
        socket: std::os::unix::net::UnixStream,
        timeout: Duration,
    ) -> io::Result<Self> {
        socket.set_write_timeout(Some(timeout))?;
        let context = RtuContext::channel(config, &socket)?;
        let mut client = Self {
            channel: Some(Channel::RtuLocal { context, socket }),
        };
        client.configure(1, timeout)?;
        Ok(client)
    }
    /// 清除私有RTU通道残留；调用方须已完成真实串口迟到数据隔离。
    /// # Errors
    /// 非私有RTU通道或本地I/O失败。
    pub fn drain_rtu_channel(&mut self) -> io::Result<()> {
        use std::io::Read;
        let Some(Channel::RtuLocal { socket, .. }) = &mut self.channel else {
            return Err(invalid("仅私有RTU通道可清理"));
        };
        socket.set_nonblocking(true)?;
        let mut buf = [0; 512];
        let result = loop {
            match socket.read(&mut buf) {
                Ok(0) => break Err(io::ErrorKind::BrokenPipe.into()),
                Ok(_) => {}
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break Ok(()),
                Err(e) => break Err(e),
            }
        };
        socket.set_nonblocking(false)?;
        result
    }
    /// 手动执行只读串行诊断或回送测试；不开放广播、清零、重启或只听。
    /// 返回包含CRC的实际响应，校验站号、功能、长度、CRC及回送内容。
    /// # Errors
    /// 操作不在白名单、I/O、超时、响应不匹配或远端异常。
    pub fn diagnose(
        &mut self,
        unit: u8,
        function: u8,
        sub: u16,
        data: u16,
        timeout: Duration,
    ) -> io::Result<Vec<u8>> {
        use std::io::Read;
        timeout_parts(timeout)?;
        if !matches!(self.channel, Some(Channel::RtuLocal { .. })) {
            return Err(invalid("诊断仅在受控RTU通道执行"));
        }
        if function != 8 && (sub != 0 || data != 0)
            || !(1..=247).contains(&unit)
            || !matches!(function, 7 | 8 | 11 | 12 | 17)
            || function == 8 && !(sub == 0 || sub == 2 || (11..=18).contains(&sub))
            || function == 8 && sub != 0 && data != 0
        {
            return Err(invalid("不支持该诊断或子功能参数"));
        }
        self.set_unit(unit)?;
        let mut request = vec![unit, function];
        if function == 8 {
            request.extend(sub.to_be_bytes());
            request.extend(data.to_be_bytes());
        }
        let h = self.handle()?;
        // SAFETY: 独占存活上下文，白名单请求为2或6字节且缓冲调用期间存活，C负责补CRC。
        checked(unsafe { sys::modbus_send_raw_request(h, request.as_ptr(), request.len() as i32) })
            .map_err(|e| io::Error::new(e.kind(), format!("诊断发送：{e}")))?;
        let Some(Channel::RtuLocal { socket, .. }) = &mut self.channel else {
            return Err(invalid("诊断仅在受控RTU通道执行"));
        };
        let deadline = std::time::Instant::now() + timeout;
        let mut response = Vec::new();
        fn receive(
            socket: &mut std::os::unix::net::UnixStream,
            out: &mut Vec<u8>,
            count: usize,
            deadline: std::time::Instant,
        ) -> io::Result<()> {
            while out.len() < count {
                let left = deadline
                    .checked_duration_since(std::time::Instant::now())
                    .filter(|v| !v.is_zero())
                    .ok_or_else(|| io::Error::from(io::ErrorKind::TimedOut))?;
                let mut ready = libc::pollfd {
                    fd: std::os::fd::AsRawFd::as_raw_fd(socket),
                    events: libc::POLLIN,
                    revents: 0,
                };
                // SAFETY: 当前线程独占存活socket；ready指向一个pollfd，有限等待不超过剩余时间取整后的1ms误差。
                let n = unsafe {
                    libc::poll(
                        &mut ready,
                        1,
                        left.as_millis().max(1).min(i32::MAX as u128) as i32,
                    )
                };
                if n == 0 {
                    return Err(io::ErrorKind::TimedOut.into());
                }
                if n < 0 {
                    return Err(io::Error::last_os_error());
                }
                let mut buf = [0; 256];
                let n = socket
                    .read(&mut buf[..count - out.len()])
                    .map_err(|e| io::Error::new(e.kind(), format!("诊断读取：{e}")))?;
                if n == 0 {
                    return Err(io::ErrorKind::BrokenPipe.into());
                }
                out.extend_from_slice(&buf[..n]);
            }
            Ok(())
        }
        receive(socket, &mut response, 2, deadline)?;
        if response[0] != unit || response[1] != function && response[1] != function | 0x80 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "诊断响应站号／功能码不匹配",
            ));
        }
        let count = if response[1] & 0x80 != 0 {
            5
        } else {
            match function {
                7 => 5,
                8 | 11 => 8,
                _ => {
                    receive(socket, &mut response, 3, deadline)?;
                    5 + response[2] as usize
                }
            }
        };
        if !(5..=256).contains(&count) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "诊断响应长度越界",
            ));
        }
        receive(socket, &mut response, count, deadline)?;
        if crate::rtu_crc(&response) != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "诊断CRC错误"));
        }
        if response[1] & 0x80 != 0 {
            return Err(io::Error::from_raw_os_error(
                sys::MODBUS_ENOBASE + i32::from(response[2]),
            ));
        }
        if function == 8
            && (response[2..4] != sub.to_be_bytes()
                || sub == 0 && response[4..6] != data.to_be_bytes())
            || function == 12 && response[2] < 6
            || function == 17 && response[2] < 2
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "诊断响应字段不匹配",
            ));
        }
        Ok(response)
    }
    /// 打开 RTU 串口主站；串口参数复用 RtuConfig，目标站号为其中的 1–247。
    /// 独立 response_timeout，不受从站接收等待 10–100ms 限制。
    /// 不启用 RTS 方向控制；真实 RS-485 时序和驱动须在目标硬件另验。
    ///
    /// # Errors
    /// 超时无效、串口打开或配置失败时返回错误并清理已获得资源。
    pub fn rtu(config: &RtuConfig, response_timeout: Duration) -> io::Result<Self> {
        let (sec, usec) = timeout_parts(response_timeout)?;
        let context = RtuContext::open(config)?;
        let mut client = Self {
            channel: Some(Channel::Rtu(context)),
        };
        let h = client.handle()?;
        // SAFETY: 独占已打开 context，超时已校验；禁用字节超时沿用上游 response_timeout 等待语义。
        unsafe {
            checked(sys::modbus_set_response_timeout(h, sec, usec))?;
            checked(sys::modbus_set_byte_timeout(h, 0, 0))?;
        }
        Ok(client)
    }
    fn handle(&mut self) -> io::Result<*mut sys::modbus_t> {
        match self.channel.as_mut() {
            Some(Channel::Tcp(c)) => Ok(c.handle.as_ptr()),
            Some(Channel::Rtu(c)) => Ok(c.handle.as_ptr()),
            Some(Channel::Local(c)) => Ok(c.handle.as_ptr()),
            Some(Channel::RtuLocal { context, .. }) => Ok(context.handle.as_ptr()),
            None => Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "主站通道已关闭，请重新连接",
            )),
        }
    }
    fn configure(&mut self, unit: u8, timeout: Duration) -> io::Result<()> {
        self.set_unit(unit)?;
        let (sec, usec) = timeout_parts(timeout)?;
        let h = self.handle()?;
        // SAFETY: context 存活且独占，超时参数已校验，没有在途请求。
        unsafe {
            checked(sys::modbus_set_response_timeout(h, sec, usec))?;
            checked(sys::modbus_set_byte_timeout(h, 0, 0))?;
        }
        Ok(())
    }
    /// 切换后续请求目标：TCP 0–255，RTU 1–247；不发送请求。
    ///
    /// # Errors
    /// RTU 站号越界、通道已关闭或 C 配置失败时返回错误。
    pub fn set_unit(&mut self, unit: u8) -> io::Result<()> {
        if matches!(
            self.channel,
            Some(Channel::Rtu(_) | Channel::RtuLocal { .. })
        ) && !(1..=247).contains(&unit)
        {
            return Err(invalid("RTU 主站目标须为 1–247，不支持广播"));
        }
        let h = self.handle()?;
        // SAFETY: context 独占且存活，无在途请求，站号符合当前传输约束。
        checked(unsafe { sys::modbus_set_slave(h, unit.into()) })?;
        Ok(())
    }
    fn range(address: u16, count: usize, max: usize) -> io::Result<()> {
        if count == 0 || count > max || count > 65536 - usize::from(address) {
            Err(invalid("数量为零、超过功能码上限或地址区间溢出"))
        } else {
            Ok(())
        }
    }
    fn finish(&mut self, result: i32, expected: usize) -> io::Result<()> {
        let error = if result < 0 {
            io::Error::last_os_error()
        } else if result as usize == expected {
            return Ok(());
        } else {
            io::Error::new(io::ErrorKind::InvalidData, "响应数量与请求不符")
        };
        if exception_code(&error).is_none()
            && !matches!(self.channel, Some(Channel::RtuLocal { .. }))
        {
            self.channel.take();
        }
        Err(error)
    }
    /// 主动读取 coil 区，最多 2000 点；地址为零基。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效响应或远端异常时返回错误。
    pub fn read_coil(&mut self, address: u16, count: u16) -> io::Result<Vec<bool>> {
        Self::range(address, count as usize, 2000)?;
        let mut data = vec![0; count as usize];
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；区间已校验，缓冲含 count 个对齐可写元素且调用期间存活。
        let n =
            unsafe { sys::modbus_read_bits(h, address.into(), count.into(), data.as_mut_ptr()) };
        self.finish(n, count as usize)?;
        Ok(data.into_iter().map(|b| b != 0).collect())
    }
    /// 主动读取 discrete 区，最多 2000 点；地址为零基。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效响应或远端异常时返回错误。
    pub fn read_discrete(&mut self, address: u16, count: u16) -> io::Result<Vec<bool>> {
        Self::range(address, count as usize, 2000)?;
        let mut data = vec![0; count as usize];
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；区间已校验，缓冲含 count 个对齐可写元素且调用期间存活。
        let n = unsafe {
            sys::modbus_read_input_bits(h, address.into(), count.into(), data.as_mut_ptr())
        };
        self.finish(n, count as usize)?;
        Ok(data.into_iter().map(|b| b != 0).collect())
    }
    /// 主动读取 holding 区，最多 125 点；地址为零基。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效响应或远端异常时返回错误。
    pub fn read_holding(&mut self, address: u16, count: u16) -> io::Result<Vec<u16>> {
        Self::range(address, count as usize, 125)?;
        let mut data = vec![0; count as usize];
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；区间已校验，缓冲含 count 个对齐可写元素且调用期间存活。
        let n = unsafe {
            sys::modbus_read_registers(h, address.into(), count.into(), data.as_mut_ptr())
        };
        self.finish(n, count as usize)?;
        Ok(data)
    }
    /// 主动读取 input 区，最多 125 点；地址为零基。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效响应或远端异常时返回错误。
    pub fn read_input(&mut self, address: u16, count: u16) -> io::Result<Vec<u16>> {
        Self::range(address, count as usize, 125)?;
        let mut data = vec![0; count as usize];
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；区间已校验，缓冲含 count 个对齐可写元素且调用期间存活。
        let n = unsafe {
            sys::modbus_read_input_registers(h, address.into(), count.into(), data.as_mut_ptr())
        };
        self.finish(n, count as usize)?;
        Ok(data)
    }
    /// 写入单个 coil，成功表示收到匹配的确认；超时可能已执行，不自动重试。
    ///
    /// # Errors
    /// 通道关闭、超时、I/O、无效确认或远端异常时返回错误。
    pub fn write_coil(&mut self, address: u16, value: bool) -> io::Result<()> {
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；u16 地址和值/布尔值均满足协议参数范围。
        let n = unsafe { sys::modbus_write_bit(h, address.into(), i32::from(value)) };
        self.finish(n, 1)
    }
    /// 写入单个 holding，成功表示收到匹配的确认；超时可能已执行，不自动重试。
    ///
    /// # Errors
    /// 通道关闭、超时、I/O、无效确认或远端异常时返回错误。
    pub fn write_holding(&mut self, address: u16, value: u16) -> io::Result<()> {
        let h = self.handle()?;
        // SAFETY: 独占已连接 context；u16 地址和值/布尔值均满足协议参数范围。
        let n = unsafe { sys::modbus_write_register(h, address.into(), value) };
        self.finish(n, 1)
    }
    /// 连续写入 coils，最多 1968 点；收到匹配确认才成功，不自动重试。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效确认或远端异常时返回错误。
    pub fn write_coils(&mut self, address: u16, values: &[bool]) -> io::Result<()> {
        Self::range(address, values.len(), 1968)?;
        let data: Vec<u8> = values.iter().copied().map(u8::from).collect();
        let h = self.handle()?;
        // SAFETY: 区间/数量已校验，独占 context；连续输入缓冲对齐、存活且长度至少为请求数量。
        let n = unsafe {
            sys::modbus_write_bits(h, address.into(), values.len() as i32, data.as_ptr())
        };
        self.finish(n, values.len())
    }
    /// 连续写入 holdings，最多 123 点；收到匹配确认才成功，不自动重试。
    ///
    /// # Errors
    /// 区间无效、通道关闭、超时、I/O、无效确认或远端异常时返回错误。
    pub fn write_holdings(&mut self, address: u16, values: &[u16]) -> io::Result<()> {
        Self::range(address, values.len(), 123)?;
        let data = values;
        let h = self.handle()?;
        // SAFETY: 区间/数量已校验，独占 context；连续输入缓冲对齐、存活且长度至少为请求数量。
        let n = unsafe {
            sys::modbus_write_registers(h, address.into(), values.len() as i32, data.as_ptr())
        };
        self.finish(n, values.len())
    }
}

#[cfg(test)]
mod rtu_diagnostic_tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };
    #[test]
    fn forbidden_diagnostic_never_sends_and_known_crc_matches() {
        assert_eq!(crate::rtu_crc(&[1, 3, 0, 10, 0, 2, 0xe4, 9]), 0);
        let (a, mut b) = UnixStream::pair().unwrap();
        b.set_nonblocking(true).unwrap();
        let cfg = RtuConfig::new("/unused", 19200, 'N', 8, 2, 1, 50, 2).unwrap();
        let mut c = ModbusClient::rtu_channel(&cfg, a, Duration::from_millis(100)).unwrap();
        for sub in [1, 3, 4, 10, 20] {
            assert!(c
                .diagnose(1, 8, sub, 0, Duration::from_millis(100))
                .is_err());
        }
        let mut buf = [0; 10];
        assert_eq!(
            b.read(&mut buf).unwrap_err().kind(),
            io::ErrorKind::WouldBlock
        );
    }
    #[test]
    fn raw_diagnostics_validate_actual_crc_and_echo() {
        for (fc, sub, body) in [
            (7, 0, vec![1, 7, 5]),
            (8, 0, vec![1, 8, 0, 0, 0x12, 0x34]),
            (11, 0, vec![1, 11, 0, 0, 0, 3]),
            (12, 0, vec![1, 12, 6, 0, 0, 0, 1, 0, 2]),
            (17, 0, vec![1, 17, 2, 7, 255]),
        ] {
            let (a, mut b) = UnixStream::pair().unwrap();
            let cfg = RtuConfig::new("/unused", 19200, 'N', 8, 2, 1, 50, 2).unwrap();
            let mut c = ModbusClient::rtu_channel(&cfg, a, Duration::from_millis(100)).unwrap();
            let response = crate::test_pty::adu(&body);
            let expected = response.clone();
            let worker = std::thread::spawn(move || {
                let mut req = vec![0; if fc == 8 { 8 } else { 4 }];
                b.read_exact(&mut req).unwrap();
                assert_eq!(crate::rtu_crc(&req), 0);
                for chunk in response.chunks(2) {
                    b.write_all(chunk).unwrap();
                }
            });
            assert_eq!(
                c.diagnose(
                    1,
                    fc,
                    sub,
                    if fc == 8 { 0x1234 } else { 0 },
                    Duration::from_millis(100)
                )
                .unwrap_or_else(|e| panic!("FC {fc}: {e:?}")),
                expected
            );
            worker.join().unwrap();
        }
    }
}

#[cfg(test)]mod diagnostic_integrity_tests{
 use super::*;use std::{os::unix::net::UnixStream,io::{Read,Write}};
 #[test]fn bad_crc_wrong_echo_and_exception_are_not_success(){
  for mode in 0..3{
   let(a,mut b)=UnixStream::pair().unwrap();let cfg=RtuConfig::new("/unused",19200,'N',8,2,1,50,2).unwrap();let mut c=ModbusClient::rtu_channel(&cfg,a,Duration::from_millis(100)).unwrap();
   let worker=std::thread::spawn(move||{let mut request=[0;8];b.read_exact(&mut request).unwrap();let body=match mode{0=>vec![1,8,0,0,0x12,0x35],1=>vec![1,8,0,0,0x12,0x34],_=>vec![1,0x88,1]};let mut bytes=crate::test_pty::adu(&body);if mode==1{let last=bytes.len()-1;bytes[last]^=1;}b.write_all(&bytes).unwrap();});
   let error=c.diagnose(1,8,0,0x1234,Duration::from_millis(100)).unwrap_err();if mode==2{assert_eq!(exception_code(&error),Some(1));}else{assert_eq!(error.kind(),io::ErrorKind::InvalidData);}worker.join().unwrap();
  }
 }
}
