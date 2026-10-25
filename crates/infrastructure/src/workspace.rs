//! 会话配置适配器；可选装配真实TCP从站，配置目录仍不持久化。
use crate::operation_gates::{OperationGates, OperationPermit};
use essim_core::{
    application::{
        catalog::Catalog,
        commands::{Action, Command, Receipt},
        events::ConfigEvent,
        ports::{ApplyBoundary, ConfigRepository, StoppedBoundary},
        queries::{inspect_endpoint, ApplyPreview, WorkspaceView},
        service::Workspace,
    },
    domain::{
        endpoint::EndpointConfig,
        error::Error,
        identity::{EndpointId, UnitId},
    },
};
use std::sync::Mutex;

/// 本次应用会话的内存配置目录，提交替换整体值。
#[derive(Default)]
pub struct MemoryRepository {
    catalog: Catalog,
    resources: crate::resources::ResourceManager,
    charge: Option<crate::resources::Reservation>,
}
impl MemoryRepository {
    pub(crate) fn with_resources(resources: crate::resources::ResourceManager) -> Self {
        Self {
            resources,
            ..Default::default()
        }
    }
}
impl ConfigRepository for MemoryRepository {
    fn reserve_staging(&self) -> Result<Box<dyn Send>, Error> {
        Ok(Box::new(self.resources.reserve(
            essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::catalog_memory(&self.catalog)?,
                ..Default::default()
            },
        )?))
    }

    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error> {
        let charge = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::catalog_memory(&catalog)?,
                ..Default::default()
            })?;
        charge.commit();
        self.catalog = catalog;
        self.charge = Some(charge);
        Ok(())
    }
}
/// 应用组装使用的线程安全入口；临界区只处理短配置操作。
pub struct WorkspaceEngine {
    pub(crate) workspace: Mutex<Workspace<Box<dyn ConfigRepository + Send>>>,
    pub(crate) partition_plans: crate::partition::Plans,
    pub(crate) resources: crate::resources::ResourceManager,
    operations: OperationGates,
    pub(crate) project_gate: std::sync::RwLock<()>,
    pub(crate) project_root: Mutex<Option<crate::project::ProjectInfo>>,
    pub(crate) project_required: std::sync::atomic::AtomicBool,
    pub(crate) project_clients: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    pub(crate) boundary: Mutex<Box<dyn ApplyBoundary + Send>>,
    #[cfg(feature = "tcp-runtime")]
    pub(crate) runtime: Option<std::sync::Arc<crate::tcp_slave::TcpSlaveRuntime>>,
    #[cfg(feature = "tcp-runtime")]
    pub(crate) master: Option<crate::tcp_master::TcpMasterRuntime>,
}
impl Default for WorkspaceEngine {
    fn default() -> Self {
        let resources = crate::resources::ResourceManager::default();
        Self {
            workspace: Mutex::new(Workspace::new(Box::new(MemoryRepository {
                resources: resources.clone(),
                ..Default::default()
            }))),
            partition_plans: Default::default(),
            resources,
            operations: Default::default(),
            project_root: Mutex::new(None),
            project_required: std::sync::atomic::AtomicBool::new(false),
            project_clients: Default::default(),
            project_gate: std::sync::RwLock::new(()),
            boundary: Mutex::new(Box::new(StoppedBoundary)),
            #[cfg(feature = "tcp-runtime")]
            runtime: None,
            #[cfg(feature = "tcp-runtime")]
            master: None,
        }
    }
}
impl WorkspaceEngine {
    /// 读取工程额度与本进程RSS采样；不复制模型和值。
    /// # Errors
    /// 资源或目录状态不可用。
    pub fn query_resources(&self) -> Result<crate::resources::ResourceView, Error> {
        self.ensure_project()?;
        let mut view = self.resources.query()?;
        #[cfg(feature = "tcp-runtime")]
        if let Some(runtime) = &self.runtime {
            view.recording = Some(runtime.recording_view()?);
            view.runtime_data = runtime.project_recording_view();
        }
        view.configured_endpoints = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?
            .catalog_ref()
            .endpoints
            .len();
        view.process_rss_bytes = std::process::Command::new("/bin/ps")
            .args(["-o", "rss=", "-p", &std::process::id().to_string()])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .and_then(|s| s.trim().parse::<u64>().ok())
            .and_then(|v| v.checked_mul(1024));
        Ok(view)
    }
    /// 获取同一会话的公共操作门，供应用级固定执行器装配。
    pub fn operation_gates(&self) -> OperationGates {
        self.operations.clone()
    }
    /// 在已取得操作凭证后复核固定目标，不重新解析范围。
    /// # Errors
    /// 凭证不匹配、目标删除或相关定义变化。
    pub fn revalidate_batch(
        &self,
        plan: &essim_core::application::commands::batch_targets::BatchPlan,
        id: &EndpointId,
        permit: &OperationPermit,
    ) -> Result<(), Error> {
        self.ensure_project()?;
        let _child = self.operations.enter(id, Some(permit))?;
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?
            .revalidate_batch(plan, id)
    }

