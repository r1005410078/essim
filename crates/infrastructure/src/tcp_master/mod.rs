//! TCP／RTU主站共享适配器（保留既有模块路径）：独立连接所有者、有限手动队列和按需查询。
use crate::operation_gates::OperationPermit;
mod bridge;
mod journal;
mod rtu_bridge;
mod worker;
use essim_core::{
    application::{
        commands::master::{MasterAction, MasterCommand},
        events::master::MasterFact,
        queries::{
            master::{MasterPoint, MasterQuery, MasterView},
            slave::SlaveObservation,
        },
    },
    domain::{
        endpoint::EndpointConfig,
        error::Error,
        identity::{EndpointId, PointId, UnitId},
        values::UnitValues,
    },
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
/// 自包含已应用主站配置，不读取草稿。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MasterSnapshot {
    /// 已应用端点。
    pub endpoint: EndpointConfig,
    /// 已应用模型。
    pub values: BTreeMap<UnitId, UnitValues>,
}
pub(super) struct Projection {
    publish_gate: Arc<Mutex<()>>,
    view: MasterView,
    points: BTreeMap<UnitId, BTreeMap<PointId, MasterPoint>>,
    sequence: u64,
    journal: Option<journal::Journal>,
    record_failed: bool,
    disconnect_pending: bool,
    snapshot: Option<MasterSnapshot>,
    release_permit: Option<OperationPermit>,
    memory_charge: Option<crate::resources::Reservation>,
    active_charge: Option<crate::resources::Reservation>,
}
pub(super) type Shared = Arc<Mutex<Projection>>;
pub(super) fn failure(e: impl std::fmt::Display) -> Error {
    Error::new("MASTER_RUNTIME", e.to_string())
}
pub(super) fn now() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .to_string()
}
pub(super) fn publish(shared: &Shared, fact: MasterFact) {
    // 主站所有者与wire观测线程共享文件，按端点串行写；查询不持有此锁。
    let gate = match shared.lock() {
        Ok(p) => p.publish_gate.clone(),
        Err(_) => return,
    };
    let Ok(_writer) = gate.lock() else {
        if let Ok(mut p) = shared.lock() {
            p.record_failed = true;
        }
        return;
    };
    let (sequence, unix_ms, mut journal) = {
        let Ok(mut p) = shared.lock() else {
            return;
        };
        if p.record_failed {
            return;
        }
        (p.sequence + 1, now(), p.journal.take())
    };
    let result = if let Some(j) = &mut journal {
        j.write(&serde_json::json!({"sequence":sequence.to_string(),"unixMs":unix_ms,"fact":fact}))
    } else {
        Ok(())
    };
    if let Ok(mut p) = shared.lock() {
        p.journal = journal;
        if let Err(e) = result {
            p.record_failed = true;
            p.view.error = Some(e.message);
            return;
        }
        p.sequence = sequence;
        p.view.revision = sequence.to_string();
        p.view.observations.push(SlaveObservation {
            sequence: sequence.to_string(),
            unix_ms,
            event: serde_json::to_string(&fact).unwrap_or_default(),
        });
        if p.view.observations.len() > 100 {
            p.view.observations.remove(0);
        }
    }
}

