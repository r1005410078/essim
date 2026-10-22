//! 点位分组的只读摘要，不复制每组完整点表。
use crate::domain::identity::ModelId;
use serde::{Deserialize, Serialize};
/// 单组地址范围摘要；多个范围表示空洞或不同数据区。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionGroup {
    /// 目标IPv4。
    pub ip: String,
    /// 统一端口。
    pub port: u16,
    /// 统一站号。
    pub station: u8,
    /// 模型及端点名称。
    pub name: String,
    /// 完整业务点数。
    pub points: usize,
    /// 数据区、起始地址及结束地址（闭区间）。
    pub ranges: Vec<(String, u16, u16)>,
}
/// 总体分组预览。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PartitionPreview {
    /// 源模型稳定身份。
    pub source: ModelId,
    /// 源点总数。
    pub total_points: usize,
    /// 分组摘要。
    pub groups: Vec<PartitionGroup>,
}