    /// 装配窄资源释放边界；不改变生产执行能力门禁。
    pub fn with_boundary(boundary: Box<dyn ApplyBoundary + Send>) -> Self {
        Self {
            boundary: Mutex::new(boundary),
            ..Self::default()
        }
    }
    /// 仅获取配置通知位置；空闲轮询不克隆大型点表。
    /// # Errors
    /// 内部状态锁中毒时失败。
    pub fn sequence(&self) -> Result<u64, Error> {
        self.ensure_project()?;
        Ok(self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .sequence())
    }
    /// 获取真实目录和能力快照，不触发远端采集。
    /// # Errors
    /// 内部状态锁中毒时返回明确故障。
    pub fn query(&self) -> Result<WorkspaceView, Error> {
        self.ensure_project()?;
        #[allow(unused_mut)]
        let mut view = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .query();
        #[cfg(feature = "tcp-runtime")]
        if self.runtime.is_some() {
            view.capabilities.execution = true;
            view.capabilities.recording = true;
            view.capabilities.reason = "当前支持TCP／RTU主从站运行与文件录制".into();
        }
        Ok(view)
    }
    /// 导航摘要不复制任何模型点或运行值；内容通过按需接口读取。
    /// # Errors
    /// 目录锁不可用。
    pub fn query_navigation(&self) -> Result<WorkspaceView, Error> {
        self.ensure_project()?;
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?;
        let c = w.catalog_ref();
        #[allow(unused_mut)]
        let mut view = WorkspaceView {
            sequence: w.sequence(),
            application_matches: essim_core::application::queries::inspect_endpoint_matches(c),
            application_states: c
                .endpoints
                .iter()
                .filter_map(|e| w.application_state(&e.id).map(|s| (e.id.clone(), s)))
                .collect(),
            capabilities: Default::default(),
            catalog: Catalog {
                models: c
                    .models
                    .iter()
                    .map(|m| essim_core::domain::model::CollectionModel {
                        id: m.id.clone(),
                        name: m.name.clone(),
                        points: vec![],
                    })
                    .collect(),
                endpoints: c.endpoints.clone(),
                devices: c.devices.clone(),
                applied: c.applied.clone(),
                navigation: c.navigation.clone(),
                values: Default::default(),
            },
        };
        #[cfg(feature = "tcp-runtime")]
        if self.runtime.is_some() {
            view.capabilities.execution = true;
            view.capabilities.recording = true;
            view.capabilities.reason = "TCP／RTU主从站；目录按需读取".into();
        }
        Ok(view)
    }
    /// 按需读取单个保存模型，不复制其他模型或运行值。
    /// # Errors
    /// 模型缺失或锁不可用。
    pub fn load_model(
        &self,
        id: &essim_core::domain::identity::ModelId,
    ) -> Result<essim_core::domain::model::CollectionModel, Error> {
        self.ensure_project()?;
        Ok(self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?
            .saved_model(id)?
            .clone())
    }
    /// 按需读取单端点的应用模型与静止值；真实运行值继续使用有界查询。
    /// # Errors
    /// 未应用或目录锁不可用。
    pub fn load_endpoint_values(
        &self,
        id: &EndpointId,
    ) -> Result<std::collections::BTreeMap<UnitId, essim_core::domain::values::UnitValues>, Error>
    {
        self.ensure_project()?;
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?
            .applied_values(id)
            .map(|v| v.1)
    }
    /// 比较草稿是否仍对应已应用映射，不提交目录或运行值。
    /// # Errors
    /// 配置状态锁中毒时失败。
    pub fn inspect_endpoint(
        &self,
        endpoint: EndpointConfig,
    ) -> Result<std::collections::BTreeMap<UnitId, bool>, Error> {
        self.ensure_project()?;
        let workspace = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
        Ok(inspect_endpoint(workspace.catalog_ref(), &endpoint))
    }
    /// 读取完整端点迁移预览，保持无副作用。
    /// # Errors
    /// 锁中毒、配置非法或迁移不兼容时失败。
    pub fn preview_apply(&self, id: EndpointId, reset: bool) -> Result<ApplyPreview, Error> {
        self.ensure_project()?;
        let workspace = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
        let _staging = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::effective_staging_memory(
                    workspace.catalog_ref(),
                    &id,
                )?,
                ..Default::default()
            })?;
        workspace.preview_apply(&id, reset)
    }
    /// 唯一配置修改入口，生产运行能力未接入时明确拒绝。
    /// # Errors
    /// 配置校验、未接入能力或状态锁故障。
    pub fn execute(&self, command: Command) -> Result<Receipt, Error> {
        self.execute_with_event(command).map(|(receipt, _)| receipt)
    }
    /// 返回已提交的具体配置事实及快照；调用者自行消费，不承诺可靠投递或持久化。
    /// # Errors
    /// 校验、释放或提交失败时只返回错误，不返回成功事件。
    pub fn execute_with_event(&self, command: Command) -> Result<(Receipt, ConfigEvent), Error> {
        self.execute_with_event_inner(command, None, None)
    }
    /// 同一批量操作凭证内提交配置；外部IPC不能构造凭证。
    /// # Errors
    /// 凭证不匹配、配置校验或真实资源释放失败。
    pub fn execute_with_event_permitted(
        &self,
        command: Command,
        permit: &OperationPermit,
    ) -> Result<(Receipt, ConfigEvent), Error> {
        self.execute_with_event_inner(command, Some(permit), None)
    }
    /// 在同一目录临界区复核固定计划并准备应用；不自动保存草稿。
    /// # Errors
    /// 凭证、内容复核、校验或实际释放失败。
    pub fn apply_batch_target(
        &self,
        plan: &essim_core::application::commands::batch_targets::BatchPlan,
        id: &EndpointId,
        permit: &OperationPermit,
    ) -> Result<Receipt, Error> {
        self.execute_with_event_inner(
            Command {
                command_id: format!("{}:{}", plan.preview().id, id.0),
                action: Action::Apply {
                    id: id.clone(),
                    reset_incompatible: false,
                },
            },
            Some(permit),
            Some(plan),
        )
        .map(|v| v.0)
    }
    fn execute_with_event_inner(
        &self,
        command: Command,
        parent: Option<&OperationPermit>,
        batch: Option<&essim_core::application::commands::batch_targets::BatchPlan>,
    ) -> Result<(Receipt, ConfigEvent), Error> {
        let _project = self.project_operation()?;
        // 新命令及每个 Unit 的有效表都必须先预留暂存空间，失败不进入停止／提交边界。
        let _configuration_staging = {
            let workspace = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
            let memory_bytes = crate::resources::serialized_size(&command)?
                .saturating_mul(4)
                .saturating_add(match &command.action {
                    Action::Apply { id, .. } => {
                        crate::resources::effective_staging_memory(workspace.catalog_ref(), id)?
                    }
                    _ => 0,
                });
            self.resources
                .reserve(essim_core::domain::resources::ResourceUsage {
                    memory_bytes,
                    ..Default::default()
                })?
        };
        let target = match &command.action {
            Action::Apply { id, .. }
            | Action::DeleteEndpoint { id }
            | Action::ResetUnit { id, .. }
            | Action::Start { id }
            | Action::ReadRemote { id }
            | Action::WriteRemote { id } => Some(id.clone()),
            _ => None,
        };
        if parent.is_some() && target.is_none() {
            return Err(Error::new(
                "OPERATION_PERMIT_MISMATCH",
                "本命令没有与凭证对应的端点",
            ));
        }
        let _operation = target
            .as_ref()
            .map(|id| self.operations.enter(id, parent))
            .transpose()?;
        #[cfg(feature = "tcp-runtime")]
        let parent = _operation.as_ref();

        #[cfg(feature = "tcp-runtime")]
        if let (Some(runtime), Action::ResetUnit { id, .. }) = (&self.runtime, &command.action) {
            let id = id.clone();
            let gate = runtime.gate(&id)?;
            let _guard = gate
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))?;
            if let Some(master) = &self.master {
                master.disconnect_wait_permitted(&id, parent)?;
            }
            if let Some(values) = runtime.stopped_values(&id)? {
                self.workspace
                    .lock()
                    .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                    .retain_stopped_values(&id, values)?;
            }
            let result = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .execute(command);
            if result.is_ok() {
                runtime.forget_with_permit(&id, parent)?;
                if let Some(master) = &self.master {
                    master.forget(&id)?;
                }
            }
            return result;
        }
        if matches!(
            &command.action,
            Action::Apply { .. } | Action::DeleteEndpoint { .. }
        ) {
            #[cfg(feature = "tcp-runtime")]
            let gate = if let Some(runtime) = &self.runtime {
                let id = match &command.action {
                    Action::Apply { id, .. } | Action::DeleteEndpoint { id } => id,
                    _ => unreachable!(),
                };
                Some(runtime.gate(id)?)
            } else {
                None
            };
            #[cfg(feature = "tcp-runtime")]
            let _guard = gate
                .as_ref()
                .map(|gate| {
                    gate.lock()
                        .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))
                })
                .transpose()?;
            let plan = {
                let mut workspace = self
                    .workspace
                    .lock()
                    .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
                if let Some(batch) = batch {
                    let id = target
                        .as_ref()
                        .ok_or_else(|| Error::new("INVALID_BATCH", "缺少端点"))?;
                    workspace.revalidate_batch(batch, id)?;
                }
                workspace.prepare_release(command.clone())?
            };
            #[allow(unused_mut)]
            let mut stopped_values = None;
            #[cfg(feature = "tcp-runtime")]
            if let Some(master) = &self.master {
                master.disconnect_wait_permitted(&plan.endpoint().id, parent)?;
                stopped_values = master.stopped_values(&plan.endpoint().id)?;
            }
            #[cfg(feature = "tcp-runtime")]
            if let Some(runtime) = &self.runtime {
                let id = plan.endpoint().id.clone();
                if let Err(error) = runtime.execute_with_permit(
                    essim_core::application::commands::slave::SlaveCommand {
                        command_id: command.command_id.clone(),
                        endpoint: id.clone(),
                        action: essim_core::application::commands::slave::SlaveAction::Stop,
                    },
                    None,
                    parent,
                ) {
                    self.workspace
                        .lock()
                        .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                        .observe_release(id, essim_core::application::ports::ApplyState::Failed);
                    return Err(error);
                }
                if let Some(values) = runtime.stopped_values(&id)? {
                    stopped_values = Some(values);
                }
            }
            let id = plan.endpoint().id.clone();
            // 等待只持有边界锁；目录查询和其他端点保存可以继续。
            let mut boundary = self
                .boundary
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "资源边界不可用"))?;
            let result = boundary.stop(plan.endpoint());
            let state = boundary.state(&id);
            drop(boundary);
            let mut workspace = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
            if let Err(error) = result {
                workspace.observe_release(id.clone(), state);
                return Err(error);
            }
            #[cfg(feature = "tcp-runtime")]
            let deleting = matches!(&command.action, Action::DeleteEndpoint { .. });
            let result = if let Some(values) = stopped_values {
                workspace.finish_release_with_values(plan, values)
            } else {
                workspace.finish_release(plan)
            };
            drop(workspace);
            #[cfg(feature = "tcp-runtime")]
            if result.is_ok() {
                if let Some(runtime) = &self.runtime {
                    runtime.forget_with_permit(&id, parent)?;
                }
                if let Some(master) = &self.master {
                    if deleting {
                        master.forget(&id)?;
                    } else {
                        let (endpoint, values) = self
                            .workspace
                            .lock()
                            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                            .applied_values(&id)?;
                        if endpoint.role == essim_core::domain::endpoint::Role::Master {
                            master.reconfigure_permitted(
                                crate::tcp_master::MasterSnapshot { endpoint, values },
                                parent,
                            )?;
                        }
                    }
                }
            }
            return result;
        }
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .execute(command)
    }
}

