//! 跨聚合会话目录及引用协调；不是所有对象的领域聚合。
use crate::domain::{
    device::Device,
    endpoint::EndpointConfig,
    error::Error,
    identity::{EndpointId, ModelId, UnitId},
    model::{CollectionModel, ModelRef},
    navigation::Navigation,
    values::UnitValues,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 会话内权威配置目录；每个模型只保存当前配置。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// 当前模型集合，稳定ID唯一。
    pub models: Vec<CollectionModel>,
    /// 当前保存的端点聚合。
    pub endpoints: Vec<EndpointConfig>,
    /// 可选设备实体。
    pub devices: Vec<Device>,
    /// 已应用的配置副本，保存新配置不覆盖。
    pub applied: Vec<EndpointConfig>,
    /// 已应用的静止值快照；真实运行值由独立执行器所有者管理。
    pub values: BTreeMap<UnitId, UnitValues>,
    /// 会话导航，与协议配置独立。
    #[serde(default)]
    pub navigation: Navigation,
}
/// 模型的端点引用；配置和已应用状态分别计数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    /// 是否来自已应用配置。
    pub applied: bool,
    /// 引用端点身份。
    pub endpoint: EndpointId,
    /// 引用单元身份。
    pub unit: UnitId,
    /// 模型身份引用。
    pub model: ModelRef,
}
impl Catalog {
    /// 返回保存配置和已应用配置中的真实模型引用。
    pub fn model_references(&self, id: &ModelId) -> Vec<ModelUsage> {
        let mut references = Vec::new();
        for (applied, endpoints) in [(false, &self.endpoints), (true, &self.applied)] {
            for endpoint in endpoints {
                for unit in &endpoint.units {
                    if &unit.model.id == id {
                        references.push(ModelUsage {
                            applied,
                            endpoint: endpoint.id.clone(),
                            unit: unit.id.clone(),
                            model: unit.model.clone(),
                        });
                    }
                }
            }
        }
        references
    }
}
pub(crate) fn find_model<'a>(
    catalog: &'a Catalog,
    reference: &ModelRef,
) -> Result<&'a CollectionModel, Error> {
    catalog
        .models
        .iter()
        .find(|model| model.id == reference.id)
        .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "引用的模型不存在"))
}

pub(crate) fn validate_endpoint(catalog: &Catalog, endpoint: &EndpointConfig) -> Result<(), Error> {
    endpoint.validate()?;
    for unit in &endpoint.units {
        find_model(catalog, &unit.model)
            .map_err(|e| e.at_unit(&endpoint.id, Some(&unit.id), "model"))?;
        if unit
            .device
            .as_ref()
            .is_some_and(|id| !catalog.devices.iter().any(|d| &d.id == id))
        {
            return Err(Error::new("DEVICE_NOT_FOUND", "关联设备不存在").at_unit(
                &endpoint.id,
                Some(&unit.id),
                "device",
            ));
        }
        if catalog
            .endpoints
            .iter()
            .chain(&catalog.applied)
            .filter(|e| e.id != endpoint.id)
            .any(|e| e.units.iter().any(|u| u.id == unit.id))
        {
            return Err(
                Error::new("DUPLICATE_ID", "单元身份属于其他模拟器").at_unit(
                    &endpoint.id,
                    Some(&unit.id),
                    "id",
                ),
            );
        }
    }
    Ok(())
}
