//! 配置命令以权威目录校验；投影只用于查询，不反向覆盖配置。
use crate::domain::foundation::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// 会话内权威配置目录；模型保存版本不可变。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    /// 不可变模型版本集合。
    pub models: Vec<CollectionModel>,
    /// 当前保存的端点聚合。
    pub endpoints: Vec<EndpointConfig>,
    /// 可选设备实体。
    pub devices: Vec<Device>,
    /// 已应用的配置副本，保存新配置不覆盖。
    pub applied: Vec<EndpointConfig>,
    /// 会话内静止实例值；真实运行尚未接入。
    pub values: BTreeMap<UnitId, UnitValues>,
}
/// 某个模型版本的端点引用；配置和已应用状态分别计数。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    /// 是否来自已应用配置。
    pub applied: bool,
    /// 引用端点身份。
    pub endpoint: EndpointId,
    /// 引用单元身份。
    pub unit: UnitId,
    /// 完整模型版本引用。
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
/// 配置存储窄端口；提交必须原子成功或保持旧值。
pub trait ConfigRepository {
    /// 获取权威目录副本。
    fn load(&self) -> Catalog;
    /// 原子提交完整目录；不得在此持有协议资源。
    /// # Errors
    /// 存储不可用时失败，不能半写入。
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error>;
    /// 由可靠运行记录保留的模型版本；内存配置实现无此类引用。
    /// 持久化实现须在同一提交边界维护引用，不能并发绕过删除校验。
    fn retained_models(&self) -> Vec<ModelRef> {
        Vec::new()
    }
}
/// 命令携带因果标识；更新必须提供期望版本。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    /// 调用者生成的命令身份，用于关联结果和事件。
    pub command_id: String,
    /// 强类型业务操作。
    pub action: Action,
}
/// 最小配置命令及尚未接入的真实运行入口。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// 复制选定保存版本，生成独立模型与点身份。
    CopyModel {
        /// 源保存版本。
        source: ModelRef,
        /// 新模型稳定身份，必须尚未使用。
        id: ModelId,
        /// 副本名称。
        name: String,
        /// 源模型最新版本，用于拒绝过期操作。
        #[serde(rename = "expectedRevision", with = "decimal")]
        expected_revision: u64,
    },
    /// 无引用时删除模型全部会话内版本。
    DeleteModel {
        /// 模型稳定身份。
        id: ModelId,
        /// 最新保存版本。
        #[serde(rename = "expectedRevision", with = "decimal")]
        expected_revision: u64,
    },
    /// 新增或保存模型；新建期望版本为0。
    SaveModel {
        /// 完整模型草稿，提交时分配下一版本。
        model: CollectionModel,
        /// 当前已保存版本。
        #[serde(rename = "expectedRevision", with = "decimal")]
        expected_revision: u64,
    },
    /// 新增或保存端点；修改不会自动应用。
    SaveEndpoint {
        /// 聚合草稿。
        endpoint: EndpointConfig,
        /// 当前已保存版本。
        #[serde(rename = "expectedRevision", with = "decimal")]
        expected_revision: u64,
    },
    /// 新增可选设备资源。
    CreateDevice {
        /// 待创建实体。
        device: Device,
    },
    /// 原子应用一个端点的全部单元。
    Apply {
        /// 目标端点。
        id: EndpointId,
        /// 期望保存版本。
        #[serde(rename = "expectedRevision", with = "decimal")]
        expected_revision: u64,
        /// 显式允许不兼容点恢复初值。
        #[serde(rename = "resetIncompatible")]
        reset_incompatible: bool,
    },
    /// 真实协议启动，M01生产装配明确拒绝。
    Start {
        /// 目标端点。
        id: EndpointId,
    },
    /// 真实读取，绝不是查询缓存。
    ReadRemote {
        /// 目标端点。
        id: EndpointId,
    },
    /// 对外写入；未接入执行器和可靠录制时拒绝。
    WriteRemote {
        /// 目标端点。
        id: EndpointId,
    },
    /// 加载真实历史回放；M01仅提供契约测试。
    Replay {
        /// 原运行身份。
        run: RunId,
    },
}
/// 配置领域事件；不冒充实际通信或运行事实。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigEvent {
    /// 事件数据版本，当前为1。
    pub schema: u32,
    /// 全局配置通知序号。
    #[serde(with = "decimal")]
    pub sequence: u64,
    /// 命令因果身份。
    pub command_id: String,
    /// 稳定对象身份。
    pub object_id: String,
    /// 接受后的对象版本。
    #[serde(with = "decimal")]
    pub revision: u64,
    /// 当前完整配置投影，用于M01小目录恢复；不是运行历史存储。
    pub catalog: Catalog,
}
/// 命令确认，和投影位置关联。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// 修改的稳定对象。
    pub object_id: String,
    /// 新版本。
    #[serde(with = "decimal")]
    pub revision: u64,
    /// 查询可等待的配置通知位置。
    #[serde(with = "decimal")]
    pub sequence: u64,
}
/// 实际生产能力信息；不根据底层库已编译推断可运行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// 最小配置操作可用。
    pub configuration: bool,
    /// 生产协议执行器是否接入。
    pub execution: bool,
    /// 可靠录制是否接入。
    pub recording: bool,
    /// 可用历史回放是否接入。
    pub replay: bool,
    /// 当前存储是否仅本次会话。
    pub session_only: bool,
    /// 禁用运行操作的具体原因。
    pub reason: String,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            configuration: true,
            execution: false,
            recording: false,
            replay: false,
            session_only: true,
            reason: "尚未接入正式协议执行器和可靠录制；当前只能配置，不能运行或回放".into(),
        }
    }
}
/// 前端可订阅的只读查询结果。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceView {
    /// 停机边界已确认的状态；配置提交失败不能把它回滚成运行中。
    #[serde(default)]
    pub application_states: BTreeMap<EndpointId, ApplyState>,
    /// 最后已观察到的配置通知序号。
    #[serde(with = "decimal")]
    pub sequence: u64,
    /// 只读配置目录副本。
    pub catalog: Catalog,
    /// 实际能力。
    pub capabilities: Capabilities,
}
/// 可重建投影；拒绝缺口和未知数据版本。
#[derive(Debug, Clone, Default)]
pub struct Projection {
    sequence: u64,
    catalog: Catalog,
}
impl Projection {
    /// 应用有序配置事件，重复事件幂等忽略。
    /// # Errors
    /// 未知schema或序号缺口时要求重新查询完整快照。
    pub fn accept(&mut self, event: &ConfigEvent) -> Result<bool, Error> {
        if event.schema != 1 {
            return Err(Error::new(
                "UNKNOWN_SCHEMA",
                "未知事件版本，请重新获取完整快照",
            ));
        }
        if event.sequence <= self.sequence {
            return Ok(false);
        }
        if event.sequence != self.sequence + 1 {
            return Err(Error::new(
                "SEQUENCE_GAP",
                "配置通知缺口，请重新获取完整快照",
            ));
        }
        self.sequence = event.sequence;
        self.catalog = event.catalog.clone();
        Ok(true)
    }
    /// 获取独立只读副本，不触发通信。
    pub fn query(&self) -> WorkspaceView {
        WorkspaceView {
            application_states: BTreeMap::new(),
            sequence: self.sequence,
            catalog: self.catalog.clone(),
            capabilities: Capabilities::default(),
        }
    }
}
/// 配置应用时的整组停机边界；一个TCP监听端点或RTU串口通道是最小范围。
pub trait ApplyBoundary {
    /// 确认整个端点停止，不能只停一个Unit。成功后不自动启动。
    /// # Errors
    /// 未确认停止时失败，调用者不得提交新映射。
    fn stop(&mut self, endpoint: &EndpointConfig) -> Result<(), Error>;
}
/// 无生产执行器时所有端点本来就未运行；不声明获得真实停机证据。
#[derive(Default)]
pub struct StoppedBoundary;
impl ApplyBoundary for StoppedBoundary {
    fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
        Ok(())
    }
}
/// 停机确认状态与配置提交结果分开，后续提交失败不恢复成运行中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyState {
    /// 整个端点已经由边界确认停止。
    Stopped,
}
/// 显式应用前的整组迁移预览，不更改目录或运行值。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyPreview {
    /// 将应用的保存配置，包括完整端点／通道和全部Unit。
    pub endpoint: EndpointConfig,
    /// 每个单元独立的迁移规划。
    pub units: BTreeMap<UnitId, Migration>,
}
/// 配置应用服务；不持有C句柄或窗口。
pub struct Workspace<R: ConfigRepository, B: ApplyBoundary = StoppedBoundary> {
    boundary: B,
    application_states: BTreeMap<EndpointId, ApplyState>,
    repository: R,
    sequence: u64,
    projection: Projection,
}
impl<R: ConfigRepository> Workspace<R> {
    /// 从权威目录创建初始投影，真实执行器仍未装配。
    pub fn new(repository: R) -> Self {
        Self::with_boundary(repository, StoppedBoundary)
    }
}
impl<R: ConfigRepository, B: ApplyBoundary> Workspace<R, B> {
    /// 获取通知位置，不复制完整模型目录，供低频配置订阅检查变化。
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    /// 装配受控停机边界；生产适配器须确认完整资源范围停止。
    pub fn with_boundary(repository: R, boundary: B) -> Self {
        let catalog = repository.load();
        Self {
            repository,
            boundary,
            application_states: BTreeMap::new(),
            sequence: 0,
            projection: Projection {
                sequence: 0,
                catalog,
            },
        }
    }
    /// 查询投影，不读取远端寄存器。
    pub fn query(&self) -> WorkspaceView {
        let mut view = self.projection.query();
        view.application_states = self.application_states.clone();
        view
    }
    /// 获取独立于配置提交的停机确认；None表示尚未通过该边界确认。
    pub fn application_state(&self, id: &EndpointId) -> Option<ApplyState> {
        self.application_states.get(id).copied()
    }
    /// 预览完整端点的应用范围及所有单元迁移，不停机、不提交。
    /// # Errors
    /// 保存版本失效、模型非法或存在未确认的不兼容类型时拒绝。
    pub fn preview_apply(
        &self,
        id: &EndpointId,
        revision: u64,
        reset: bool,
    ) -> Result<ApplyPreview, Error> {
        prepare_apply(&self.repository.load(), id, revision, reset)
    }
    /// 执行最小配置命令，校验后原子提交。
    /// # Errors
    /// 版本冲突、失效引用、非法配置、迁移失败或未接入能力时拒绝；失败不提交、不发布事件。
    pub fn execute(&mut self, command: Command) -> Result<(Receipt, ConfigEvent), Error> {
        if command.command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "命令身份不能为空"));
        }
        let mut catalog = self.repository.load();
        let (object_id, revision) = match command.action {
            Action::CopyModel {
                source,
                id,
                name,
                expected_revision,
            } => {
                let latest = catalog
                    .models
                    .iter()
                    .filter(|m| m.id == source.id)
                    .map(|m| m.version)
                    .max()
                    .unwrap_or(0);
                check_revision(latest, expected_revision)?;
                if id.0.trim().is_empty() || catalog.models.iter().any(|m| m.id == id) {
                    return Err(Error::new("DUPLICATE_ID", "新模型身份为空或已使用"));
                }
                let mut model = find_model(&catalog, &source)?.clone();
                model.id = id.clone();
                model.name = name;
                model.version = 1;
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
                (id.0, 1)
            }
            Action::DeleteModel {
                id,
                expected_revision,
            } => {
                let latest = catalog
                    .models
                    .iter()
                    .filter(|m| m.id == id)
                    .map(|m| m.version)
                    .max()
                    .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "模型不存在"))?;
                check_revision(latest, expected_revision)?;
                let usages = catalog.model_references(&id);
                if !usages.is_empty()
                    || self.repository.retained_models().iter().any(|m| m.id == id)
                {
                    return Err(Error::new(
                        "MODEL_REFERENCED",
                        format!(
                            "模型仍被配置、已应用状态或运行记录引用：{}",
                            usages
                                .iter()
                                .map(|r| format!(
                                    "{}→{}→v{}",
                                    r.endpoint.0, r.unit.0, r.model.version
                                ))
                                .collect::<Vec<_>>()
                                .join("，")
                        ),
                    ));
                }
                catalog.models.retain(|m| m.id != id);
                (id.0, latest)
            }
            Action::SaveModel {
                mut model,
                expected_revision,
            } => {
                let current = catalog
                    .models
                    .iter()
                    .filter(|old| old.id == model.id)
                    .map(|old| old.version)
                    .max()
                    .unwrap_or(0);
                check_revision(current, expected_revision)?;
                model.version = current
                    .checked_add(1)
                    .ok_or_else(|| Error::new("VERSION_OVERFLOW", "模型版本已耗尽"))?;
                model.validate()?;
                let result = (model.id.0.clone(), model.version);
                catalog.models.push(model);
                result
            }
            Action::SaveEndpoint {
                mut endpoint,
                expected_revision,
            } => {
                let old = catalog.endpoints.iter().find(|old| old.id == endpoint.id);
                check_revision(old.map(|old| old.revision).unwrap_or(0), expected_revision)?;
                endpoint.revision = expected_revision
                    .checked_add(1)
                    .ok_or_else(|| Error::new("VERSION_OVERFLOW", "配置版本已耗尽"))?;
                endpoint.applied_revision = old.and_then(|old| old.applied_revision);
                endpoint.validate()?;
                for unit in &endpoint.units {
                    find_model(&catalog, &unit.model)?;
                    if unit
                        .device
                        .as_ref()
                        .is_some_and(|id| !catalog.devices.iter().any(|device| &device.id == id))
                    {
                        return Err(Error::new("DEVICE_NOT_FOUND", "关联设备不存在"));
                    }
                    if catalog
                        .endpoints
                        .iter()
                        .filter(|other| other.id != endpoint.id)
                        .any(|other| other.units.iter().any(|other| other.id == unit.id))
                    {
                        return Err(Error::new("DUPLICATE_ID", "单元身份已属于其他端点"));
                    }
                }
                let result = (endpoint.id.0.clone(), endpoint.revision);
                catalog.endpoints.retain(|old| old.id != endpoint.id);
                catalog.endpoints.push(endpoint);
                result
            }
            Action::CreateDevice { device } => {
                if device.id.0.is_empty()
                    || device.name.trim().is_empty()
                    || device.kind.trim().is_empty()
                    || catalog.devices.iter().any(|old| old.id == device.id)
                {
                    return Err(Error::new("INVALID_DEVICE", "设备身份重复或名称／类型为空"));
                }
                let result = (device.id.0.clone(), 1);
                catalog.devices.push(device);
                result
            }
            Action::Apply {
                id,
                expected_revision,
                reset_incompatible,
            } => {
                let preview = prepare_apply(&catalog, &id, expected_revision, reset_incompatible)?;
                let mut endpoint = preview.endpoint;
                self.boundary.stop(&endpoint)?;
                self.application_states
                    .insert(id.clone(), ApplyState::Stopped);
                // 停机期间可能等待外部适配器，重新比较权威目录，不能覆盖较新配置。
                let authority = self.repository.load();
                if authority != catalog {
                    // 外部提交不是本命令成功事件；查询重新对齐权威配置，已应用值不回滚。
                    self.projection.catalog = authority;
                    return Err(Error::new(
                        "REVISION_CONFLICT",
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
                endpoint.applied_revision = Some(endpoint.revision);
                catalog.applied.retain(|old| old.id != id);
                catalog.applied.push(endpoint.clone());
                *catalog
                    .endpoints
                    .iter_mut()
                    .find(|old| old.id == id)
                    .ok_or_else(|| Error::new("NOT_FOUND", "端点不存在"))? = endpoint;
                (id.0, expected_revision)
            }
            Action::Start { .. }
            | Action::ReadRemote { .. }
            | Action::WriteRemote { .. }
            | Action::Replay { .. } => {
                return Err(Error::new(
                    "CAPABILITY_UNAVAILABLE",
                    Capabilities::default().reason,
                ))
            }
        };
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("SEQUENCE_OVERFLOW", "配置序号已耗尽"))?;
        self.repository.commit(catalog.clone())?;
        let event = ConfigEvent {
            schema: 1,
            sequence,
            command_id: command.command_id,
            object_id: object_id.clone(),
            revision,
            catalog,
        };
        self.sequence = sequence;
        self.projection.accept(&event)?;
        Ok((
            Receipt {
                object_id,
                revision,
                sequence,
            },
            event,
        ))
    }
    /// 各端点独立应用并返回逐项结果，不承诺跨端点回滚。
    pub fn execute_batch(&mut self, commands: Vec<Command>) -> Vec<Result<Receipt, Error>> {
        commands
            .into_iter()
            .map(|command| self.execute(command).map(|(receipt, _)| receipt))
            .collect()
    }
}
fn check_revision(current: u64, expected: u64) -> Result<(), Error> {
    if current != expected {
        Err(Error::new(
            "REVISION_CONFLICT",
            format!("期望版本{expected}，实际版本{current}"),
        ))
    } else {
        Ok(())
    }
}
fn find_model<'a>(
    catalog: &'a Catalog,
    reference: &ModelRef,
) -> Result<&'a CollectionModel, Error> {
    catalog
        .models
        .iter()
        .find(|model| model.id == reference.id && model.version == reference.version)
        .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "引用的模型版本不存在"))
}

fn prepare_apply(
    catalog: &Catalog,
    id: &EndpointId,
    revision: u64,
    reset: bool,
) -> Result<ApplyPreview, Error> {
    let endpoint = catalog
        .endpoints
        .iter()
        .find(|e| &e.id == id)
        .cloned()
        .ok_or_else(|| Error::new("NOT_FOUND", "端点不存在"))?;
    check_revision(endpoint.revision, revision)?;
    endpoint.validate()?;
    let mut units = BTreeMap::new();
    for unit in &endpoint.units {
        let next = find_model(catalog, &unit.model)?;
        let current = catalog.values.get(&unit.id).cloned().unwrap_or_default();
        let old = current
            .model
            .as_ref()
            .map(|r| find_model(catalog, r))
            .transpose()?;
        let mut migration = migrate(old, &current, next, reset || endpoint.role == Role::Master)?;
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
            migration.values.stale = current.stale
                || !same_source
                || !unit.enabled
                || migration.values.raw.len() != next.points.len()
                || old.is_none();
        }
        units.insert(unit.id.clone(), migration);
    }
    Ok(ApplyPreview { endpoint, units })
}
