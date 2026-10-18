//! 模拟器配置命令，协调模型／设备引用和身份。
use super::Handled;
use crate::application::{
    catalog::{validate_endpoint, Catalog},
    events::ConfigurationFact,
};
use crate::domain::{
    endpoint::EndpointConfig,
    error::Error,
    identity::{EndpointId, UnitId},
};
pub(crate) fn create_endpoint(
    catalog: &mut Catalog,
    endpoint: EndpointConfig,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::EndpointCreated {
        id: endpoint.id.clone(),
    };
    let object_id = {
        if catalog.endpoints.iter().any(|e| e.id == endpoint.id) {
            return Err(Error::new("DUPLICATE_ID", "端点身份已使用"));
        }
        if endpoint.units.len() != 1 {
            return Err(
                Error::new("FIRST_UNIT_REQUIRED", "新建必须包含一个Unit").at_unit(
                    &endpoint.id,
                    None,
                    "units",
                ),
            );
        }
        validate_endpoint(catalog, &endpoint)?;
        let id = endpoint.id.0.clone();
        catalog.endpoints.push(endpoint);
        id
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn save_endpoint(
    catalog: &mut Catalog,
    endpoint: EndpointConfig,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::EndpointSaved {
        id: endpoint.id.clone(),
    };
    let object_id = {
        let old = catalog
            .endpoints
            .iter()
            .find(|e| e.id == endpoint.id)
            .ok_or_else(|| Error::new("NOT_FOUND", "端点已不存在，不能保存旧草稿"))?;
        old.ensure_same_type(&endpoint)?;
        validate_endpoint(catalog, &endpoint)?;
        let id = endpoint.id.0.clone();
        let target = catalog.endpoints.iter_mut().find(|e| e.id.0 == id).unwrap();
        *target = endpoint;
        id
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn copy_endpoint(
    catalog: &mut Catalog,
    source: EndpointId,
    id: EndpointId,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::EndpointCopied {
        source: source.clone(),
        id: id.clone(),
    };
    let object_id = {
        if id.0.trim().is_empty() || catalog.endpoints.iter().any(|e| e.id == id) {
            return Err(Error::new("DUPLICATE_ID", "副本身份为空或已使用"));
        }
        let mut copy = catalog
            .endpoints
            .iter()
            .find(|e| e.id == source)
            .cloned()
            .ok_or_else(|| Error::new("NOT_FOUND", "源模拟器不存在"))?;
        copy.id = id.clone();
        copy.name.push_str(" 副本");
        let mut used: std::collections::BTreeSet<_> = catalog
            .endpoints
            .iter()
            .chain(&catalog.applied)
            .flat_map(|e| e.units.iter().map(|u| u.id.clone()))
            .collect();
        for (index, unit) in copy.units.iter_mut().enumerate() {
            let mut suffix = 0u64;
            loop {
                let candidate = UnitId(format!("{}:unit:{index}:{suffix}", id.0));
                if used.insert(candidate.clone()) {
                    unit.id = candidate;
                    break;
                }
                suffix = suffix
                    .checked_add(1)
                    .ok_or_else(|| Error::new("IDENTITY_EXHAUSTED", "身份耗尽"))?;
            }
        }
        validate_endpoint(catalog, &copy)?;
        if let Some(parent) = catalog.navigation.members.get(&source).cloned() {
            catalog.navigation.members.insert(id.clone(), parent);
        }
        catalog.endpoints.push(copy);
        id.0
    };
    Ok(Handled { object_id, fact })
}
