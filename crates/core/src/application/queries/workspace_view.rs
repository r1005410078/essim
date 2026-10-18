//! 工作区只读查询结果。
use super::Capabilities;
use crate::{
    application::{catalog::Catalog, ports::ApplyState},
    domain::{
        identity::{EndpointId, UnitId},
        serialization::decimal,
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 前端可订阅的只读查询结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    /// 核心协议比较结果，按端点和单元身份索引。
    #[serde(default)]
    pub application_matches: BTreeMap<EndpointId, BTreeMap<UnitId, bool>>,
    /// 停机边界已确认的状态；配置提交失败不能把它回滚成运行中。
    #[serde(default)]
    pub application_states: BTreeMap<EndpointId, ApplyState>,
    /// 最后已观察到的配置通知序号。
    #[serde(with = "decimal")]
    pub sequence: u64,
    /// 只读配置目录副本。
    pub catalog: Catalog,
    /// 实际能力。
    pub capabilities: Capabilities,
}
