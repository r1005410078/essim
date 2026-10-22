//! 保存草稿与已应用快照的协议差异查询。
pub use crate::domain::model::protocol_model_matches;
use crate::{
    application::catalog::Catalog,
    domain::{
        endpoint::EndpointConfig,
        identity::{EndpointId, UnitId},
    },
};
use std::collections::BTreeMap;
pub(crate) fn application_matches(
    catalog: &Catalog,
) -> BTreeMap<EndpointId, BTreeMap<UnitId, bool>> {
    catalog
        .endpoints
        .iter()
        .map(|endpoint| (endpoint.id.clone(), inspect_endpoint(catalog, endpoint)))
        .collect()
}
/// 只读比较任意端点草稿和已应用映射，允许不完整输入，不保存或迁移值。
pub fn inspect_endpoint(catalog: &Catalog, endpoint: &EndpointConfig) -> BTreeMap<UnitId, bool> {
    let applied = catalog.applied.iter().find(|e| e.id == endpoint.id);
    let units: BTreeMap<UnitId, bool> = endpoint
        .units
        .iter()
        .map(|unit| {
            let matches = applied.is_some_and(|e| {
                e.role == endpoint.role
                    && e.transport == endpoint.transport
                    && e.master == endpoint.master
                    && e.units.iter().any(|u| {
                        u.id == unit.id
                            && u.station == unit.station
                            && u.enabled == unit.enabled
                            && u.model == unit.model
                    })
            }) && crate::application::catalog::effective_model(catalog, unit).ok()
                .zip(catalog.values.get(&unit.id).and_then(|v| v.model.as_ref()))
                .is_some_and(|(a, b)| protocol_model_matches(&a, b));
            (unit.id.clone(), matches)
        })
        .collect();
    units
}
