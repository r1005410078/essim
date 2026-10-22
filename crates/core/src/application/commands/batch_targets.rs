//! 固定批量目标并保留相关内容用于执行前复核；不读取或保留运行点值。
use crate::{
    application::{
        catalog::Catalog,
        commands::batch::BatchCommand,
        queries::batch_plan::{BatchPlanPreview, BatchTargetSummary},
    },
    domain::{
        aggregates::batch::BatchOperation,
        endpoint::EndpointConfig,
        error::Error,
        identity::{EndpointId, FolderId, ModelId, UnitId},
        model::CollectionModel,
    },
};
use serde::Deserialize;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
/// 明确的批量范围；不接受点表选区或当前Unit。
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum BatchScope {
    /// 用户显式选中的端点；保持首次出现顺序。
    Selected {
        /// 稳定端点身份。
        ids: Vec<EndpointId>,
    },
    /// 指定文件夹及全部后代文件夹。
    Folder {
        /// 稳定文件夹身份。
        id: FolderId,
    },
    /// 用户明确选择全部端点。
    All {},
}
/// 统一批量规划输入；默认并发4，全部端点必须显式选择All。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BatchRequest {
    /// 会话任务身份。
    pub id: String,
    /// 用户选择的生命周期操作。
    pub operation: BatchOperation,
    /// 明确目标范围。
    pub scope: BatchScope,
    /// 本次最多同时执行项，缺省4。
    #[serde(default = "default_concurrency")]
    pub concurrency: usize,
}
fn default_concurrency() -> usize {
    4
}
impl BatchRequest {
    /// 从当前目录准备固定计划，不保存配置或触发通信。
    /// # Errors
    /// 范围或执行参数非法。
    pub fn prepare(self, catalog: &Catalog) -> Result<BatchPlan, Error> {
        prepare_batch(
            catalog,
            self.id,
            self.operation,
            self.scope,
            self.concurrency,
        )
    }
}
struct TargetContent {
    saved: Option<EndpointConfig>,
    applied: Option<EndpointConfig>,
    saved_models: Vec<(ModelId, Option<Arc<CollectionModel>>)>,
    applied_models: Vec<(UnitId, Option<Arc<CollectionModel>>)>,
}
/// 一次任务的固定目标和相关定义；内部快照不向界面序列化。
pub struct BatchPlan {
    preview: BatchPlanPreview,
    contents: BTreeMap<EndpointId, TargetContent>,
    model_count: usize,
}
fn changed() -> Error {
    Error::new(
        "BATCH_TARGET_CHANGED",
        "端点或本次操作依赖的模型已改变，请重新预览该项",
    )
}
fn pool_model(
    pool: &mut BTreeMap<ModelId, Vec<Arc<CollectionModel>>>,
    model: Option<&CollectionModel>,
) -> Option<Arc<CollectionModel>> {
    model.map(|m| {
        let entries = pool.entry(m.id.clone()).or_default();
        if let Some(existing) = entries.iter().find(|v| v.as_ref() == m) {
            return existing.clone();
        }
        let m = Arc::new(m.clone());
        entries.push(m.clone());
        m
    })
}
fn resolve(catalog: &Catalog, scope: BatchScope) -> Result<Vec<EndpointId>, Error> {
    let mut unique = BTreeSet::new();
    for e in &catalog.endpoints {
        if !unique.insert(&e.id) {
            return Err(Error::new("DUPLICATE_ID", "端点身份重复，不能确定批量目标"));
        }
    }
    let targets = match scope {
        BatchScope::Selected { ids } => {
            let mut seen = BTreeSet::new();
            let mut result = Vec::new();
            for id in ids {
                if !unique.contains(&id) {
                    return Err(Error::new(
                        "ENDPOINT_NOT_FOUND",
                        format!("所选端点{}不存在", id.0),
                    ));
                }
                if seen.insert(id.clone()) {
                    result.push(id);
                }
            }
            result
        }
        BatchScope::All {} => catalog.endpoints.iter().map(|e| e.id.clone()).collect(),
        BatchScope::Folder { id } => {
            if !catalog.navigation.folders.iter().any(|f| f.id == id) {
                return Err(Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"));
            }
            let mut folders = BTreeSet::new();
            let mut queue = vec![id];
            while let Some(id) = queue.pop() {
                if !folders.insert(id.clone()) {
                    return Err(Error::new("FOLDER_CYCLE", "目标文件夹存在循环或重复身份"));
                }
                queue.extend(
                    catalog
                        .navigation
                        .folders
                        .iter()
                        .filter(|f| f.parent.as_ref() == Some(&id))
                        .map(|f| f.id.clone()),
                );
            }
            catalog
                .endpoints
                .iter()
                .filter(|e| {
                    catalog
                        .navigation
                        .members
                        .get(&e.id)
                        .is_some_and(|f| folders.contains(f))
                })
                .map(|e| e.id.clone())
                .collect()
        }
    };
    if targets.is_empty() || targets.len() > 1024 {
        return Err(Error::new(
            "BATCH_TARGET_LIMIT",
            "批量目标必须为1～1024个去重端点",
        ));
    }
    Ok(targets)
}
/// 规划固定目标；同一模型内容只保留一份比较副本，无运行值副本。
/// # Errors
/// 非法身份／并发、范围不存在、循环导航、空范围、目标超过1024或身份重复。
pub fn prepare_batch(
    catalog: &Catalog,
    id: String,
    operation: BatchOperation,
    scope: BatchScope,
    concurrency: usize,
) -> Result<BatchPlan, Error> {
    if id.trim().is_empty() || !(1..=8).contains(&concurrency) {
        return Err(Error::new("INVALID_BATCH", "任务身份或并发非法"));
    }
    let targets = resolve(catalog, scope)?;
    let mut contents = BTreeMap::new();
    let mut summaries = Vec::new();
    let mut pool = BTreeMap::new();
    for id in targets {
        let saved = catalog
            .endpoints
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| Error::new("ENDPOINT_NOT_FOUND", "端点不存在"))?;
        let applied = catalog.applied.iter().find(|e| e.id == id);
        let mut saved_models = Vec::new();
        let mut applied_models = Vec::new();
        if operation == BatchOperation::Apply {
            let models = saved
                .units
                .iter()
                .map(|u| u.model.id.clone())
                .collect::<BTreeSet<_>>();
            for id in models {
                let model = catalog.models.iter().find(|m| m.id == id);
                saved_models.push((id, pool_model(&mut pool, model)));
            }
        }
        if operation != BatchOperation::Stop {
            if let Some(applied) = applied {
                for unit in &applied.units {
                    let model = catalog
                        .values
                        .get(&unit.id)
                        .and_then(|v| v.model.as_deref());
                    applied_models.push((unit.id.clone(), pool_model(&mut pool, model)));
                }
            }
        }
        summaries.push(BatchTargetSummary {
            endpoint: id.clone(),
            name: saved.name.clone(),
            role: saved.role,
            units: saved.units.len(),
            applied: applied.is_some(),
            wait_ms: match operation {
                BatchOperation::Apply => {
                    if saved.role == crate::domain::endpoint::Role::Master
                        || applied.is_some_and(|e| e.role == crate::domain::endpoint::Role::Master)
                    {
                        65_000
                    } else {
                        10_000
                    }
                }
                BatchOperation::Start => applied
                    .filter(|e| e.role == crate::domain::endpoint::Role::Master)
                    .map_or(10_000, |e| {
                        e.master.connect_timeout_ms.saturating_add(10_000)
                    }),
                BatchOperation::Stop => {
                    if applied.is_some_and(|e| e.role == crate::domain::endpoint::Role::Master) {
                        65_000
                    } else {
                        10_000
                    }
                }
            },
        });
        contents.insert(
            id,
            TargetContent {
                saved: (operation == BatchOperation::Apply).then(|| saved.clone()),
                applied: applied.cloned(),
                saved_models,
                applied_models,
            },
        );
    }
    Ok(BatchPlan {
        preview: BatchPlanPreview {
            id,
            operation,
            concurrency,
            targets: summaries,
        },
        contents,
        model_count: pool.values().map(Vec::len).sum(),
    })
}
impl BatchPlan {
    /// 返回不含模型和点值的固定摘要。
    pub fn preview(&self) -> &BatchPlanPreview {
        &self.preview
    }
    /// 固定目标；后续目录移动或新增不改变此集合。
    pub fn targets(&self) -> Vec<EndpointId> {
        self.preview
            .targets
            .iter()
            .map(|t| t.endpoint.clone())
            .collect()
    }
    /// 当前计划保留的不同模型内容数量；不是配置版本数。
    pub fn retained_model_count(&self) -> usize {
        self.model_count
    }
    /// 转为任务状态命令；真实执行器仍须对每项调用revalidate。
    pub fn start_command(&self) -> BatchCommand {
        BatchCommand::Start {
            id: self.preview.id.clone(),
            operation: self.preview.operation,
            targets: self.targets(),
            concurrency: self.preview.concurrency,
        }
    }
    /// 在取得端点操作门后、实际动作前复核本项；不重解析范围。
    /// # Errors
    /// 目标不在计划、已删除，或相关保存／应用配置及模型内容发生变化。
    pub fn revalidate(&self, catalog: &Catalog, id: &EndpointId) -> Result<(), Error> {
        let expected = self
            .contents
            .get(id)
            .ok_or_else(|| Error::new("BATCH_TARGET_OUTSIDE_PLAN", "端点不在本次固定清单中"))?;
        let saved = catalog
            .endpoints
            .iter()
            .find(|e| &e.id == id)
            .ok_or_else(|| Error::new("BATCH_TARGET_DELETED", "目标端点已删除"))?;
        let applied = catalog.applied.iter().find(|e| &e.id == id);
        if expected.saved.as_ref().is_some_and(|s| s != saved)
            || expected.applied.as_ref() != applied
        {
            return Err(changed());
        }
        for (id, model) in &expected.saved_models {
            if catalog.models.iter().find(|m| &m.id == id) != model.as_deref() {
                return Err(changed());
            }
        }
        for (unit, model) in &expected.applied_models {
            if catalog.values.get(unit).and_then(|v| v.model.as_deref()) != model.as_deref() {
                return Err(changed());
            }
        }
        Ok(())
    }
}
