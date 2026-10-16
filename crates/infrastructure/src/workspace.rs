//! M01 会话内配置适配器；没有持久化、协议执行或可靠录制承诺。
use essim_core::{
    application::workspace::{
        ApplyPreview, Catalog, Command, ConfigRepository, Receipt, Workspace, WorkspaceView,
    },
    domain::foundation::{EndpointId, Error},
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
}
impl Default for WorkspaceEngine {
    fn default() -> Self {
        Self {
            workspace: Mutex::new(Workspace::new(MemoryRepository::default())),
        }
    }
}
impl WorkspaceEngine {
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
    /// 读取完整端点迁移预览，保持无副作用。
    /// # Errors
    /// 锁中毒、版本冲突或迁移不兼容时失败。
    pub fn preview_apply(
        &self,
        id: EndpointId,
        revision: u64,
        reset: bool,
    ) -> Result<ApplyPreview, Error> {
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .preview_apply(&id, revision, reset)
    }
    /// 唯一配置修改入口，生产运行能力未接入时明确拒绝。
    /// # Errors
    /// 配置校验、版本冲突、未接入能力或状态锁故障。
    pub fn execute(&self, command: Command) -> Result<Receipt, Error> {
        self.workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置状态不可用"))?
            .execute(command)
            .map(|(receipt, _)| receipt)
    }
}
