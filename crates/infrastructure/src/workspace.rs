//! 会话配置适配器；可选装配真实TCP从站，配置目录仍不持久化。
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
}
impl ConfigRepository for MemoryRepository {
    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error> {
        self.catalog = catalog;
        Ok(())
    }
}
/// 应用组装使用的线程安全入口；临界区只处理短配置操作。
pub struct WorkspaceEngine {
    workspace: Mutex<Workspace<MemoryRepository>>,
    boundary: Mutex<Box<dyn ApplyBoundary + Send>>,
    #[cfg(feature = "tcp-runtime")]
    runtime: Option<std::sync::Arc<crate::tcp_slave::TcpSlaveRuntime>>,
    #[cfg(feature = "tcp-runtime")]
    master: Option<crate::tcp_master::TcpMasterRuntime>,
}
impl Default for WorkspaceEngine {
    fn default() -> Self {
        Self {
            workspace: Mutex::new(Workspace::new(MemoryRepository::default())),
            boundary: Mutex::new(Box::new(StoppedBoundary)),
            #[cfg(feature = "tcp-runtime")]
            runtime: None,
            #[cfg(feature = "tcp-runtime")]
            master: None,
        }
    }
}
impl WorkspaceEngine {
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
            view.capabilities.reason =
                "当前支持TCP主从站运行及文件录制；RTU工作台和历史回放尚未接入".into();
        }
        Ok(view)
    }
    /// 比较草稿是否仍对应已应用映射，不提交目录或运行值。
    /// # Errors
    /// 配置状态锁中毒时失败。
    pub fn inspect_endpoint(
        &self,
        endpoint: EndpointConfig,
    ) -> Result<std::collections::BTreeMap<UnitId, bool>, Error> {
        let workspace = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?;
        Ok(inspect_endpoint(&workspace.query().catalog, &endpoint))
    }
    /// 读取完整端点迁移预览，保持无副作用。
    /// # Errors
    /// 锁中毒、配置非法或迁移不兼容时失败。
    pub fn preview_apply(&self, id: EndpointId, reset: bool) -> Result<ApplyPreview, Error> {
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .preview_apply(&id, reset)
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
        #[cfg(feature = "tcp-runtime")]
        if let (Some(runtime), Action::ResetUnit { id, .. }) = (&self.runtime, &command.action) {
            let id = id.clone();
            let gate = runtime.gate(&id)?;
            let _guard = gate
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))?;
            if let Some(master) = &self.master {
                master.disconnect_wait(&id)?;
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
                runtime.forget(&id)?;
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
            let plan = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .prepare_release(command.clone())?;
            #[allow(unused_mut)]
            let mut stopped_values = None;
            #[cfg(feature = "tcp-runtime")]
            if let Some(master) = &self.master {
                master.disconnect_wait(&plan.endpoint().id)?;
                stopped_values = master.stopped_values(&plan.endpoint().id)?;
            }
            #[cfg(feature = "tcp-runtime")]
            if let Some(runtime) = &self.runtime {
                let id = plan.endpoint().id.clone();
                if let Err(error) = runtime.execute(
                    essim_core::application::commands::slave::SlaveCommand {
                        command_id: command.command_id.clone(),
                        endpoint: id.clone(),
                        action: essim_core::application::commands::slave::SlaveAction::Stop,
                    },
                    None,
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
            #[cfg(feature="tcp-runtime")]
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
                    runtime.forget(&id)?;
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
                        if endpoint.role == essim_core::domain::endpoint::Role::Master
                            && matches!(
                                endpoint.transport,
                                essim_core::domain::endpoint::Transport::Tcp { .. }
                            )
                        {
                            master.reconfigure(crate::tcp_master::MasterSnapshot {
                                endpoint,
                                values,
                            })?;
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
        Ok(Self {
            master: Some(crate::tcp_master::TcpMasterRuntime::new(
                record_root.join("masters"),
            )),
            runtime: Some(std::sync::Arc::new(crate::tcp_slave::TcpSlaveRuntime::new(
                record_root,
            )?)),
            ..Self::default()
        })
    }
    /// 独立运行命令入口，与配置提交事件和序号分开。
    /// # Errors
    /// 未应用、非TCP从站、端点操作冲突、监听或记录失败。
    pub fn execute_slave(
        &self,
        command: essim_core::application::commands::slave::SlaveCommand,
    ) -> Result<essim_core::application::queries::slave::SlaveView, Error> {
        use essim_core::application::commands::slave::SlaveAction;
        let runtime = self
            .runtime
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP从站运行服务"))?;
        let gate = runtime.gate(&command.endpoint)?;
        let _guard = gate
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "端点操作锁不可用"))?;
        let snapshot = if matches!(&command.action, SlaveAction::Start { .. }) {
            let (endpoint, values) = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .applied_values(&command.endpoint)?;
            Some(crate::tcp_slave::Snapshot { endpoint, values })
        } else {
            None
        };
        let id = command.endpoint.clone();
        let result = runtime.execute(command, snapshot);
        let state = runtime.query(essim_core::application::queries::slave::SlaveQuery {
            endpoint: id.clone(),
            unit: None,
            points: vec![],
        });
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
    /// 应用退出时停止TCP所有者并等待连接及记录句柄回收。
    pub fn shutdown_slave(&self) {
        if let Some(master) = &self.master {
            master.shutdown();
        }
        if let Some(runtime) = &self.runtime {
            runtime.shutdown();
        }
    }
    /// 查询独立运行摘要及最多1024个指定点的实际值，不复制配置目录。
    /// # Errors
    /// 未装配运行服务、查询非法或执行器不可用。
    pub fn query_slave(
        &self,
        query: essim_core::application::queries::slave::SlaveQuery,
    ) -> Result<essim_core::application::queries::slave::SlaveView, Error> {
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
        let snapshot = if matches!(command.action, MasterAction::Connect) {
            let (endpoint, values) = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .applied_values(&command.endpoint)?;
            Some(crate::tcp_master::MasterSnapshot { endpoint, values })
        } else {
            None
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
        self.master
            .as_ref()
            .ok_or_else(|| Error::new("CAPABILITY_UNAVAILABLE", "未装配TCP主站"))?
            .query(query)
    }
}
