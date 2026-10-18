//! 已应用快照和会话值迁移；不读取远端或操纵连接。
use super::{
    endpoint::Role, error::Error, identity::*, model::CollectionModel, point_table::RawValue,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 已应用单元的会话内值；运行时接入后所有权归执行器。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitValues {
    /// 实际应用的模型快照。
    pub model: Option<CollectionModel>,
    /// 原始值，不随倍率变化重算。
    pub raw: BTreeMap<PointId, RawValue>,
    /// 主站上次结果是否过期。
    pub stale: bool,
}
/// 倍率变化时同一原始值对应的工程显示变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringChange {
    /// 稳定点身份。
    pub point: PointId,
    /// 原倍率下的工程显示。
    pub previous: String,
    /// 新倍率下的工程显示。
    pub next: String,
}
/// 应用预览，展示增删、恢复及倍率变化。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Migration {
    /// 原始值保留时的工程值变化。
    pub engineering_changed: Vec<EngineeringChange>,
    /// 迁移后的独立值。
    pub values: UnitValues,
    /// 倍率改变而保留原始值的点。
    pub scale_changed: Vec<PointId>,
    /// 新增或明确恢复初值的点。
    pub initialized: Vec<PointId>,
    /// 删除的点。
    pub removed: Vec<PointId>,
}
/// 纯迁移预览；失败不修改原模型或原始值。
/// # Errors
/// 类型变化且未显式允许恢复初值时返回不兼容错误。
pub fn migrate(
    old: Option<&CollectionModel>,
    current: &UnitValues,
    next: &CollectionModel,
    reset_incompatible: bool,
) -> Result<Migration, Error> {
    next.validate()?;
    let mut preview = Migration::default();
    preview.values.model = Some(next.clone());
    preview.values.stale = current.stale;
    let previous_points: BTreeMap<_, _> = old
        .filter(|model| model.id == next.id)
        .map(|model| {
            model
                .points
                .iter()
                .map(|point| (&point.id, point))
                .collect()
        })
        .unwrap_or_default();
    for point in &next.points {
        let previous = previous_points.get(&point.id).copied();
        let value = match previous {
            Some(previous)
                if super::point_table::convert_compatible(
                    previous.value_type,
                    point.value_type,
                    current.raw.get(&point.id).unwrap_or(&previous.initial),
                )
                .is_some() =>
            {
                if previous.scale != point.scale {
                    preview.scale_changed.push(point.id.clone());
                    let raw = current.raw.get(&point.id).unwrap_or(&previous.initial);
                    preview.engineering_changed.push(EngineeringChange {
                        point: point.id.clone(),
                        previous: super::point_table::engineering_value(raw, &previous.scale)?,
                        next: super::point_table::engineering_value(raw, &point.scale)?,
                    });
                }
                super::point_table::convert_compatible(
                    previous.value_type,
                    point.value_type,
                    current.raw.get(&point.id).unwrap_or(&previous.initial),
                )
                .ok_or_else(|| Error::new("INCOMPATIBLE_TYPE", "点值不能保留"))?
            }
            Some(_) if !reset_incompatible => {
                return Err(Error::new(
                    "INCOMPATIBLE_TYPE",
                    "点类型变化，请明确选择恢复初值",
                ))
            }
            _ => {
                preview.initialized.push(point.id.clone());
                point.initial.clone()
            }
        };
        preview.values.raw.insert(point.id.clone(), value);
    }
    preview.removed = current
        .raw
        .keys()
        .filter(|id| !preview.values.raw.contains_key(*id))
        .cloned()
        .collect();
    Ok(preview)
}
/// 显式复位静止实例的本地值，不调用任何协议端口。
/// 从站恢复已应用模型初值；主站仅清空缓存并标记过期，不能隐式向远端写入。
pub fn reset_values(role: Role, model: &CollectionModel) -> UnitValues {
    UnitValues {
        model: Some(model.clone()),
        raw: if role == Role::Slave {
            model
                .points
                .iter()
                .map(|point| (point.id.clone(), point.initial.clone()))
                .collect()
        } else {
            BTreeMap::new()
        },
        stale: role == Role::Master,
    }
}
