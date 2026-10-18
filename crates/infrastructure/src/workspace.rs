//! M01 会话内配置适配器；没有持久化、协议执行或可靠录制承诺。
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
}
impl Default for WorkspaceEngine {
    fn default() -> Self {
        Self {
            workspace: Mutex::new(Workspace::new(MemoryRepository::default())),
            boundary: Mutex::new(Box::new(StoppedBoundary)),
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
        Ok(self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .query())
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
        if matches!(
            &command.action,
            Action::Apply { .. } | Action::DeleteEndpoint { .. }
        ) {
            let plan = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
                .prepare_release(command)?;
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
                workspace.observe_release(id, state);
                return Err(error);
            }
            return workspace.finish_release(plan);
        }
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .execute(command)
    }
}
