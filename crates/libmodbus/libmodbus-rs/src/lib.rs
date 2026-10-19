//! libmodbus 3.2.0 TCP/RTU 主站及从站 API 子集，不包含设备或运行调度。
//! TCP context借用Rust socket；RTU直接上下文独占串口，私有通道上下文借用Unix socket。
//! SerialPort独占实际串口，句柄禁止跨线程，无C回调。
mod responder;
/// 完整TCP帧响应生成器。
pub use responder::TcpResponder;
mod client;
pub use client::{exception_code, ModbusClient};
mod rtu;
#[cfg(test)]
#[path = "../tests/support/mod.rs"]
mod test_pty;
use libmodbus_sys as sys;
pub use rtu::{RtuConfig, RtuContext, RtuRequest, RTU_MAX_ADU_LENGTH};
use std::{
    io,
    marker::PhantomData,
    os::fd::{AsFd, AsRawFd},
    ptr::NonNull,
    rc::Rc,
    time::Duration,
};
pub const MAX_ADU_LENGTH: usize = sys::MAX_ADU_LENGTH;
pub fn library_version() -> (u32, u32, u32) {
    unsafe {
        (
            sys::libmodbus_version_major,
            sys::libmodbus_version_minor,
            sys::libmodbus_version_micro,
        )
    }
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn checked(n: i32) -> io::Result<usize> {
    if n < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(n as usize)
    }
}

/// 持有一段连续的 Modbus 保持寄存器（Holding Registers）及其 C 映射资源。
///
/// 每个寄存器保存一个原始 `u16` 值；单位、倍率、SOC 等业务含义由调用方解释。
/// 地址使用协议中的零基地址，不是文档中常见的 40001 等展示编号。
/// `set` / `get` 直接访问本地映射，不发送网络请求；将映射交给
/// [`TcpContext::reply`] 后，libmodbus 才会按客户端请求读取或写入其中的值。
/// 是否允许客户端写入由上层策略决定，这个类型本身不限制为只读。
///
/// 对象独占 C 映射，在 `Drop` 时自动释放；不实现 `Send` / `Sync`，
/// 必须在创建它的线程内使用。
pub struct HoldingRegisters {
    mapping: Mapping,
}
impl HoldingRegisters {
    /// 创建仅含保持寄存器的映射，初始值为 0；地址为零基协议地址。
    ///
    /// # Errors
    /// 区间为空或超出 0–65535、运行库版本不符或 C 分配失败时返回错误。
    pub fn new(start: u16, count: u32) -> io::Result<Self> {
        Ok(Self {
            mapping: Mapping::new(None, None, Some(AddressRange::new(start, count)?), None)?,
        })
    }
    /// 更新本地保持寄存器，不发送网络请求。
    ///
    /// # Errors
    /// 地址越界或 C 辅助访问失败时返回错误。
    pub fn set(&mut self, address: u16, value: u16) -> io::Result<()> {
        self.mapping.set_holding(address, value)
    }
    /// 读取本地保持寄存器的原始值。
    ///
    /// # Errors
    /// 地址越界或 C 辅助访问失败时返回错误。
    pub fn get(&self, address: u16) -> io::Result<u16> {
        self.mapping.holding(address)
    }
}
impl std::ops::Deref for HoldingRegisters {
    type Target = Mapping;
    fn deref(&self) -> &Mapping {
        &self.mapping
    }
}
impl std::ops::DerefMut for HoldingRegisters {
    fn deref_mut(&mut self) -> &mut Mapping {
        &mut self.mapping
    }
}

/// 一个非空、连续的零基 Modbus 地址区间，终点不超过 65535。
#[derive(Debug, Clone, Copy)]
pub struct AddressRange {
    start: u16,
    count: u32,
}
impl AddressRange {
    /// 创建起始地址及数量；例如 `(100, 3)` 表示地址 100、101、102。
    ///
    /// # Errors
    /// 数量为 0 或末地址超过 65535 时返回错误。
    pub fn new(start: u16, count: u32) -> io::Result<Self> {
        if count == 0 || count > 65536 - u32::from(start) {
            return Err(invalid("数据区间必须非空且落在 0–65535"));
        }
        Ok(Self { start, count })
    }
}