#[cfg(feature = "tcp-runtime")]
impl WorkspaceEngine {
    /// 装配真实TCP从站服务；配置仍仅保留于会话内，运行记录写入指定目录。
    /// # Errors
    /// 工作线程或I/O轮询器创建失败。
    pub fn with_tcp_runtime(record_root: std::path::PathBuf) -> Result<Self, Error> {
        let operations = OperationGates::default();
        let resources = crate::resources::ResourceManager::default();
        Self::with_runtime_resources(record_root, operations, resources)
    }
    /// 注入同一资源账本供无界面运行和预算边界验证。
    /// # Errors
    /// 协议所有者创建失败。
    pub fn with_runtime_resources(
        record_root: std::path::PathBuf,
        operations: OperationGates,
        resources: crate::resources::ResourceManager,
    ) -> Result<Self, Error> {
        let recording = crate::recording::RecordingPool::new(resources.clone())?;
        Self::with_runtime_recording(record_root, operations, resources, recording)
    }
    /// 注入同一可靠文件执行资源，用于协议所有者的磁盘故障验证。
    /// # Errors
    /// 协议所有者创建失败。
    pub fn with_runtime_recording(
        record_root: std::path::PathBuf,
        operations: OperationGates,
        resources: crate::resources::ResourceManager,
        recording: crate::recording::RecordingPool,
    ) -> Result<Self, Error> {
        Ok(Self {
            master: Some(crate::tcp_master::TcpMasterRuntime::with_recording(
                record_root.join("masters"),
                resources.clone(),
                recording.clone(),
            )),
            operations: operations.clone(),
            runtime: Some(std::sync::Arc::new(
                crate::tcp_slave::TcpSlaveRuntime::with_recording(
                    record_root,
                    operations,
                    resources.clone(),
                    recording,
                )?,
            )),
            workspace: Mutex::new(Workspace::new(Box::new(MemoryRepository {
                resources: resources.clone(),
                ..Default::default()
            }))),
            resources,
            ..Self::default()
        })
    }
    fn applied_runtime_snapshot(
        &self,
        id: &EndpointId,
    ) -> Result<(crate::tcp_slave::Snapshot, crate::resources::Reservation), Error> {
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?;
        w.ensure_repository_available()?;
        let c = w.catalog_ref();
        let endpoint = c
            .applied
            .iter()
            .find(|e| &e.id == id)
            .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用配置"))?;
        let mut bytes = crate::resources::serialized_size(endpoint)?.saturating_mul(4);
        for unit in &endpoint.units {
            if let Some(values) = c.values.get(&unit.id) {
                bytes = bytes.saturating_add(
                    crate::resources::serialized_size(&values.raw)?.saturating_mul(4),
                );
            }
        }
        let staging = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: bytes,
                ..Default::default()
            })?;
        let (endpoint, values) = w.applied_values(id)?;
        Ok((crate::tcp_slave::Snapshot { endpoint, values }, staging))
    }
    /// 独立运行命令入口，与配置提交事件和序号分开。
    /// # Errors
    /// 未应用、非TCP从站、端点操作冲突、监听或记录失败。
    pub fn execute_slave(
        &self,
        command: essim_core::application::commands::slave::SlaveCommand,
    ) -> Result<essim_core::application::queries::slave::SlaveView, Error> {
        self.execute_slave_permitted(command, None)
    }
    /// 在本次操作凭证内调用从站；等待超时后回调仍持有门。
    /// # Errors
    /// 凭证不匹配、运行能力缺失、命令非法或结果未确认。
    pub fn execute_slave_permitted(
        &self,
        command: essim_core::application::commands::slave::SlaveCommand,
        parent: Option<&OperationPermit>,
    ) -> Result<essim_core::application::queries::slave::SlaveView, Error> {
        let _project = self.project_operation()?;
        let operation = self.operations.enter(&command.endpoint, parent)?;

        use essim_core::application::commands::slave::SlaveAction;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP从站运行服务"))?;
        let gate = runtime.gate(&command.endpoint)?;
        let _guard = gate
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))?;
        let (snapshot, _staging) = if matches!(&command.action, SlaveAction::Start { .. }) {
            let (snapshot, staging) = self.applied_runtime_snapshot(&command.endpoint)?;
            (Some(snapshot), Some(staging))
        } else {
            (None, None)
        };
        let id = command.endpoint.clone();
        let result = runtime.execute_with_permit(command, snapshot, Some(&operation));
        let state = if result.as_ref().is_err_and(|e| e.code == "RUNTIME_TIMEOUT") {
            Err(Error::new("RUNTIME_TIMEOUT", "实际结果尚未确认"))
        } else {
            runtime.query(essim_core::application::queries::slave::SlaveQuery {
                endpoint: id.clone(),
                unit: None,
                points: vec![],
            })
        };
        if let Ok(state) = state {
            use essim_core::application::{ports::ApplyState, queries::slave::SlaveState};
            let state = match state.state {
                SlaveState::Running | SlaveState::Starting => ApplyState::Running,
                SlaveState::Stopping => ApplyState::Releasing,
                SlaveState::Failed => ApplyState::Failed,
                _ => ApplyState::Stopped,
            };
            self.workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .observe_release(id, state);
        }
        result
    }
    /// 在批量凭证内提交从站操作，返回真实所有者完成句柄；不以接收成功代替执行成功。
    /// # Errors
    /// 凭证不匹配、能力未装配、快照未应用或命令拒绝。
    pub fn submit_slave_permitted(
        &self,
        command: essim_core::application::commands::slave::SlaveCommand,
        parent: &OperationPermit,
    ) -> Result<crate::tcp_slave::SlaveCompletion, Error> {
        let _project = self.project_operation()?;
        let _operation = self.operations.enter(&command.endpoint, Some(parent))?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配从站运行服务"))?;
        let gate = runtime.gate(&command.endpoint)?;
        let _guard = gate
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))?;
        let (snapshot, _staging) = if matches!(
            &command.action,
            essim_core::application::commands::slave::SlaveAction::Start { .. }
        ) {
            let (snapshot, staging) = self.applied_runtime_snapshot(&command.endpoint)?;
            (Some(snapshot), Some(staging))
        } else {
            (None, None)
        };
        runtime.execute_pending_permitted(command, snapshot, parent)
    }
    /// 应用退出时停止TCP所有者并等待连接及记录句柄回收。
    pub fn shutdown_slave(&self) {
        if let Some(master) = &self.master {
            master.shutdown();
        }
        if let Some(runtime) = &self.runtime {
            runtime.shutdown();
            if let Err(error) = runtime.flush_project_recording() {
                tracing::error!(
                    event = "runtime_database_flush_failed",
                    errorCode = error.code.as_str(),
                    message = "运行数据库刷新未确认"
                );
            }
        }
    }
    /// 停止后确认工程数据库已处理所有入队记录，供显式退出链路使用。
    /// # Errors
    /// SQLite 队列积压或限时刷新未确认。
    pub fn flush_project_records(&self) -> Result<(), Error> {
        if let Some(runtime) = &self.runtime {
            runtime.flush_project_recording()?;
        }
        Ok(())
    }
    /// 查询独立运行摘要及最多1024个指定点的实际值，不复制配置目录。
    /// # Errors
    /// 未装配运行服务、查询非法或执行器不可用。
    pub fn query_slave(
        &self,
        query: essim_core::application::queries::slave::SlaveQuery,
    ) -> Result<essim_core::application::queries::slave::SlaveView, Error> {
        self.ensure_project()?;
        self.runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP从站运行服务"))?
            .query(query)
    }
}

