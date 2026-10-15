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
}

/// 主动访问 TCP 或 RTU 从站的同步主站，独占通道且不实现 Send/Sync。
///
/// 地址均为零基协议地址；读取返回 bool/u16 原始值，无倍率或字序转换。
/// 支持 FC01/02/03/04/05/06/0F/10，不包含轮询调度、广播或自动重连。
/// TCP 响应校验沿用上游：不严格校验响应 Unit ID 和 MBAP 长度，不作为不可信对端的协议防火墙。
/// response_timeout 为上游等待语义，依赖平台 select 行为，不承诺墙钟整次操作截止时间。
/// 一次只允许一个在途请求；合法异常保留连接，I/O/超时/无效响应立即关闭，须重新构造。
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
        if matches!(self.channel, Some(Channel::Rtu(_))) && !(1..=247).contains(&unit) {
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
        if exception_code(&error).is_none() {
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
