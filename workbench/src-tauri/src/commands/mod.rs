use essim_core::{
    application::simulator::{Error, SimulatorService, Snapshot},
    domain::modbus::{Communication, Endpoint, RtuEndpoint, State},
};
use essim_infrastructure::ModbusSimulator;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::{ipc::Channel, State as Managed};
pub type Engine = Arc<SimulatorService<ModbusSimulator>>;
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotDto {
    instance_id: String,
    point_id: String,
    sequence: String,
    generation: String,
    state: &'static str,
    address: String,
    port: u16,
    soc_raw: u16,
    soc_percent: f64,
    units: Vec<UnitDto>,
    communication: CommunicationDto,
    rtu: Option<RtuDto>,
    connections: u8,
    error: Option<ErrorDto>,
    logs: Vec<LogDto>,
}
/// IPC 串口配置；所有字段整组转换，核心再次校验。
#[derive(serde::Serialize, serde::Deserialize, Clone)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RtuDto {
    device: String,
    baud: u32,
    parity: char,
    data_bits: u8,
    stop_bits: u8,
    slave: u16,
    receive_timeout_ms: u32,
    byte_timeout_ms: u32,
}
impl TryFrom<RtuDto> for RtuEndpoint {
    type Error = Error;
    fn try_from(c: RtuDto) -> Result<Self, Error> {
        RtuEndpoint::new(
            &c.device,
            c.baud,
            c.parity,
            c.data_bits,
            c.stop_bits,
            c.slave,
            c.receive_timeout_ms,
            c.byte_timeout_ms,
        )
        .map_err(|e| Error::new("INVALID_RTU_CONFIG", e))
    }
}
impl From<RtuEndpoint> for RtuDto {
    fn from(c: RtuEndpoint) -> Self {
        Self {
            device: c.device,
            baud: c.baud,
            parity: c.parity,
            data_bits: c.data_bits,
            stop_bits: c.stop_bits,
            slave: c.slave.raw().into(),
            receive_timeout_ms: c.receive_timeout_ms,
            byte_timeout_ms: c.byte_timeout_ms,
        }
    }
}
/// 按 transport 区分配置，避免把串口路径塞入 TCP address/port。
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(tag = "transport", rename_all = "lowercase", deny_unknown_fields)]
pub enum CommunicationDto {
    Tcp { address: String, port: u16 },
    Rtu { config: RtuDto },
}
impl TryFrom<CommunicationDto> for Communication {
    type Error = Error;
    fn try_from(c: CommunicationDto) -> Result<Self, Error> {
        match c {
            CommunicationDto::Tcp { address, port } => Endpoint::new(&address, port)
                .map(Communication::Tcp)
                .map_err(|e| Error::new("INVALID_ENDPOINT", e)),
            CommunicationDto::Rtu { config } => {
                RtuEndpoint::try_from(config).map(Communication::Rtu)
            }
        }
    }
}
impl From<Communication> for CommunicationDto {
    fn from(c: Communication) -> Self {
        match c {
            Communication::Tcp(e) => Self::Tcp {
                address: e.address.to_string(),
                port: e.port,
            },
            Communication::Rtu(e) => Self::Rtu { config: e.into() },
        }
    }
}
/// 按 Unit ID 区分的运行值，前端据此选择单元。
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitDto {
    unit_id: u8,
    soc_raw: u16,
    soc_percent: f64,
}
#[derive(serde::Serialize)]
pub struct ErrorDto {
    code: String,
    message: String,
}
impl From<Error> for ErrorDto {
    fn from(e: Error) -> Self {
        Self {
            code: e.code.into(),
            message: e.message,
        }
    }
}
#[derive(serde::Serialize)]
pub struct LogDto {
    direction: &'static str,
    hex: String,
    message: String,
}
impl From<Snapshot> for SnapshotDto {
    fn from(s: Snapshot) -> Self {
        Self {
            instance_id: s.instance_id.into(),
            point_id: s.point_id.into(),
            sequence: s.sequence.to_string(),
            generation: s.generation.to_string(),
            state: match s.state {
                State::Stopped => "stopped",
                State::Starting => "starting",
                State::Listening => "listening",
                State::Stopping => "stopping",
                State::Failed => "failed",
            },
            address: s.endpoint.address.to_string(),
            port: s.endpoint.port,
            soc_raw: s.soc.raw(),
            soc_percent: s.soc.engineering(),
            communication: s.communication.clone().into(),
            rtu: s.rtu.map(Into::into),
            units: match &s.communication {
                Communication::Tcp(_) => s.units.into_iter().collect::<Vec<_>>(),
                Communication::Rtu(e) => vec![(e.slave, s.rtu_soc)],
            }
            .into_iter()
            .map(|(id, soc)| UnitDto {
                unit_id: id.raw(),
                soc_raw: soc.raw(),
                soc_percent: soc.engineering(),
            })
            .collect(),
            connections: s.connections,
            error: s.error.map(Into::into),
            logs: s
                .logs
                .into_iter()
                .map(|l| LogDto {
                    direction: l.direction,
                    hex: l.hex,
                    message: l.message,
                })
                .collect(),
        }
    }
}
fn validate(id: &str) -> Result<(), ErrorDto> {
    if id == "demo-bms" {
        Ok(())
    } else {
        Err(Error::new("INSTANCE_NOT_FOUND", "实例不存在").into())
    }
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendInfoDto {
    name: String,
    version: String,
    protocol_engine_ready: bool,
}
#[tauri::command]
pub fn get_backend_info() -> BackendInfoDto {
    let info = essim_core::application::backend_info();
    BackendInfoDto {
        name: info.name,
        version: info.version,
        protocol_engine_ready: true,
    }
}
#[tauri::command]
pub fn get_simulator_snapshot(
    instance_id: String,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    Ok(engine.snapshot().into())
}
async fn blocking(
    f: impl FnOnce() -> Result<Snapshot, Error> + Send + 'static,
) -> Result<SnapshotDto, ErrorDto> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| ErrorDto {
            code: "WORKER_FAILED".into(),
            message: e.to_string(),
        })?
        .map(Into::into)
        .map_err(Into::into)
}
#[tauri::command]
pub async fn start_simulator(
    instance_id: String,
    address: String,
    port: u16,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let e = engine.inner().clone();
    blocking(move || e.start(&address, port)).await
}
#[tauri::command]
pub async fn configure_simulator(
    instance_id: String,
    communication: CommunicationDto,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let c = Communication::try_from(communication).map_err(ErrorDto::from)?;
    let e = engine.inner().clone();
    blocking(move || e.configure(c)).await
}
#[tauri::command]
pub async fn start_rtu_simulator(
    instance_id: String,
    config: RtuDto,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let config = RtuEndpoint::try_from(config).map_err(ErrorDto::from)?;
    let e = engine.inner().clone();
    blocking(move || e.start_rtu(config)).await
}
#[tauri::command]
pub async fn stop_simulator(
    instance_id: String,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let e = engine.inner().clone();
    blocking(move || e.stop()).await
}
#[tauri::command]
pub async fn update_simulator_soc(
    instance_id: String,
    point_id: String,
    raw: f64,
    unit_id: Option<u16>,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let e = engine.inner().clone();
    blocking(move || e.update_unit(unit_id.unwrap_or(1), &point_id, raw)).await
}
#[tauri::command]
pub async fn add_simulator_unit(
    instance_id: String,
    unit_id: u16,
    engine: Managed<'_, Engine>,
) -> Result<SnapshotDto, ErrorDto> {
    validate(&instance_id)?;
    let e = engine.inner().clone();
    blocking(move || e.add_unit(unit_id)).await
}
struct Subscription {
    cancel: Arc<AtomicBool>,
    join: JoinHandle<()>,
}
#[derive(Default)]
pub struct Subscriptions {
    next: AtomicU64,
    items: Mutex<HashMap<String, Subscription>>,
}
impl Drop for Subscriptions {
    fn drop(&mut self) {
        for (_, s) in self.items.get_mut().unwrap().drain() {
            s.cancel.store(true, Ordering::Release);
            let _ = s.join.join();
        }
    }
}
#[tauri::command]
pub fn subscribe_simulator(
    instance_id: String,
    channel: Channel<SnapshotDto>,
    engine: Managed<'_, Engine>,
    subscriptions: Managed<'_, Subscriptions>,
) -> Result<String, ErrorDto> {
    validate(&instance_id)?;
    let mut items = subscriptions.items.lock().unwrap();
    items.retain(|_, s| !s.join.is_finished());
    if items.len() >= 8 {
        return Err(Error::new("SUBSCRIPTION_LIMIT", "最多允许 8 个订阅，请先关闭旧面板").into());
    }
    let id = subscriptions
        .next
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    let receiver = engine.subscribe();
    let engine = engine.inner().clone();
    let join = thread::spawn(move || {
        let mut last = None;
        while !flag.load(Ordering::Acquire) {
            match receiver.recv_timeout(Duration::from_millis(100)) {
                Ok(mut s) => {
                    while let Ok(new) = receiver.try_recv() {
                        s = new;
                    }
                    if channel.send(SnapshotDto::from(s.clone())).is_err() {
                        break;
                    }
                    last = Some(s.sequence);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                Err(_) => {}
            }
            // A full bounded queue may coalesce events. Refresh its complete snapshot so
            // the last state is delivered even if no further event follows overflow.
            let snapshot = engine.snapshot();
            if last != Some(snapshot.sequence) {
                if channel.send(SnapshotDto::from(snapshot.clone())).is_err() {
                    break;
                }
                last = Some(snapshot.sequence);
            }
        }
    });
    items.insert(id.clone(), Subscription { cancel, join });
    Ok(id)
}
#[tauri::command]
pub async fn unsubscribe_simulator(
    subscription_id: String,
    subscriptions: Managed<'_, Subscriptions>,
) -> Result<(), ErrorDto> {
    let item = subscriptions.items.lock().unwrap().remove(&subscription_id);
    if let Some(s) = item {
        s.cancel.store(true, Ordering::Release);
        tauri::async_runtime::spawn_blocking(move || s.join.join())
            .await
            .map_err(|e| ErrorDto {
                code: "UNSUBSCRIBE_FAILED".into(),
                message: e.to_string(),
            })?
            .map_err(|_| ErrorDto {
                code: "UNSUBSCRIBE_FAILED".into(),
                message: "订阅线程异常退出".into(),
            })?;
    }
    Ok(())
}
