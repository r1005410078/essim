use std::net::Ipv4Addr;

pub const INSTANCE_ID: &str = "demo-bms";
pub const POINT_ID: &str = "soc";
pub const SOC_ADDRESS: u16 = 100;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Soc(u16);
impl Soc {
    pub const INITIAL: Self = Self(650);
    pub fn new(value: f64) -> Result<Self, String> {
        if !value.is_finite() || !(0.0..=1000.0).contains(&value) || value.fract() != 0.0 {
            return Err("SOC 原始值必须为 0–1000 的整数".into());
        }
        Ok(Self(value as u16))
    }
    pub fn raw(self) -> u16 {
        self.0
    }
    pub fn engineering(self) -> f64 {
        self.0 as f64 / 10.0
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    pub address: Ipv4Addr,
    pub port: u16,
}
impl Endpoint {
    pub fn new(address: &str, port: u16) -> Result<Self, String> {
        if port == 0 {
            return Err("端口必须为 1–65535".into());
        }
        Ok(Self {
            address: address.parse().map_err(|_| "必须使用 IPv4 地址")?,
            port,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Stopped,
    Starting,
    Listening,
    Stopping,
    Failed,
}
#[derive(Debug)]
pub struct Lifecycle(State);
impl Default for Lifecycle {
    fn default() -> Self {
        Self(State::Stopped)
    }
}
impl Lifecycle {
    pub fn state(&self) -> State {
        self.0
    }
    pub fn starting(&mut self) -> Result<(), String> {
        if !matches!(self.0, State::Stopped | State::Failed) {
            return Err("模拟器已启动".into());
        }
        self.0 = State::Starting;
        Ok(())
    }
    pub fn listening(&mut self) {
        self.0 = State::Listening;
    }
    pub fn stopping(&mut self) {
        self.0 = State::Stopping;
    }
    pub fn stopped(&mut self) {
        self.0 = State::Stopped;
    }
    pub fn failed(&mut self) {
        self.0 = State::Failed;
    }
}

/// 当前模拟器允许的单元标识，产品配置范围为 1–247，不表示覆盖全部 TCP Unit ID 用途。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UnitId(u8);
impl UnitId {
    /// 默认教学单元，与旧单 SOC 接口保持兼容。
    pub const DEFAULT: Self = Self(1);
    /// 校验新增单元标识。
    ///
    /// # Errors
    /// 不在本阶段配置范围 1–247 内时返回错误。
    pub fn new(value: u16) -> Result<Self, String> {
        if !(1..=247).contains(&value) {
            return Err("Unit ID 必须为 1–247 的整数".into());
        }
        Ok(Self(value as u8))
    }
    /// 返回报文中的单字节单元标识。
    pub fn raw(self) -> u8 {
        self.0
    }
}

/// 单串口单从站的通信配置；字段单位为波特/秒、位及毫秒。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RtuEndpoint {
    /// 系统串口路径，不能包含 NUL。
    pub device: String,
    /// 当前支持 9600、19200、38400、57600、115200。
    pub baud: u32,
    /// N/E/O：无/偶/奇校验。
    pub parity: char,
    /// 首版固定为 8 数据位。
    pub data_bits: u8,
    /// 1 或 2 停止位。
    pub stop_bits: u8,
    /// 单一 RTU 从站号，1–247。
    pub slave: UnitId,
    /// 新请求/非本站确认等待，10–100ms。
    pub receive_timeout_ms: u32,
    /// 相邻字节等待，1–2ms。
    pub byte_timeout_ms: u32,
}
impl RtuEndpoint {
    /// 校验整组串口配置；这些是首版产品限制，不代表所有平台已完成硬件验证。
    ///
    /// # Errors
    /// 空/NUL 路径、不支持的串口参数、站号或超时时返回错误，不返回半配置对象。
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
    ) -> Result<Self, String> {
        if device.trim().is_empty() || device.contains('\0') {
            return Err("串口路径不能为空或含 NUL".into());
        }
        if ![9600, 19200, 38400, 57600, 115200].contains(&baud)
            || !['N', 'E', 'O'].contains(&parity)
            || data_bits != 8
            || ![1, 2].contains(&stop_bits)
        {
            return Err("串口参数不在首版支持范围内".into());
        }
        if !(10..=100).contains(&receive_timeout_ms) || !(1..=2).contains(&byte_timeout_ms) {
            return Err("接收超时须为 10–100ms，字节间超时为 1–2ms".into());
        }
        Ok(Self {
            device: device.into(),
            baud,
            parity,
            data_bits,
            stop_bits,
            slave: UnitId::new(slave)?,
            receive_timeout_ms,
            byte_timeout_ms,
        })
    }
}
/// 已应用的通信方式；TCP 与 RTU 使用不同的端点语义。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Communication {
    /// TCP 监听端点，支持同一端点多个 Unit ID。
    Tcp(Endpoint),
    /// 单串口单从站。
    Rtu(RtuEndpoint),
}
