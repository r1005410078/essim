//! 采集模型命令，协调引用检查与独立复制。
use super::Handled;
use crate::application::{
    catalog::{find_model, Catalog},
    events::ConfigurationFact,
};
use crate::domain::{
    error::Error,
    identity::{ModelId, PointId},
    model::{CollectionModel, ModelRef},
};
pub(crate) fn copy_model(
    catalog: &mut Catalog,
    source: ModelRef,
    id: ModelId,
    name: String,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::ModelCopied {
        source: source.id.clone(),
        id: id.clone(),
    };
    let object_id = {
        if id.0.trim().is_empty() || catalog.models.iter().any(|m| m.id == id) {
            return Err(Error::new("DUPLICATE_ID", "新模型身份为空或已使用"));
        }
        let mut model = find_model(catalog, &source)?.clone();
        model.id = id.clone();
        model.name = name;
        let mut used: std::collections::BTreeSet<_> = catalog
            .models
            .iter()
            .flat_map(|m| m.points.iter().map(|p| p.id.clone()))
            .collect();
        for (index, point) in model.points.iter_mut().enumerate() {
            let mut suffix = 0_u64;
            loop {
                let candidate = PointId(format!("{}:point:{index}:{suffix}", id.0));
                if used.insert(candidate.clone()) {
                    point.id = candidate;
                    break;
                }
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| Error::new("IDENTITY_EXHAUSTED", "点身份已耗尽"))?;
            }
        }
        model.validate()?;
        catalog.models.push(model);
        id.0
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn delete_model(catalog: &mut Catalog, id: ModelId) -> Result<Handled, Error> {
    let fact = ConfigurationFact::ModelDeleted { id: id.clone() };
    let object_id = {
        if !catalog.models.iter().any(|m| m.id == id) {
            return Err(Error::new("MODEL_NOT_FOUND", "模型不存在"));
        }
        let usages = catalog.model_references(&id);
        if !usages.is_empty() {
            return Err(Error::new(
                "MODEL_REFERENCED",
                format!(
                    "模型仍被配置或已应用状态引用：{}",
                    usages
                        .iter()
                        .map(|r| format!("{}→{}", r.endpoint.0, r.unit.0))
                        .collect::<Vec<_>>()
                        .join("，")
                ),
            ));
        }
        catalog.models.retain(|m| m.id != id);
        catalog.navigation.model_members.remove(&id);
        id.0
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn save_model(catalog: &mut Catalog, model: CollectionModel) -> Result<Handled, Error> {
    let fact = ConfigurationFact::ModelSaved {
        id: model.id.clone(),
    };
    let object_id = {
        model.validate()?;
        for endpoint in &catalog.endpoints {
            for unit in endpoint.units.iter().filter(|u|u.model.id==model.id) {
                let mut overrides=unit.overrides.clone();
                overrides.removed_point_ids.retain(|id|model.points.iter().any(|p|&p.id==id));
                overrides.resolve(&model).map_err(|mut e|{e.endpoint_id=Some(endpoint.id.0.clone());e.unit_id=Some(unit.id.0.clone());e})?;
            }
        }
        for endpoint in &mut catalog.endpoints {
            for unit in endpoint.units.iter_mut().filter(|u|u.model.id==model.id) {
                unit.overrides.removed_point_ids.retain(|id|model.points.iter().any(|p|&p.id==id));
            }
        }
        let result = model.id.0.clone();
        if let Some(current) = catalog.models.iter_mut().find(|old| old.id == model.id) {
            *current = model;
        } else {
            catalog.models.push(model);
        }
        result
    };
    Ok(Handled { object_id, fact })
}
