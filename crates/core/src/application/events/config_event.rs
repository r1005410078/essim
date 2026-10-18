//! 配置事件信封与查询恢复快照。
use super::ConfigurationFact;
use crate::{application::catalog::Catalog, domain::serialization::decimal};
use serde::{Deserialize, Serialize};
/// 配置提交事件信封；具体事实与查询快照分别表达。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEvent {
    /// 事件数据版本，当前为1。
    pub schema: u32,
    /// 本次原子提交确认的具体配置事实。
    pub fact: ConfigurationFact,
    /// 全局配置通知序号。
    #[serde(with = "decimal")]
    pub sequence: u64,
    /// 命令因果身份。
    pub command_id: String,
    /// 稳定对象身份。
    pub object_id: String,
    /// 当前完整配置投影，用于M01小目录恢复；不是运行历史存储。
    pub catalog: Catalog,
}