#[cfg(feature = "tcp-runtime")]
impl WorkspaceEngine {
    /// 提交TCP主站命令；连接只读取已应用快照，不混入保存或草稿。
    /// # Errors
    /// 未装配服务、未应用或操作不合法。
    pub fn execute_master(
        &self,
        command: essim_core::application::commands::master::MasterCommand,
    ) -> Result<essim_core::application::queries::master::MasterView, Error> {
        let _project = self.project_operation()?;
        self.operations.ensure_available(&command.endpoint)?;
        use essim_core::application::commands::master::MasterAction;
        let master = self
            .master
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP主站"))?;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配端点操作门"))?;
        let gate = runtime.gate(&command.endpoint)?;
        let _guard = gate
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "端点操作门不可用"))?;
        self.operations.ensure_available(&command.endpoint)?;
        let (snapshot, _staging) = if matches!(command.action, MasterAction::Connect) {
            let (snapshot, staging) = self.applied_runtime_snapshot(&command.endpoint)?;
            (
                Some(crate::tcp_master::MasterSnapshot {
                    endpoint: snapshot.endpoint,
                    values: snapshot.values,
                }),
                Some(staging),
            )
        } else {
            (None, None)
        };
        master.execute(command, snapshot)
    }
    /// 查询主站值和质量，不等待网络请求。
    /// # Errors
    /// 查询非法或服务不可用。
    pub fn query_master(
        &self,
        query: essim_core::application::queries::master::MasterQuery,
    ) -> Result<essim_core::application::queries::master::MasterView, Error> {
        self.ensure_project()?;
        self.master
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP主站"))?
            .query(query)
    }
}

