//! 查询投影状态与只读快照构造。
use super::{application_matches, Capabilities, WorkspaceView};
use crate::{
    application::{catalog::Catalog, events::ConfigEvent},
    domain::error::Error,
};
use std::collections::BTreeMap;
/// 可重建投影；拒绝缺口和未知数据版本。
#[derive(Debug, Clone, Default)]
pub struct Projection {
    pub(in crate::application) sequence: u64,
    pub(in crate::application) catalog: Catalog,
}
impl Projection {
    pub(crate) fn new(catalog: Catalog) -> Self {
        Self {
            sequence: 0,
            catalog,
        }
    }
    pub(crate) fn query_catalog(&self) -> &Catalog {
        &self.catalog
    }
    pub(crate) fn replace_catalog(&mut self, catalog: Catalog) {
        self.catalog = catalog;
    }

    /// 同步消费配置事件；兼容入口委托给显式投影监听器。
    /// # Errors
    /// 未知schema或序号缺口时要求重新查询完整快照。
    pub fn accept(&mut self, event: &ConfigEvent) -> Result<bool, Error> {
        crate::application::event_handlers::WorkspaceProjectionListener::handle(self, event)
    }
    /// 获取独立只读副本，不触发通信。
    pub fn query(&self) -> WorkspaceView {
        WorkspaceView {
            application_matches: application_matches(&self.catalog),
            application_states: BTreeMap::new(),
            sequence: self.sequence,
            catalog: self.catalog.clone(),
            capabilities: Capabilities::default(),
        }
    }
}