fn empty(id: EndpointId) -> Projection {
    Projection {
        publish_gate: Default::default(),
        view: MasterView {
            diagnostic: None,
            endpoint: id,
            state: "disconnected".into(),
            polling: false,
            paused_units: vec![],
            busy: false,
            operation: None,
            completed: 0,
            total: 0,
            requests: 0,
            error: None,
            revision: "0".into(),
            points: BTreeMap::new(),
            observations: vec![],
            record_path: None,
        },
        points: BTreeMap::new(),
        sequence: 0,
        journal: None,
        record_failed: false,
        disconnect_pending: false,
        snapshot: None,
        release_permit: None,
        memory_charge: None,
        active_charge: None,
    }
}
pub(super) struct Job {
    command: MasterCommand,
    snapshot: Option<MasterSnapshot>,
    permit: Option<OperationPermit>,
    memory_charge: Option<crate::resources::Reservation>,
    active_charge: Option<crate::resources::Reservation>,
}
struct Owner {
    tx: SyncSender<Job>,
    shared: Shared,
    cancel: Arc<AtomicBool>,
    disconnect: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
/// TCP／RTU合计最多32个独立主站所有者，每个手动命令队列容量1；查询不等待网络。
pub struct TcpMasterRuntime {
    recording: Result<crate::recording::RecordingPool, Error>,
    resources: crate::resources::ResourceManager,
    owners: Mutex<BTreeMap<EndpointId, Owner>>,
    root: std::path::PathBuf,
}
impl TcpMasterRuntime {
    /// 指定可靠运行记录目录，连接前建立自包含快照。
    pub fn new(root: std::path::PathBuf) -> Self {
        Self::with_resources(root, Default::default())
    }
    /// 共享会话资源账本，32主站所有者边界仍独立生效。
    pub fn with_resources(
        root: std::path::PathBuf,
        resources: crate::resources::ResourceManager,
    ) -> Self {
        let recording = crate::recording::RecordingPool::new(resources.clone());
        Self {
            recording,
            resources,
            owners: Mutex::new(BTreeMap::new()),
            root,
        }
    }
    /// 装配会话共享录制池，不增加每端点录制线程。
    pub fn with_recording(
        root: std::path::PathBuf,
        resources: crate::resources::ResourceManager,
        recording: crate::recording::RecordingPool,
    ) -> Self {
        Self {
            root,
            resources,
            recording: Ok(recording),
            owners: Default::default(),
        }
    }
    /// 提交已应用主站操作；立即返回执行投影，最终结果通过查询获取。
    /// # Errors
    /// 命令非法、未应用、非TCP主站、32槽耗尽或已有未完成手动操作。
    pub fn execute(
        &self,
        command: MasterCommand,
        snapshot: Option<MasterSnapshot>,
    ) -> Result<MasterView, Error> {
        self.execute_permitted(command, snapshot, None)
    }
    pub(crate) fn execute_permitted(
        &self,
        command: MasterCommand,
        snapshot: Option<MasterSnapshot>,
        parent: Option<&OperationPermit>,
    ) -> Result<MasterView, Error> {
        if command.command_id.trim().is_empty()
            || serde_json::to_vec(&command).map_err(failure)?.len() > 8 * 1024 * 1024
        {
            return Err(Error::new("INVALID_COMMAND", "主站命令身份为空或载荷过大"));
        }
        if matches!(&command.action,MasterAction::Write{values,..} if values.len()>1024) {
            return Err(Error::new("BATCH_LIMIT", "单批主站下发最多1024个完整点"));
        }
        let memory_charge = snapshot
            .as_ref()
            .map(|s| {
                self.resources
                    .reserve(essim_core::domain::resources::ResourceUsage {
                        memory_bytes: crate::resources::serialized_size(s)?.saturating_mul(8),
                        ..Default::default()
                    })
            })
            .transpose()?;
        let active_charge = if matches!(command.action, MasterAction::Connect) {
            Some(
                self.resources
                    .reserve(essim_core::domain::resources::ResourceUsage {
                        endpoints: 1,
                        connections: 1,
                        ..Default::default()
                    })?,
            )
        } else {
            None
        };
        let id = command.endpoint.clone();
        let mut owners = self.owners.lock().map_err(failure)?;
        if !owners.contains_key(&id) {
            if !matches!(command.action, MasterAction::Connect) {
                return Ok(empty(id).view);
            }
            let s = snapshot
                .as_ref()
                .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用主站配置"))?;
            worker::validate(s)?;
            if owners.len() >= 32 {
                return Err(Error::new(
                    "MASTER_LIMIT",
                    "首版最多32个主站运行槽，请删除不用的实例释放槽位",
                ));
            }
            let shared = Arc::new(Mutex::new(empty(id.clone())));
            let cancel = Arc::new(AtomicBool::new(false));
            let disconnect = Arc::new(AtomicBool::new(false));
            let shutdown = Arc::new(AtomicBool::new(false));
            let (tx, rx) = mpsc::sync_channel(1);
            let root = self.root.clone();
            let recording = self.recording.clone()?;
            let (state, c, d, stop) = (
                shared.clone(),
                cancel.clone(),
                disconnect.clone(),
                shutdown.clone(),
            );
            let join = std::thread::Builder::new()
                .name("modbus-master".into())
                .spawn(move || worker::run(rx, state, c, d, stop, root, recording))
                .map_err(failure)?;
            owners.insert(
                id.clone(),
                Owner {
                    tx,
                    shared,
                    cancel,
                    disconnect,
                    shutdown,
                    join: Some(join),
                },
            );
        }
        let owner = owners.get(&id).unwrap();
        let control = matches!(
            command.action,
            MasterAction::StopPolling | MasterAction::Disconnect
        );
        if control {
            owner.cancel.store(true, Ordering::Release);
            if matches!(command.action, MasterAction::Disconnect) {
                owner.disconnect.store(true, Ordering::Release);
            }
            {
                let mut p = owner.shared.lock().map_err(failure)?;
                p.view.polling = false;
                if matches!(command.action, MasterAction::Disconnect) {
                    p.disconnect_pending = true;
                }
            }
            return Ok(owner.shared.lock().map_err(failure)?.view.clone());
        }
        if let Some(s) = &snapshot {
            if s.endpoint.id != id {
                return Err(Error::new("ENDPOINT_MISMATCH", "主站快照身份不匹配"));
            }
            worker::validate(s)?;
        }
        let mut p = owner.shared.lock().map_err(failure)?;
        if p.view.busy || p.disconnect_pending {
            return Err(Error::new(
                "MASTER_BUSY",
                "当前连接已有手动操作，请等待结果",
            ));
        }
        p.view.busy = true;
        p.view.operation = Some(command.command_id.clone());
        p.view.completed = 0;
        p.view.total = 0;
        owner.cancel.store(false, Ordering::Release);
        owner.disconnect.store(false, Ordering::Release);
        if let Err(e) = owner.tx.try_send(Job {
            command,
            snapshot,
            permit: parent.map(OperationPermit::retain_callback).transpose()?,
            memory_charge,
            active_charge,
        }) {
            p.view.busy = false;
            return Err(failure(e));
        }
        Ok(p.view.clone())
    }
    /// 读取指定Unit最多1024点；返回真实最近成功值及质量。
    /// # Errors
    /// 查询过大、未知Unit或点、内部锁中毒。
    pub fn query(&self, q: MasterQuery) -> Result<MasterView, Error> {
        if q.points.len() > 1024 {
            return Err(Error::new("QUERY_LIMIT", "主站查询最多1024点"));
        }
        let owners = self.owners.lock().map_err(failure)?;
        let Some(owner) = owners.get(&q.endpoint) else {
            return Ok(empty(q.endpoint).view);
        };
        let p = owner.shared.lock().map_err(failure)?;
        let mut view = p.view.clone();
        view.busy |= p.disconnect_pending;
        if let Some(unit) = q.unit {
            if !q.points.is_empty() {
                if p.view.busy && !p.points.contains_key(&unit) {
                    return Ok(view);
                }
                let points = p
                    .points
                    .get(&unit)
                    .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "运行单元不存在"))?;
                for id in q.points {
                    view.points.insert(
                        id.clone(),
                        points
                            .get(&id)
                            .ok_or_else(|| Error::new("POINT_NOT_FOUND", "运行点不存在"))?
                            .clone(),
                    );
                }
            }
        }
        Ok(view)
    }
    /// 取消未发出请求并等待实际连接释放；用于应用／删除，不持有配置锁。
    /// # Errors
    /// 在最大连接／请求等待后仍不能确认释放时拒绝配置切换。
    pub fn disconnect_wait(&self, id: &EndpointId) -> Result<(), Error> {
        self.disconnect_wait_permitted(id, None)
    }
    pub(crate) fn disconnect_wait_permitted(
        &self,
        id: &EndpointId,
        parent: Option<&OperationPermit>,
    ) -> Result<(), Error> {
        {
            let owners = self.owners.lock().map_err(failure)?;
            let Some(o) = owners.get(id) else {
                return Ok(());
            };
            let mut projection = o.shared.lock().map_err(failure)?;
            if projection.release_permit.is_none() {
                projection.release_permit =
                    parent.map(OperationPermit::retain_callback).transpose()?;
            }
            projection.view.polling = false;
            projection.disconnect_pending = true;
            o.cancel.store(true, Ordering::Release);
            o.disconnect.store(true, Ordering::Release);
        }
        let until = Instant::now() + Duration::from_secs(65);
        loop {
            let view = self.query(MasterQuery {
                endpoint: id.clone(),
                unit: None,
                points: vec![],
            })?;
            if view.state == "disconnected" && !view.busy {
                return Ok(());
            }
            if Instant::now() > until {
                return Err(Error::new("MASTER_RELEASE_TIMEOUT", "主站连接释放尚未确认"));
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
    /// 释放后导出真实成功缓存，供配置应用预览及原子迁移使用。
    /// # Errors
    /// 连接未释放或内部锁中毒。
    pub fn stopped_values(
        &self,
        id: &EndpointId,
    ) -> Result<Option<BTreeMap<UnitId, UnitValues>>, Error> {
        let owners = self.owners.lock().map_err(failure)?;
        let Some(o) = owners.get(id) else {
            return Ok(None);
        };
        let p = o.shared.lock().map_err(failure)?;
        if p.view.state != "disconnected" || p.view.busy {
            return Err(Error::new("MASTER_BUSY", "主站尚未确认断开"));
        }
        Ok(p.snapshot.as_ref().map(|s| {
            s.values
                .iter()
                .map(|(unit, v)| {
                    let raw = p
                        .points
                        .get(unit)
                        .map(|points| {
                            points
                                .iter()
                                .filter_map(|(id, p)| p.value.clone().map(|raw| (id.clone(), raw)))
                                .collect()
                        })
                        .unwrap_or_default();
                    (
                        unit.clone(),
                        UnitValues {
                            model: v.model.clone(),
                            raw,
                            stale: true,
                        },
                    )
                })
                .collect()
        }))
    }
    /// 已确认断开后迁移缓存到新快照；不建立连接、不启动采集。
    /// # Errors
    /// 快照无效、尚未断开或队列不可用。
    pub fn reconfigure(&self, snapshot: MasterSnapshot) -> Result<(), Error> {
        self.reconfigure_permitted(snapshot, None)
    }
    pub(crate) fn reconfigure_permitted(
        &self,
        snapshot: MasterSnapshot,
        parent: Option<&OperationPermit>,
    ) -> Result<(), Error> {
        validate_snapshot(&snapshot)?;
        let owners = self.owners.lock().map_err(failure)?;
        let Some(o) = owners.get(&snapshot.endpoint.id) else {
            return Ok(());
        };
        let mut p = o.shared.lock().map_err(failure)?;
        if p.view.busy || p.disconnect_pending || p.view.state != "disconnected" {
            return Err(Error::new("MASTER_BUSY", "配置替换前必须断开"));
        }
        o.cancel.store(false, Ordering::Release);
        o.disconnect.store(false, Ordering::Release);
        let permit = parent.map(OperationPermit::retain_callback).transpose()?;
        let memory_charge = Some(self.resources.reserve(
            essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::serialized_size(&snapshot)?.saturating_mul(8),
                ..Default::default()
            },
        )?);
        p.view.busy = true;
        if let Err(error) = o.tx.try_send(Job {
            command: MasterCommand {
                command_id: "apply-master".into(),
                endpoint: snapshot.endpoint.id.clone(),
                action: MasterAction::Disconnect,
            },
            snapshot: Some(snapshot),
            permit,
            memory_charge,
            active_charge: None,
        }) {
            p.view.busy = false;
            return Err(failure(error));
        }
        Ok(())
    }
    /// 删除端点后释放所有者槽位；必须先确认断开。
    /// # Errors
    /// 释放失败或锁中毒。
    pub fn forget(&self, id: &EndpointId) -> Result<(), Error> {
        self.forget_permitted(id, None)
    }
    pub(crate) fn forget_permitted(
        &self,
        id: &EndpointId,
        parent: Option<&OperationPermit>,
    ) -> Result<(), Error> {
        self.disconnect_wait_permitted(id, parent)?;
        let owner = self.owners.lock().map_err(failure)?.remove(id);
        if let Some(mut o) = owner {
            o.shutdown.store(true, Ordering::Release);
            if let Some(j) = o.join.take() {
                let _ = j.join();
            }
        }
        Ok(())
    }
    /// 回收全部主站线程和连接。
    pub fn shutdown(&self) {
        if let Ok(mut owners) = self.owners.lock() {
            for o in owners.values() {
                o.cancel.store(true, Ordering::Release);
                o.shutdown.store(true, Ordering::Release);
            }
            for o in owners.values_mut() {
                if let Some(j) = o.join.take() {
                    let _ = j.join();
                }
            }
            owners.clear();
        }
    }
}
impl Drop for TcpMasterRuntime {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn validate_snapshot(s: &MasterSnapshot) -> Result<(), Error> {
    worker::validate(s)
}

fn release_completed(shared: &Shared) {
    let permit = shared.lock().ok().and_then(|mut p| {
        if p.view.state == "disconnected" && !p.view.busy && !p.disconnect_pending {
            p.release_permit.take()
        } else {
            None
        }
    });
    drop(permit);
}

#[cfg(test)]
mod release_gate_tests {
    use super::*;
    use crate::operation_gates::OperationGates;

    #[test]
    fn release_callback_keeps_gate_until_disconnected_and_idle() {
        let gates = OperationGates::default();
        let id = EndpointId("master".into());
        let parent = gates.acquire(&id).unwrap();
        let mut projection = empty(id.clone());
        projection.view.state = "connected".into();
        projection.view.busy = true;
        projection.release_permit = Some(parent.retain_callback().unwrap());
        let shared = Arc::new(Mutex::new(projection));
        drop(parent);
        release_completed(&shared);
        assert_eq!(gates.acquire(&id).err().unwrap().code, "ENDPOINT_BUSY");
        shared.lock().unwrap().view.state = "disconnected".into();
        release_completed(&shared);
        assert!(gates.acquire(&id).is_err());
        shared.lock().unwrap().view.busy = false;
        release_completed(&shared);
        assert!(gates.acquire(&id).is_ok());
    }
}
