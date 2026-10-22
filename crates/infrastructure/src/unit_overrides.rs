//! 单层覆盖的只读预览、有效点表与确认值捕获适配。
use crate::workspace::WorkspaceEngine;
#[cfg(feature = "tcp-runtime")]
use essim_core::domain::{
    endpoint::Role,
    point_table::{engineering_value, validate_raw},
};
use essim_core::{
    application::{
        catalog::effective_model,
        commands::overrides::{OverrideSelection, PromotionPlan, preview_promotion},
    },
    domain::{
        endpoint::UnitConfig,
        error::Error,
        identity::{EndpointId, PointId, UnitId},
        model::CollectionModel,
        point_table::RawValue,
    },
};
use serde::{Deserialize, Serialize};
/// 一次固定取样，只用于默认初值草稿。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitialSample {
    /// 点身份。
    pub point: PointId,
    /// 确认原始值。
    pub value: RawValue,
    /// 精确工程表达。
    pub engineering: String,
    /// 采样或最近成功读取时间，Unix 毫秒。
    pub unix_ms: String,
}
impl WorkspaceEngine {
    /// 解析配置草稿有效点表，不写权威目录。
    /// # Errors
    /// 引用或覆盖非法、预算不足或目录锁不可用。
    pub fn resolve_unit(&self, unit: UnitConfig) -> Result<CollectionModel, Error> {
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
        let _budget = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::catalog_memory(w.catalog_ref())?
                    .saturating_add(crate::resources::serialized_size(&unit)?.saturating_mul(4)),
                ..Default::default()
            })?;
        effective_model(w.catalog_ref(), &unit)
    }
    /// 规划共享回写，不改变目录或运行状态。
    /// # Errors
    /// 差异非法、共享引用冲突或预算不足。
    pub fn preview_unit_promotion(
        &self,
        endpoint: EndpointId,
        unit: UnitId,
        selection: OverrideSelection,
    ) -> Result<PromotionPlan, Error> {
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
        let _budget = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: crate::resources::catalog_memory(w.catalog_ref())?.saturating_mul(3),
                ..Default::default()
            })?;
        preview_promotion(w.catalog_ref(), endpoint, unit, selection)
    }
    /// 取得保存单元的有效点表，用于显式展平导出。
    /// # Errors
    /// 单元、模型缺失或覆盖非法。
    pub fn saved_effective_model(
        &self,
        endpoint: &EndpointId,
        unit: &UnitId,
    ) -> Result<CollectionModel, Error> {
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
        let c = w.catalog_ref();
        let u = c
            .endpoints
            .iter()
            .find(|e| &e.id == endpoint)
            .and_then(|e| e.units.iter().find(|u| &u.id == unit))
            .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "单元不存在"))?;
        effective_model(c, u)
    }
}
#[cfg(feature = "tcp-runtime")]
impl WorkspaceEngine {
    /// 固定当前已应用单元的确认值，不保存配置、不发送读写请求。
    /// # Errors
    /// 点过多、映射待应用、值未确认／过期、配置竞态或精确转换失败。
    pub fn capture_initials(
        &self,
        endpoint: EndpointId,
        unit: UnitId,
        points: Vec<PointId>,
    ) -> Result<Vec<InitialSample>, Error> {
        use essim_core::application::queries::{
            master::MasterQuery,
            slave::{SlaveQuery, SlaveState},
        };
        if points.is_empty()
            || points.len() > 1024
            || points
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != points.len()
        {
            return Err(Error::new(
                "INVALID_SELECTION",
                "一次选择1～1024个不重复的点",
            ));
        }
        let (config, saved, applied) = {
            let w = self
                .workspace
                .lock()
                .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
            let c = w.catalog_ref();
            let e = c
                .endpoints
                .iter()
                .find(|e| e.id == endpoint)
                .ok_or_else(|| Error::new("NOT_FOUND", "模拟器不存在"))?;
            let u = e
                .units
                .iter()
                .find(|u| u.id == unit)
                .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "单元不存在"))?;
            let applied = c
                .values
                .get(&unit)
                .cloned()
                .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用配置"))?;
            (e.clone(), effective_model(c, u)?, applied)
        };
        let old = applied
            .model
            .as_ref()
            .ok_or_else(|| Error::new("NOT_APPLIED", "缺少已应用映射"))?;
        let mut definitions = Vec::new();
        for id in &points {
            let a = old.points.iter().find(|p| &p.id == id);
            let b = saved.points.iter().find(|p| &p.id == id);
            if old.id != saved.id
                || !matches!((a,b),(Some(a),Some(b)) if a.area==b.area&&a.address==b.address&&a.value_type==b.value_type&&a.scale==b.scale&&a.byte_order==b.byte_order&&a.word_order==b.word_order)
            {
                return Err(
                    Error::new("MAPPING_PENDING", "选中点映射已改变，请先应用配置")
                        .at_point(id, "initialEngineering"),
                );
            }
            definitions.push(b.unwrap());
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| Error::new("CLOCK_INVALID", "时钟不可用"))?
            .as_millis()
            .to_string();
        let samples = if config.role == Role::Master {
            let view = self.query_master(MasterQuery {
                endpoint: endpoint.clone(),
                unit: Some(unit.clone()),
                points: points.clone(),
            })?;
            definitions
                .iter()
                .map(|p| {
                    let sample = view
                        .points
                        .get(&p.id)
                        .filter(|v| v.quality == "valid")
                        .ok_or_else(|| {
                            Error::new("VALUE_UNCONFIRMED", "仅可捕获有效且未过期的采集值")
                                .at_point(&p.id, "initialEngineering")
                        })?;
                    let value = sample
                        .value
                        .clone()
                        .ok_or_else(|| Error::new("VALUE_UNCONFIRMED", "缺少确认值"))?;
                    validate_raw(p.value_type, &value)?;
                    Ok(InitialSample {
                        point: p.id.clone(),
                        engineering: engineering_value(&value, &p.scale)?,
                        value,
                        unix_ms: sample
                            .unix_ms
                            .clone()
                            .ok_or_else(|| Error::new("VALUE_UNCONFIRMED", "缺少取样时间"))?,
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?
        } else {
            let view = self.query_slave(SlaveQuery {
                endpoint: endpoint.clone(),
                unit: Some(unit.clone()),
                points: points.clone(),
            })?;
            if view.pending_recording || view.uncertain {
                return Err(Error::new("VALUE_UNCONFIRMED", "运行值仍等待可靠确认"));
            }
            definitions
                .iter()
                .map(|p| {
                    let value = view
                        .values
                        .get(&p.id)
                        .or_else(|| {
                            if view.state == SlaveState::Idle {
                                applied.raw.get(&p.id)
                            } else {
                                None
                            }
                        })
                        .cloned()
                        .ok_or_else(|| {
                            Error::new("VALUE_UNCONFIRMED", "缺少确认值")
                                .at_point(&p.id, "initialEngineering")
                        })?;
                    validate_raw(p.value_type, &value)?;
                    Ok(InitialSample {
                        point: p.id.clone(),
                        engineering: engineering_value(&value, &p.scale)?,
                        value,
                        unix_ms: now.clone(),
                    })
                })
                .collect::<Result<Vec<_>, Error>>()?
        };
        let w = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "配置不可用"))?;
        let c = w.catalog_ref();
        if c.endpoints.iter().find(|e| e.id == endpoint) != Some(&config)
            || c.values.get(&unit).and_then(|v| v.model.as_ref()) != applied.model.as_ref()
            || effective_model(c, config.units.iter().find(|u| u.id == unit).unwrap())? != saved
        {
            return Err(Error::new("CONFIG_CHANGED", "捕获期间配置改变，请重新取样"));
        }
        Ok(samples)
    }
}