/// 保存 Modbus 四类数据区的本地值，并提供给 libmodbus 生成请求响应。
///
/// | 名称 | 数据区 | 每点大小 / 本地类型 | 客户端访问 | 常用功能码（十六进制） |
/// | --- | --- | --- | --- | --- |
/// | `holding` | 保持寄存器 | 16 bit / `u16` | 读、写 | 03 读，06 单写，10 多写 |
/// | `input` | 输入寄存器 | 16 bit / `u16` | 只读 | 04 读 |
/// | `coil` | 线圈 | 1 bit / `bool` | 读、写 | 01 读，05 单写，0F 多写 |
/// | `discrete` | 离散输入 | 1 bit / `bool` | 只读 | 02 读 |
///
/// 客户端访问权限指网络请求允许的操作；服务端内部可用 `set_*` 更新全部四区。
/// 上层仍可施加更严格的策略，例如拒绝客户端对某个保持寄存器的写入。
///
/// 用“数据区 + 零基地址”定位一个点：`holding(100)` 和 `input(100)` 是两个独立
/// 位置。一个寄存器是存值的格子，地址是格子的编号，值是格子当前保存的内容。
/// 地址不是数组下标，也不是文档常见的 40001 等展示编号；创建 `(100, 2)` 区间后，
/// 使用地址 100、101 访问，而非 0、1。每区当前支持一段连续区间，`None` 表示不分配。
///
/// 寄存器仅保存原始 `u16`（0–65535）；SOC、单位、倍率、有符号数或多寄存器字序
/// 由上层点定义解释。映射不会自动换算数值或执行设备控制逻辑。
///
/// 本类型不监听网络，`set_*` / 读取方法也不发请求。接收请求后可调用
/// `context.reply(&request, &mut mapping)`，由 [`TcpContext::reply`] 访问映射并应答。
/// 对象独占 C 资源，在 `Drop` 时自动释放，不实现 `Send` / `Sync`。
///
/// # Examples
///
/// 以下地址、值与倍率均为教学约定，不是标准设备点表。构造参数顺序为
/// `coil`、`discrete`、`holding`、`input`；各区初值均为 0（位区为 false）。
///
/// ```
/// use libmodbus_rs::{AddressRange, Mapping};
/// # fn main() -> std::io::Result<()> {
/// let mut map = Mapping::new(
///     Some(AddressRange::new(0, 2)?),   // coil：地址 0、1
///     Some(AddressRange::new(0, 1)?),   // discrete：地址 0
///     Some(AddressRange::new(100, 2)?), // holding：地址 100、101
///     Some(AddressRange::new(100, 1)?), // input：地址 100
/// )?;
///
/// map.set_coil(0, true)?;       // 教学运行使能
/// map.set_discrete(0, false)?;  // 教学故障状态，服务端内部更新
/// map.set_holding(100, 650)?;   // 教学 SOC 原始值；倍率 0.1，表示 65.0%
/// map.set_input(100, 2300)?;    // 教学电压原始值；倍率 0.1，表示 230.0 V
///
/// assert!(map.coil(0)?);
/// assert!(!map.discrete(0)?);
/// assert_eq!(map.holding(100)?, 650);
/// assert_eq!(map.input(100)?, 2300); // 同地址、不同区，值互不影响
/// assert_eq!(map.holding(101)?, 0); // 未更新的格子保持初值
/// assert!(map.holding(102).is_err()); // 越界访问返回错误
///
/// // 也可以只分配保持寄存器，其他数据区不分配。
/// let only_holding = Mapping::new(None, None, Some(AddressRange::new(100, 1)?), None)?;
/// assert!(only_holding.coil(0).is_err());
/// # Ok(())
/// # }
/// ```
pub struct Mapping {
    // 不在 Rust 侧解引用 C 布局；所有数组访问均由 C 辅助函数校验。
    handle: NonNull<sys::modbus_mapping_t>,
    // 不实际持有 Rc，仅阻止 Send/Sync。
    _thread_bound: PhantomData<Rc<()>>,
}
impl Mapping {
    /// 按线圈、离散输入、保持寄存器、输入寄存器的顺序创建映射，初值均为 0。
    ///
    /// `None` 表示不分配该区；允许四区均为空，此时所有地址均不可访问。
    ///
    /// # Errors
    /// 运行库不是 3.2.0 或 C 库分配失败时返回错误。
    pub fn new(
        coil: Option<AddressRange>,
        discrete: Option<AddressRange>,
        holding: Option<AddressRange>,
        input: Option<AddressRange>,
    ) -> io::Result<Self> {
        if library_version() != (3, 2, 0) {
            return Err(invalid("运行库必须为 libmodbus 3.2.0"));
        }
        let parts = |r: Option<AddressRange>| r.map_or((0, 0), |r| (u32::from(r.start), r.count));
        let (a, b) = parts(coil);
        let (c, d) = parts(discrete);
        let (e, f) = parts(holding);
        let (g, h) = parts(input);
        // SAFETY: AddressRange 已保证所有非空区间合法；返回句柄检查非空并独占管理。
        let handle =
            NonNull::new(unsafe { sys::modbus_mapping_new_start_address(a, b, c, d, e, f, g, h) })
                .ok_or_else(io::Error::last_os_error)?;
        Ok(Self {
            handle,
            _thread_bound: PhantomData,
        })
    }
    fn read(&self, area: u32, address: u16) -> io::Result<u16> {
        let mut value = 0;
        // SAFETY: 映射存活且禁止跨线程；write=0 仅读取，输出指针有效且与映射不重叠。
        checked(unsafe {
            sys::libmodbus_rs_value(self.handle.as_ptr(), area, address.into(), &mut value, 0)
        })?;
        Ok(value)
    }
    fn write(&mut self, area: u32, address: u16, mut value: u16) -> io::Result<()> {
        // SAFETY: 独占借用存活映射；输入指针有效，C 辅助函数检查区间及位值。
        checked(unsafe {
            sys::libmodbus_rs_value(self.handle.as_ptr(), area, address.into(), &mut value, 1)
        })?;
        Ok(())
    }
    /// 读取本地线圈值。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn coil(&self, address: u16) -> io::Result<bool> {
        self.read(0, address).map(|v| v != 0)
    }
    /// 更新本地线圈，不发送网络请求。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn set_coil(&mut self, address: u16, value: bool) -> io::Result<()> {
        self.write(0, address, u16::from(value))
    }
    /// 读取本地离散输入值。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn discrete(&self, address: u16) -> io::Result<bool> {
        self.read(1, address).map(|v| v != 0)
    }
    /// 更新本地离散输入，不发送网络请求。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn set_discrete(&mut self, address: u16, value: bool) -> io::Result<()> {
        self.write(1, address, u16::from(value))
    }
    /// 读取本地保持寄存器值。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn holding(&self, address: u16) -> io::Result<u16> {
        self.read(2, address)
    }
    /// 更新本地保持寄存器，不发送网络请求。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn set_holding(&mut self, address: u16, value: u16) -> io::Result<()> {
        self.write(2, address, value)
    }
    /// 读取本地输入寄存器值。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn input(&self, address: u16) -> io::Result<u16> {
        self.read(3, address)
    }
    /// 更新本地输入寄存器，不发送网络请求。
    ///
    /// # Errors
    /// 数据区未分配、地址越界或 C 辅助访问失败时返回错误。
    pub fn set_input(&mut self, address: u16, value: u16) -> io::Result<()> {
        self.write(3, address, value)
    }
}
impl Drop for Mapping {
    fn drop(&mut self) {
        // SAFETY: 句柄由本对象独占，所有安全借用已结束，仅在此释放一次。
        unsafe { sys::modbus_mapping_free(self.handle.as_ptr()) }
    }
}

