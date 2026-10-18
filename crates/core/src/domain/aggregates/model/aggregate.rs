//! 采集模型聚合根及协议比较规则。
use super::PointDefinition;
use crate::domain::{error::Error, identity::ModelId};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// 采集模型的当前配置。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionModel {
    /// 模型身份。
    pub id: ModelId,
    /// 名称。
    pub name: String,
    /// 唯一点集合，主从角色共用。
    pub points: Vec<PointDefinition>,
}
impl CollectionModel {
    /// 校验基础点定义；完整地址占用和编码规则由M02实现。
    /// # Errors
    /// 名称或ID为空、重复点ID、类型与数据区不符、倍率无效或采集周期为零时失败。
    pub fn validate(&self) -> Result<(), Error> {
        let mut ids = BTreeSet::new();
        if self.id.0.is_empty() || self.name.trim().is_empty() {
            return Err(Error::new("INVALID_MODEL", "模型身份和名称不能为空"));
        }
        for point in &self.points {
            if !ids.insert(&point.id) {
                return Err(
                    Error::new("DUPLICATE_POINT_ID", "点身份重复").at_point(&point.id, "id")
                );
            }
            point.validate()?;
        }
        crate::domain::point_table::validate_addresses(&self.points)?;
        Ok(())
    }
}
/// 比较协议点定义；名称、备注及单位显示文字不参与。
pub fn protocol_model_matches(a: &CollectionModel, b: &CollectionModel) -> bool {
    if a.id != b.id || a.points.len() != b.points.len() {
        return false;
    }
    a.points.iter().all(|p| {
        b.points.iter().find(|q| q.id == p.id).is_some_and(|q| {
            p.category == q.category
                && p.area == q.area
                && p.address == q.address
                && p.value_type == q.value_type
                && p.byte_order == q.byte_order
                && p.word_order == q.word_order
                && p.scale == q.scale
                && p.initial == q.initial
                && p.slave_writable == q.slave_writable
                && p.master_period_ms == q.master_period_ms
        })
    })
}
