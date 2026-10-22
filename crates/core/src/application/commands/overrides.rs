//! 单层覆盖的共享回写与独立提取；提交由工作区仓储保证原子性。
use super::{Handled, endpoint, model};
use crate::application::{
    catalog::{Catalog, effective_model, find_model},
    events::ConfigurationFact,
};
use crate::domain::{
    endpoint::{EndpointConfig, UnitConfig},
    error::Error,
    identity::{EndpointId, ModelId, PointId, UnitId},
    model::{CollectionModel, OverrideField},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
/// 用户明确选中的回写差异。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OverrideSelection {
    /// 基础点的选定字段。
    #[serde(default)]
    pub fields: BTreeMap<PointId, BTreeSet<OverrideField>>,
    /// 要沉淀到基础模型的局部点。
    #[serde(default)]
    pub added: BTreeSet<PointId>,
    /// 要从基础模型删除的隐藏点。
    #[serde(default)]
    pub removed: BTreeSet<PointId>,
}
/// 回写期间需要复核的保存引用；不包含独立运行状态。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OverrideReference {
    /// 所属端点。
    pub endpoint: EndpointId,
    /// 当时的单元配置。
    pub unit: UnitConfig,
}
/// 操作级内容基线，不是模型配置版本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionPlan {
    /// 源端点。
    pub endpoint: EndpointId,
    /// 源单元。
    pub unit: UnitId,
    /// 实际选定差异。
    pub selection: OverrideSelection,
    /// 预览时的基础内容。
    pub before: CollectionModel,
    /// 预览时的保存引用集合。
    pub references: Vec<OverrideReference>,
    /// 预览后的模型；提交时重新计算并核对。
    pub after: CollectionModel,
}
fn source<'a>(c: &'a Catalog, e: &EndpointId, u: &UnitId) -> Result<&'a UnitConfig, Error> {
    c.endpoints
        .iter()
        .find(|x| &x.id == e)
        .and_then(|x| x.units.iter().find(|x| &x.id == u))
        .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "单元不存在").at_unit(e, Some(u), "unit"))
}
fn references(c: &Catalog, id: &ModelId) -> Vec<OverrideReference> {
    let mut result: Vec<_> = c
        .endpoints
        .iter()
        .flat_map(|e| {
            e.units
                .iter()
                .filter(|u| &u.model.id == id)
                .map(|u| OverrideReference {
                    endpoint: e.id.clone(),
                    unit: u.clone(),
                })
        })
        .collect();
    result.sort_by(|a, b| (&a.endpoint, &a.unit.id).cmp(&(&b.endpoint, &b.unit.id)));
    result
}
fn selected_model(
    c: &Catalog,
    e: &EndpointId,
    u: &UnitId,
    selection: &OverrideSelection,
) -> Result<(CollectionModel, UnitConfig), Error> {
    let mut unit = source(c, e, u)?.clone();
    let mut result = find_model(c, &unit.model)?.clone();
    if selection.fields.values().all(|f| f.is_empty())
        && selection.added.is_empty()
        && selection.removed.is_empty()
    {
        return Err(Error::new("EMPTY_SELECTION", "请选择要回写的差异"));
    }
    for (id, fields) in &selection.fields {
        let patch = unit.overrides.point_overrides.get_mut(id).ok_or_else(|| {
            Error::new("OVERRIDE_NOT_FOUND", "选定覆盖已不存在").at_point(id, "overrides")
        })?;
        if fields.iter().any(|f| !patch.fields().contains(f)) {
            return Err(Error::new("OVERRIDE_NOT_FOUND", "选定字段未覆盖"));
        }
        let patch = patch.take(fields);
        let point = result
            .points
            .iter_mut()
            .find(|p| &p.id == id)
            .ok_or_else(|| Error::new("POINT_NOT_FOUND", "基础点不存在"))?;
        *point = patch.apply(point)?;
    }
    unit.overrides
        .point_overrides
        .retain(|_, p| !p.fields().is_empty());
    if selection
        .added
        .iter()
        .any(|id| !unit.overrides.added_points.iter().any(|p| &p.id == id))
    {
        return Err(Error::new("OVERRIDE_NOT_FOUND", "局部点不存在"));
    }
    // 选区是集合，但追加顺序来自原局部点表，不能按随机 PointId 排序。
    let (promoted, retained): (Vec<_>, Vec<_>) = unit
        .overrides
        .added_points
        .into_iter()
        .partition(|p| selection.added.contains(&p.id));
    result.points.extend(promoted);
    unit.overrides.added_points = retained;
    for id in &selection.removed {
        if !unit.overrides.removed_point_ids.remove(id) {
            return Err(Error::new("OVERRIDE_NOT_FOUND", "隐藏点不存在"));
        }
        result.points.retain(|p| &p.id != id);
    }
    result.validate()?;
    Ok((result, unit))
}
/// 基于保存内容规划选择性回写并校验所有引用。
/// # Errors
/// 源失效、选区为空、字段依赖或引用合成冲突时拒绝。
pub fn preview_promotion(
    c: &Catalog,
    e: EndpointId,
    u: UnitId,
    selection: OverrideSelection,
) -> Result<PromotionPlan, Error> {
    let unit = source(c, &e, &u)?;
    let before = find_model(c, &unit.model)?.clone();
    let refs = references(c, &before.id);
    let (after, next) = selected_model(c, &e, &u, &selection)?;
    // 仅暂存配置，省略运行快照和值，避免预览复制无关运行数据。
    let mut staged = Catalog {
        models: c.models.clone(),
        endpoints: c.endpoints.clone(),
        ..Default::default()
    };
    *staged
        .endpoints
        .iter_mut()
        .find(|x| x.id == e)
        .unwrap()
        .units
        .iter_mut()
        .find(|x| x.id == u)
        .unwrap() = next;
    model::save_model(&mut staged, after.clone())?;
    Ok(PromotionPlan {
        endpoint: e,
        unit: u,
        selection,
        before,
        references: refs,
        after,
    })
}
pub(crate) fn promote(c: &mut Catalog, plan: PromotionPlan) -> Result<Handled, Error> {
    let current = source(c, &plan.endpoint, &plan.unit)?;
    if current.model.id != plan.before.id
        || find_model(c, &current.model)? != &plan.before
        || references(c, &plan.before.id) != plan.references
    {
        return Err(Error::new(
            "PREVIEW_CHANGED",
            "模型或引用已改变，请刷新回写预览",
        ));
    }
    let (after, next) = selected_model(c, &plan.endpoint, &plan.unit, &plan.selection)?;
    if after != plan.after {
        return Err(Error::new("PREVIEW_CHANGED", "预览内容不一致，请重新预览"));
    }
    *c.endpoints
        .iter_mut()
        .find(|e| e.id == plan.endpoint)
        .unwrap()
        .units
        .iter_mut()
        .find(|u| u.id == plan.unit)
        .unwrap() = next;
    model::save_model(c, after)?;
    Ok(Handled {
        object_id: plan.endpoint.0.clone(),
        fact: ConfigurationFact::UnitOverridesPromoted {
            endpoint: plan.endpoint,
            unit: plan.unit,
            model: plan.before.id,
        },
    })
}
pub(crate) fn create(
    c: &mut Catalog,
    endpoint: EndpointConfig,
    model: CollectionModel,
) -> Result<Handled, Error> {
    if c.models.iter().any(|m| m.id == model.id) {
        return Err(Error::new("DUPLICATE_ID", "新模型身份已使用"));
    }
    if endpoint.units.len() != 1 || endpoint.units[0].model.id != model.id {
        return Err(Error::new("INVALID_BINDING", "新模型必须绑定首个 Unit"));
    }
    let id = model.id.clone();
    model::save_model(c, model)?;
    let endpoint_id = endpoint.id.clone();
    endpoint::create_endpoint(c, endpoint)?;
    Ok(Handled {
        object_id: endpoint_id.0.clone(),
        fact: ConfigurationFact::EndpointModelCreated {
            endpoint: endpoint_id,
            model: id,
        },
    })
}
pub(crate) fn extract(
    c: &mut Catalog,
    e: EndpointId,
    u: UnitId,
    id: ModelId,
    name: String,
) -> Result<Handled, Error> {
    if id.0.trim().is_empty() || c.models.iter().any(|m| m.id == id) {
        return Err(Error::new("DUPLICATE_ID", "新模型身份为空或已使用"));
    }
    let mut result = effective_model(c, source(c, &e, &u)?)?;
    result.id = id.clone();
    result.name = name;
    let mut used: BTreeSet<_> = c
        .models
        .iter()
        .flat_map(|m| m.points.iter().map(|p| p.id.clone()))
        .chain(c.endpoints.iter().flat_map(|e| {
            e.units
                .iter()
                .flat_map(|u| u.overrides.added_points.iter().map(|p| p.id.clone()))
        }))
        .collect();
    for (i, p) in result.points.iter_mut().enumerate() {
        let mut suffix = 0u64;
        loop {
            let next = PointId(format!("{}:point:{i}:{suffix}", id.0));
            if used.insert(next.clone()) {
                p.id = next;
                break;
            }
            suffix = suffix
                .checked_add(1)
                .ok_or_else(|| Error::new("IDENTITY_EXHAUSTED", "点身份耗尽"))?;
        }
    }
    result.validate()?;
    c.models.push(result);
    let unit = c
        .endpoints
        .iter_mut()
        .find(|x| x.id == e)
        .unwrap()
        .units
        .iter_mut()
        .find(|x| x.id == u)
        .unwrap();
    unit.model.id = id.clone();
    unit.overrides = Default::default();
    Ok(Handled {
        object_id: e.0.clone(),
        fact: ConfigurationFact::UnitModelExtracted {
            endpoint: e,
            unit: u,
            model: id,
        },
    })
}
