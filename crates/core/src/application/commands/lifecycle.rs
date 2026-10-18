//! 应用、删除及复位用例；请求释放与配置提交分开。
use super::Handled;
use crate::application::{catalog::Catalog, events::ConfigurationFact};
use crate::application::{
    ports::{ApplyBoundary, ApplyState, ConfigRepository},
    queries::prepare_apply,
};
use crate::domain::{
    error::Error,
    identity::{EndpointId, UnitId},
    values::reset_values,
};
use std::collections::BTreeMap;
pub(crate) struct LifecycleContext<'a, R: ConfigRepository, B: ApplyBoundary> {
    pub boundary: &'a mut B,
    pub states: &'a mut BTreeMap<EndpointId, ApplyState>,
    pub repository: &'a R,
    pub observed_catalog: &'a mut Option<Catalog>,
    pub released: bool,
}
pub(crate) fn delete_endpoint<R: ConfigRepository, B: ApplyBoundary>(
    catalog: &mut Catalog,
    id: EndpointId,
    context: &mut LifecycleContext<'_, R, B>,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::EndpointDeleted { id: id.clone() };
    let object_id = {
        let endpoint = catalog
            .endpoints
            .iter()
            .find(|e| e.id == id)
            .cloned()
            .ok_or_else(|| Error::new("NOT_FOUND", "模拟器不存在"))?;
        if !context.released {
            if let Err(error) = context.boundary.stop(&endpoint) {
                context
                    .states
                    .insert(endpoint.id.clone(), context.boundary.state(&endpoint.id));
                return Err(error);
            }
        }
        context.states.insert(id.clone(), ApplyState::Stopped);
        let authority = context.repository.load();
        if authority != *catalog {
            *context.observed_catalog = Some(authority);
            return Err(Error::new(
                "CONFIG_CHANGED",
                "释放期间配置变化，请重新确认删除",
            ));
        }
        for unit in catalog
            .endpoints
            .iter()
            .chain(&catalog.applied)
            .filter(|e| e.id == id)
            .flat_map(|e| &e.units)
        {
            catalog.values.remove(&unit.id);
        }
        catalog.endpoints.retain(|e| e.id != id);
        catalog.applied.retain(|e| e.id != id);
        catalog.navigation.members.remove(&id);
        id.0
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn reset_unit<R: ConfigRepository, B: ApplyBoundary>(
    catalog: &mut Catalog,
    id: EndpointId,
    unit: UnitId,
    context: &mut LifecycleContext<'_, R, B>,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::UnitValuesReset {
        id: id.clone(),
        unit: unit.clone(),
    };
    let object_id = {
        let state = context
            .states
            .get(&id)
            .copied()
            .unwrap_or_else(|| context.boundary.state(&id));
        if state != ApplyState::Stopped {
            return Err(Error::new(
                "CAPABILITY_UNAVAILABLE",
                "运行状态复位须由执行器值所有者处理",
            ));
        }
        let endpoint = catalog
            .applied
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用模拟器"))?;
        if !endpoint.units.iter().any(|u| u.id == unit) {
            return Err(Error::new("NOT_APPLIED", "此单元尚未应用"));
        }
        let model = catalog
            .values
            .get(&unit)
            .and_then(|v| v.model.as_ref())
            .ok_or_else(|| Error::new("NOT_APPLIED", "缺少已应用模型"))?;
        // 当前装配没有执行器；未来运行态必须经值所有者串行复位。
        catalog
            .values
            .insert(unit, reset_values(endpoint.role, model));
        id.0
    };
    Ok(Handled { object_id, fact })
}
pub(crate) fn apply<R: ConfigRepository, B: ApplyBoundary>(
    catalog: &mut Catalog,
    id: EndpointId,
    reset_incompatible: bool,
    context: &mut LifecycleContext<'_, R, B>,
) -> Result<Handled, Error> {
    let fact = ConfigurationFact::EndpointApplied { id: id.clone() };
    let object_id = {
        let preview = prepare_apply(catalog, &id, reset_incompatible)?;
        let endpoint = preview.endpoint;
        if !context.released {
            if let Err(error) = context.boundary.stop(&endpoint) {
                context
                    .states
                    .insert(endpoint.id.clone(), context.boundary.state(&endpoint.id));
                return Err(error);
            }
        }
        context.states.insert(id.clone(), ApplyState::Stopped);
        // 停机期间可能等待外部适配器，重新比较权威目录，不能覆盖较新配置。
        let authority = context.repository.load();
        if authority != *catalog {
            // 外部提交不是本命令成功事件；查询重新对齐权威配置，已应用值不回滚。
            *context.observed_catalog = Some(authority);
            return Err(Error::new(
                "CONFIG_CHANGED",
                "停机期间配置已变化，端点已停止，请重新预览",
            ));
        }
        let prepared = preview
            .units
            .into_iter()
            .map(|(id, migration)| (id, migration.values));
        if let Some(previous) = catalog.applied.iter().find(|old| old.id == id) {
            for unit in &previous.units {
                catalog.values.remove(&unit.id);
            }
        }
        catalog.values.extend(prepared);
        catalog.applied.retain(|old| old.id != id);
        catalog.applied.push(endpoint.clone());
        *catalog
            .endpoints
            .iter_mut()
            .find(|old| old.id == id)
            .ok_or_else(|| Error::new("NOT_FOUND", "端点不存在"))? = endpoint;
        id.0
    };
    Ok(Handled { object_id, fact })
}