#[cfg(feature = "tcp-runtime")]
impl WorkspaceEngine {
    pub(crate) fn batch_target_state(
        &self,
        id: &EndpointId,
        role: essim_core::domain::endpoint::Role,
    ) -> Result<String, Error> {
        use essim_core::application::queries::{
            master::MasterQuery,
            slave::{SlaveQuery, SlaveState},
        };
        if role == essim_core::domain::endpoint::Role::Master {
            Ok(self
                .query_master(MasterQuery {
                    endpoint: id.clone(),
                    unit: None,
                    points: vec![],
                })?
                .state)
        } else {
            Ok(match self
                .query_slave(SlaveQuery {
                    endpoint: id.clone(),
                    unit: None,
                    points: vec![],
                })?
                .state
            {
                SlaveState::Running => "running",
                SlaveState::Starting => "starting",
                SlaveState::Stopping => "stopping",
                SlaveState::Failed => "failed",
                _ => "stopped",
            }
            .into())
        }
    }
    pub(crate) fn run_batch_target(
        &self,
        plan: &essim_core::application::commands::batch_targets::BatchPlan,
        id: &EndpointId,
        permit: &OperationPermit,
    ) -> Result<Option<String>, Error> {
        use essim_core::{
            application::{
                commands::{
                    master::{MasterAction, MasterCommand},
                    slave::{SlaveAction, SlaveCommand},
                },
                queries::master::MasterQuery,
            },
            domain::{aggregates::batch::BatchOperation, endpoint::Role},
        };
        let _operation = self.operations.enter(id, Some(permit))?;
        if plan.preview().operation == BatchOperation::Apply {
            self.apply_batch_target(plan, id, permit)?;
            return Ok(None);
        }
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配运行服务"))?;
        let gate = runtime.gate(id)?;
        let _guard = gate
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "端点锁不可用"))?;
        let mut snapshot = {
            let w = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?;
            w.revalidate_batch(plan, id)?;
            match w.catalog_ref().applied.iter().find(|e| &e.id == id) {
                Some(endpoint) => crate::tcp_slave::Snapshot {
                    endpoint: endpoint.clone(),
                    values: Default::default(),
                },
                None => return Ok(Some("尚未应用，已跳过".into())),
            }
        };
        let starting = plan.preview().operation == BatchOperation::Start;
        let role = snapshot.endpoint.role;
        let state = self.batch_target_state(id, role)?;
        if starting && ["running", "connected", "polling"].contains(&state.as_str()) {
            return Ok(Some("已运行，已跳过".into()));
        }
        if !starting && ["stopped", "disconnected"].contains(&state.as_str()) {
            return Ok(Some("已停止，已跳过".into()));
        }
        let _staging = if starting {
            let (prepared, staging) = self.applied_runtime_snapshot(id)?;
            snapshot = prepared;
            Some(staging)
        } else {
            None
        };
        let command_id = format!("{}:{}", plan.preview().id, id.0);
        if role == Role::Master {
            let master = self
                .master
                .as_ref()
                .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配主站"))?;
            if !starting {
                match master.disconnect_wait_permitted(id, Some(permit)) {
                    Ok(()) => return Ok(None),
                    Err(e) if e.code == "MASTER_RELEASE_TIMEOUT" => loop {
                        let v = master.query(MasterQuery {
                            endpoint: id.clone(),
                            unit: None,
                            points: vec![],
                        })?;
                        if v.state == "disconnected" && !v.busy {
                            return Ok(None);
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    },
                    Err(e) => return Err(e),
                }
            }
            master.execute_permitted(
                MasterCommand {
                    command_id: command_id.clone(),
                    endpoint: id.clone(),
                    action: MasterAction::Connect,
                },
                Some(crate::tcp_master::MasterSnapshot {
                    endpoint: snapshot.endpoint,
                    values: snapshot.values,
                }),
                Some(permit),
            )?;
            loop {
                let v = master.query(MasterQuery {
                    endpoint: id.clone(),
                    unit: None,
                    points: vec![],
                })?;
                if !v.busy {
                    if v.state == "connected" && v.operation.as_deref() == Some(&command_id) {
                        return Ok(None);
                    }
                    return Err(Error::new(
                        "MASTER_CONNECT_FAILED",
                        v.error.unwrap_or_else(|| "连接未成功".into()),
                    ));
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
        }
        let mut completion = runtime.execute_pending_permitted(
            SlaveCommand {
                command_id,
                endpoint: id.clone(),
                action: if starting {
                    SlaveAction::Start { max_clients: 10 }
                } else {
                    SlaveAction::Stop
                },
            },
            if starting { Some(snapshot) } else { None },
            permit,
        )?;
        loop {
            match completion.wait_timeout(std::time::Duration::from_millis(100))? {
                None => continue,
                Some(result) => {
                    let view = result?;
                    self.workspace
                        .lock()
                        .map_err(|_| Error::new("STATE_POISONED", "目录不可用"))?
                        .observe_release(
                            id.clone(),
                            if starting {
                                essim_core::application::ports::ApplyState::Running
                            } else {
                                essim_core::application::ports::ApplyState::Stopped
                            },
                        );
                    let _ = view;
                    return Ok(None);
                }
            }
        }
    }
}