/// 私有构造，只有成功 receive 能生成有效请求。
pub struct Request {
    bytes: [u8; MAX_ADU_LENGTH],
    length: usize,
}
impl Request {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.length]
    }
}
#[derive(Debug, Clone, Copy)]
#[repr(u32)]
pub enum Exception {
    IllegalFunction = 1,
    IllegalAddress = 2,
    IllegalValue = 3,
    ServerFailure = 4,
    /// 配置端点下的目标Unit不可用。
    GatewayTargetUnavailable = 11,
}

/// 持有 socket owner，或 BorrowedFd guard。先销毁 C context 再释放 Rust owner。
///
/// ```compile_fail
/// use libmodbus_rs::TcpContext;
/// use std::{os::unix::net::UnixStream,time::Duration};
/// let (socket,_)=UnixStream::pair().unwrap();
/// let context=TcpContext::with_socket(socket,Duration::from_millis(100)).unwrap();
/// std::thread::spawn(move || drop(context)); // context 不能跨线程
/// ```
/// ```compile_fail
/// use libmodbus_rs::TcpContext;
/// use std::{os::{fd::AsFd,unix::net::UnixStream},time::Duration};
/// let context;
/// {let (socket,_)=UnixStream::pair().unwrap();
/// context=TcpContext::with_socket(socket.as_fd(),Duration::from_millis(100)).unwrap();}
/// drop(context); // socket 不得早于 context 失效
/// ```
pub struct TcpContext<S: AsFd> {
    handle: NonNull<sys::modbus_t>,
    socket: S,
    _thread_bound: PhantomData<Rc<()>>,
}
impl<S: AsFd> TcpContext<S> {
    pub fn with_socket(socket: S, timeout: Duration) -> io::Result<Self> {
        Self::attach(socket, timeout, 0)
    }
    fn attach(socket: S, timeout: Duration, fail_stage: u8) -> io::Result<Self> {
        let sec = u32::try_from(timeout.as_secs()).map_err(|_| invalid("超时秒数超过 u32 范围"))?;
        let usec = timeout.subsec_micros();
        if sec == 0 && usec == 0 {
            return Err(invalid("超时至少为 1 微秒"));
        }
        if library_version() != (3, 2, 0) {
            return Err(invalid("运行库必须为 libmodbus 3.2.0"));
        }
        // SAFETY: static nul-terminated address, no network connect is performed.
        let handle =
            NonNull::new(unsafe { sys::modbus_new_tcp(b"127.0.0.1\0".as_ptr().cast(), 0) })
                .ok_or_else(io::Error::last_os_error)?;
        let context = Self {
            handle,
            socket,
            _thread_bound: PhantomData,
        };
        // Private injection exercises the same early-return Drop path as setter errors.
        if fail_stage == 1 {
            return Err(io::Error::other("注入：创建 context 后失败"));
        }
        unsafe {
            // SAFETY: Rust owner/guard remains alive until after Drop, no C ownership transfer.
            checked(sys::modbus_set_socket(
                context.handle.as_ptr(),
                context.socket.as_fd().as_raw_fd(),
            ))?;
            if fail_stage == 2 {
                return Err(io::Error::other("注入：借用 socket 后失败"));
            }
            checked(sys::modbus_set_indication_timeout(
                context.handle.as_ptr(),
                sec,
                usec,
            ))?;
            checked(sys::modbus_set_byte_timeout(
                context.handle.as_ptr(),
                sec,
                usec,
            ))?;
        }
        Ok(context)
    }
    /// 接收使用 libmodbus 的分帧语义；网络应用仍应校验不可信 MBAP/超时。
    pub fn receive(&mut self) -> io::Result<Request> {
        let mut request = Request {
            bytes: [0; MAX_ADU_LENGTH],
            length: 0,
        };
        // SAFETY: maximum ADU-sized writable buffer and exclusively borrowed context.
        let n = checked(unsafe {
            sys::modbus_receive(self.handle.as_ptr(), request.bytes.as_mut_ptr())
        })?;
        if !(8..=MAX_ADU_LENGTH).contains(&n) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "请求长度异常"));
        }
        request.length = n;
        Ok(request)
    }
    /// 返回向绑定 socket 写出的字节数，不代表远端应用执行成功。
    ///
    /// ```compile_fail
    /// use libmodbus_rs::{TcpContext,HoldingRegisters};
    /// use std::{os::unix::net::UnixStream,time::Duration};
    /// let (socket,_)=UnixStream::pair().unwrap();
    /// let mut ctx=TcpContext::with_socket(socket,Duration::from_millis(100)).unwrap();
    /// let mut map=HoldingRegisters::new(0,1).unwrap();
    /// ctx.reply(&[0u8;4], &mut map); // 截断/伪造切片不是 Request
    /// ```
    pub fn reply(&mut self, request: &Request, mapping: &mut Mapping) -> io::Result<usize> {
        // SAFETY: Request came from receive; context and mapping are live/exclusive.
        checked(unsafe {
            sys::modbus_reply(
                self.handle.as_ptr(),
                request.bytes.as_ptr(),
                request.length as i32,
                mapping.handle.as_ptr(),
            )
        })
    }
    pub fn reply_exception(
        &mut self,
        request: &Request,
        exception: Exception,
    ) -> io::Result<usize> {
        // SAFETY: received Request and a valid protocol exception code.
        checked(unsafe {
            sys::modbus_reply_exception(
                self.handle.as_ptr(),
                request.bytes.as_ptr(),
                exception as u32,
            )
        })
    }
}
impl<S: AsFd> Drop for TcpContext<S> {
    fn drop(&mut self) {
        // SAFETY: unique live context. C never closes the borrowed descriptor.
        unsafe {
            sys::modbus_set_socket(self.handle.as_ptr(), -1);
            sys::modbus_free(self.handle.as_ptr());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::net::UnixStream,
    };
    #[test]
    fn failed_initialization_never_closes_a_borrowed_socket() {
        for stage in [1, 2] {
            for _ in 0..100 {
                let (mut a, mut b) = UnixStream::pair().unwrap();
                assert!(TcpContext::attach(a.as_fd(), Duration::from_millis(100), stage).is_err());
                a.write_all(&[42]).unwrap();
                let mut value = [0];
                b.read_exact(&mut value).unwrap();
                assert_eq!(value, [42]);
            }
        }
        for _ in 0..100 {
            let (a, mut b) = UnixStream::pair().unwrap();
            drop(TcpContext::with_socket(a, Duration::from_millis(100)).unwrap());
            assert_eq!(b.read(&mut [0]).unwrap(), 0);
        }
    }
    #[test]
    fn truncated_request_returns_timeout() {
        let (a, mut b) = UnixStream::pair().unwrap();
        let mut c = TcpContext::with_socket(a, Duration::from_millis(10)).unwrap();
        b.write_all(&[0, 1, 0, 0, 0, 6, 1, 3]).unwrap();
        assert!(c.receive().is_err());
    }
}

mod serial_port;
/// 独占Unix串口，仅供受控传输所有者使用。
pub use serial_port::SerialPort;
/// 计算RTU CRC16；对完整合法ADU返回0，仅用于观测及库未支持的受限诊断。
pub fn rtu_crc(bytes:&[u8])->u16{let mut crc=0xffff;for b in bytes{crc^=u16::from(*b);for _ in 0..8{crc=if crc&1!=0{(crc>>1)^0xa001}else{crc>>1};}}crc}
