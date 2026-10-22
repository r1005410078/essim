//! 无副作用的完整配置应用与值迁移预览。
use crate::{
    application::catalog::{effective_model, validate_endpoint, Catalog},
    domain::{
        endpoint::{EndpointConfig, Role},
        error::Error,
        identity::{EndpointId, UnitId},
        values::{migrate, Migration},
    },
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 显式应用前的整组迁移预览，不更改目录或运行值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPreview {
    /// 将应用的保存配置，包括完整端点／通道和全部Unit。
    pub endpoint: EndpointConfig,
    /// 每个单元独立的迁移规划。
    pub units: BTreeMap<UnitId, Migration>,
}
pub(crate) fn prepare_apply(
    catalog: &Catalog,
    id: &EndpointId,
    reset: bool,
) -> Result<ApplyPreview, Error> {
    let endpoint = catalog
        .endpoints
        .iter()
        .find(|e| &e.id == id)
        .cloned()
        .ok_or_else(|| Error::new("NOT_FOUND", "端点不存在"))?;
    validate_endpoint(catalog, &endpoint)?;
    let mut units = BTreeMap::new();
    let mut models: Vec<std::sync::Arc<crate::domain::model::CollectionModel>> = Vec::new();
    for unit in &endpoint.units {
        let next = effective_model(catalog, unit)?;
        let current = catalog.values.get(&unit.id).cloned().unwrap_or_default();
        let old = current.model.as_deref();
        let mut migration = migrate(old, &current, &next, reset || endpoint.role == Role::Master)?;
        if endpoint.role == Role::Master {
            let same_source = catalog
                .applied
                .iter()
                .find(|e| &e.id == id)
                .is_some_and(|e| {
                    e.role == Role::Master
                        && e.transport == endpoint.transport
                        && e.units
                            .iter()
                            .any(|u| u.id == unit.id && u.station == unit.station)
                });
            let previous_points: BTreeMap<_, _> = old
                .filter(|m| m.id == next.id)
                .map(|m| m.points.iter().map(|p| (&p.id, p)).collect())
                .unwrap_or_default();
            let next_points: BTreeMap<_, _> = next.points.iter().map(|p| (&p.id, p)).collect();
            // 主站结构变化只让对应结果失效，不能把模型初值当成实际采集结果。
            migration.values.raw.retain(|id,_| {
                let previous=previous_points.get(id);
                let following=next_points.get(id);
                matches!((previous,following),(Some(a),Some(b)) if same_source && current.raw.contains_key(id) && a.area==b.area && a.address==b.address && a.value_type==b.value_type && a.byte_order==b.byte_order && a.word_order==b.word_order && a.scale==b.scale)
            });
            // 完整端点应用经过停止边界，保留的主站缓存也必须标记过期。
            migration.values.stale = true;
        }
        let shared = models.iter().chain(catalog.values.values().filter_map(|v|v.model.as_ref()))
            .find(|m|m.as_ref()==&next).cloned().unwrap_or_else(||std::sync::Arc::new(next));
        models.push(shared.clone());
        migration.values.model=Some(shared);
        units.insert(unit.id.clone(), migration);
    }
    Ok(ApplyPreview { endpoint, units })
}
