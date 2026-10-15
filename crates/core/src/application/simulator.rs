use crate::domain::modbus::{Communication, Endpoint, RtuEndpoint, Soc, State, UnitId};
use std::{collections::BTreeMap, sync::mpsc::Receiver};

#[derive(Debug, Clone)]
pub struct Error {
    pub code: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}
pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub instance_id: &'static str,
    pub point_id: &'static str,
    pub sequence: u64,
    pub generation: u64,
    pub state: State,
    pub endpoint: Endpoint,
    /// 当前整体应用的通信配置，与 TCP 历史端点分开保存。
    pub communication: Communication,
    /// 最近应用的 RTU 配置，切回 TCP 后仍保留。
    pub rtu: Option<RtuEndpoint>,
    /// RTU 专用 SOC，不与任何 TCP 单元共享运行值。
    pub rtu_soc: Soc,
    /// 默认 Unit ID 1 的值，保留旧单点接口兼容。
    pub soc: Soc,
    /// 单元配置与运行值；键为稳定 Unit ID，各单元 SOC 相互独立。
    pub units: BTreeMap<UnitId, Soc>,
    pub connections: u8,
    pub error: Option<Error>,
    pub logs: Vec<PacketLog>,
}
#[derive(Debug, Clone)]
pub struct PacketLog {
    pub direction: &'static str,
    pub hex: String,
    pub message: String,
}
// 每条发布消息均包含完整快照；有界订阅队列允许合并高频变化。
// update 返回时，协议映射已完成更新；start 返回时，真实监听已建立。
pub trait SimulatorPort: Send + Sync {
    fn snapshot(&self) -> Snapshot;
    fn start(&self, endpoint: Endpoint) -> Result<Snapshot>;
    /// 停止状态整体应用通信配置。
    ///
    /// # Errors
    /// 运行中或整组配置无效时返回错误，保留原配置。
    fn configure(&self, communication: Communication) -> Result<Snapshot>;
    /// 启动单串口单从站，返回成功时设备已实际打开。
    ///
    /// # Errors
    /// 重复启动、参数错误、串口失败或启动确认失败时返回错误。
    fn start_rtu(&self, endpoint: RtuEndpoint) -> Result<Snapshot>;
    fn stop(&self) -> Result<Snapshot>;
    fn update(&self, value: Soc) -> Result<Snapshot>;
    /// 添加单元，运行中须等待协议映射创建完成后确认。
    ///
    /// # Errors
    /// 单元重复、资源创建失败、worker 退出或确认超时时返回错误。
    fn add_unit(&self, unit_id: UnitId) -> Result<Snapshot>;
    /// 更新当前通信方式下指定站号的运行值，成功后可由协议读取。
    ///
    /// # Errors
    /// 站号不存在、映射更新失败、worker 退出或确认超时时返回错误。
    fn update_unit(&self, unit_id: UnitId, value: Soc) -> Result<Snapshot>;
    fn subscribe(&self) -> Receiver<Snapshot>;
}
pub struct SimulatorService<P: SimulatorPort>(pub P);
impl<P: SimulatorPort> SimulatorService<P> {
    pub fn start(&self, address: &str, port: u16) -> Result<Snapshot> {
        self.0
            .start(Endpoint::new(address, port).map_err(|e| Error::new("INVALID_ENDPOINT", e))?)
    }
    /// 停止状态整体应用 TCP 或 RTU 通信配置。
    ///
    /// # Errors
    /// 正在运行或配置无效时返回错误。
    pub fn configure(&self, communication: Communication) -> Result<Snapshot> {
        self.0.configure(communication)
    }
    /// 启动 RTU 从站，成功时串口已打开。
    ///
    /// # Errors
    /// 参数、打开设备、重复启动或启动确认失败时返回错误。
    pub fn start_rtu(&self, endpoint: RtuEndpoint) -> Result<Snapshot> {
        self.0.start_rtu(endpoint)
    }
    pub fn stop(&self) -> Result<Snapshot> {
        self.0.stop()
    }
    pub fn update(&self, point_id: &str, raw: f64) -> Result<Snapshot> {
        self.update_unit(1, point_id, raw)
    }
    /// 添加独立教学单元；返回成功时，运行中的协议映射已创建完成。
    ///
    /// # Errors
    /// Unit ID 非法、重复或底层创建映射失败时返回错误。
    pub fn add_unit(&self, unit_id: u16) -> Result<Snapshot> {
        self.0
            .add_unit(UnitId::new(unit_id).map_err(|e| Error::new("INVALID_UNIT_ID", e))?)
    }
    /// 更新指定单元的 SOC，保持其他单元不变。
    ///
    /// # Errors
    /// 单元/点不存在、值非法或底层更新失败时返回错误。
    pub fn update_unit(&self, unit_id: u16, point_id: &str, raw: f64) -> Result<Snapshot> {
        if point_id != crate::domain::modbus::POINT_ID {
            return Err(Error::new("POINT_NOT_FOUND", "点不存在"));
        }
        self.0.update_unit(
            UnitId::new(unit_id).map_err(|e| Error::new("INVALID_UNIT_ID", e))?,
            Soc::new(raw).map_err(|e| Error::new("INVALID_VALUE", e))?,
        )
    }
    pub fn snapshot(&self) -> Snapshot {
        self.0.snapshot()
    }
    pub fn subscribe(&self) -> Receiver<Snapshot> {
        self.0.subscribe()
    }
}
