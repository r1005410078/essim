//! 配置用例协调：权威读取、处理器分派、原子提交和投影；不定义UI或协议执行器。
use super::{
    catalog::Catalog,
    commands::{
        device, endpoint, lifecycle, model, navigation, operations, Action, Command, Handled,
        Receipt,
    },
    event_handlers::WorkspaceProjectionListener,
    events::ConfigEvent,
    ports::{ApplyBoundary, ApplyState, ConfigRepository, StoppedBoundary},
    queries::{prepare_apply, ApplyPreview, Projection, WorkspaceView},
};
use crate::domain::{
    endpoint::EndpointConfig, error::Error, identity::EndpointId, navigation::Navigation,
};
use std::collections::BTreeMap;
/// 释放等待前校验通过的命令；不允许调用者修改规划内容。
pub struct ReleasePlan {
    command: Command,
    authority: Catalog,
    _staging: Box<dyn Send>,
    endpoint: EndpointConfig,
}
impl ReleasePlan {
    /// 完整释放范围，不以单元为停机边界。
    pub fn endpoint(&self) -> &EndpointConfig {
        &self.endpoint
    }
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
        let mut catalog = repository.load();
        // 只归一化可完整校验的旧导航投影；损坏目录保留原状，由写命令返回明确错误。
        // 权威目录仍在下一次配置事务中统一提交，不在构造时隐式写入存储。
        let _ = catalog.navigation.normalize_model_folders();
        Self {
            repository,
            boundary,
            application_states: BTreeMap::new(),
            sequence: 0,
            projection: Projection::new(catalog),
        }
    }
    /// 借用当前目录用于有界查询和准入估计，不深复制模型或点值。
    pub fn catalog_ref(&self) -> &Catalog {
        self.projection.query_catalog()
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
    /// 模型非法或存在未确认的不兼容类型时拒绝。
    pub fn preview_apply(&self, id: &EndpointId, reset: bool) -> Result<ApplyPreview, Error> {
        prepare_apply(self.projection.query_catalog(), id, reset)
    }
    /// 执行最小配置命令，校验后原子提交。
    /// # Errors
    /// 失效引用、非法配置、迁移失败或未接入能力时拒绝；失败不提交、不发布事件。
    pub fn execute(&mut self, command: Command) -> Result<(Receipt, ConfigEvent), Error> {
        self.execute_inner(command, false)
    }
    /// 按身份借用已保存模型，不深复制工作区或运行值。
    /// # Errors
    /// 模型身份不存在。
    pub fn saved_model(
        &self,
        id: &crate::domain::identity::ModelId,
    ) -> Result<&crate::domain::model::CollectionModel, Error> {
        self.projection
            .query_catalog()
            .models
            .iter()
            .find(|m| &m.id == id)
            .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "源模型不存在"))
    }
    /// 固定批量端点范围及相关定义，不复制完整目录和运行值。
    /// # Errors
    /// 身份、范围、并发或目标数量非法。
    pub fn prepare_batch(
        &self,
        id: String,
        operation: crate::domain::aggregates::batch::BatchOperation,
        scope: super::commands::batch_targets::BatchScope,
        concurrency: usize,
    ) -> Result<super::commands::batch_targets::BatchPlan, Error> {
        super::commands::batch_targets::prepare_batch(
            self.projection.query_catalog(),
            id,
            operation,
            scope,
            concurrency,
        )
    }
    /// 取得端点操作门后复核固定目标；不会重新扩大任务范围。
    /// # Errors
    /// 目标已删除、超出清单或相关定义已改变。
    pub fn revalidate_batch(
        &self,
        plan: &super::commands::batch_targets::BatchPlan,
        id: &EndpointId,
    ) -> Result<(), Error> {
        plan.revalidate(self.projection.query_catalog(), id)
    }
    /// 规划点位分组，不复制无关端点的运行值。
    /// # Errors
    /// 输入、源模型、目录或分配冲突时拒绝。
    pub fn prepare_partition(
        &self,
        request: super::commands::partition::PartitionRequest,
    ) -> Result<super::commands::partition::PartitionPlan, Error> {
        super::commands::partition::prepare_partition(self.projection.query_catalog(), request)
    }
    /// 在同一目录事务中创建全部分组；仓储失败不发布成功事实。
    /// # Errors
    /// 原内容变化、冲突或仓储提交失败；原目录保持不变。
    pub fn commit_partition(
        &mut self,
        plan: super::commands::partition::PartitionPlan,
        command_id: String,
    ) -> Result<(Receipt, ConfigEvent), Error> {
        if command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "命令身份不能为空"));
        }
        let batch = plan.request().batch_id.clone();
        let _staging = self.repository.reserve_staging()?;
        let mut catalog = self.repository.load();
        let endpoints = super::commands::partition::commit_partition(&mut catalog, plan)?;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("SEQUENCE_OVERFLOW", "通知序号耗尽"))?;
        catalog.navigation.normalize_model_folders()?;
        self.repository.commit(catalog.clone())?;
        let event = ConfigEvent {
            schema: 1,
            fact: super::events::ConfigurationFact::PartitionCreated {
                batch: batch.clone(),
                endpoints,
            },
            sequence,
            command_id,
            object_id: batch.clone(),
            catalog,
        };
        self.sequence = sequence;
        WorkspaceProjectionListener::handle(&mut self.projection, &event)?;
        Ok((
            Receipt {
                object_id: batch,
                sequence,
            },
            event,
        ))
    }
    /// 释放前读取与纯校验；返回后可释放目录锁，等待外部边界。
    /// # Errors
    /// 非应用／删除命令、目标不存在或完整迁移校验失败。
    pub fn prepare_release(&mut self, command: Command) -> Result<ReleasePlan, Error> {
        if command.command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "命令身份不能为空"));
        }
        let _staging = self.repository.reserve_staging()?;
        let authority = self.repository.load();
        let endpoint = match &command.action {
            Action::Apply {
                id,
                reset_incompatible,
            } => prepare_apply(&authority, id, *reset_incompatible)?.endpoint,
            Action::DeleteEndpoint { id } => authority
                .endpoints
                .iter()
                .find(|e| &e.id == id)
                .cloned()
                .ok_or_else(|| Error::new("NOT_FOUND", "模拟器不存在"))?,
            _ => return Err(Error::new("INVALID_COMMAND", "该命令不需要释放边界")),
        };
        self.application_states
            .insert(endpoint.id.clone(), ApplyState::Releasing);
        Ok(ReleasePlan {
            command,
            _staging,
            authority,
            endpoint,
        })
    }
    /// 释放后接回实际值并完成原规划，仍拒绝目标配置或依赖在等待期间改变。
    /// # Errors
    /// Unit／模型与原快照不符、目标变化、迁移或存储失败时拒绝；停止事实保留。
    pub fn finish_release_with_values(
        &mut self,
        mut plan: ReleasePlan,
        values: BTreeMap<crate::domain::identity::UnitId, crate::domain::values::UnitValues>,
    ) -> Result<(Receipt, ConfigEvent), Error> {
        self.observe_release(plan.endpoint.id.clone(), ApplyState::Stopped);
        for (id, value) in &values {
            if !plan
                .authority
                .applied
                .iter()
                .find(|endpoint| endpoint.id == plan.endpoint.id)
                .is_some_and(|endpoint| endpoint.units.iter().any(|unit| &unit.id == id))
                || plan
                    .authority
                    .values
                    .get(id)
                    .and_then(|old| old.model.as_ref())
                    != value.model.as_ref()
            {
                return Err(Error::new("CONFIG_CHANGED", "释放后的值与原已应用快照不符"));
            }
        }
        plan.authority.values.extend(values.clone());
        self.retain_stopped_values(&plan.endpoint.id, values)?;
        self.finish_release(plan)
    }
    /// 更新执行器返回的真实资源状态，独立于目录提交结果。
    pub fn observe_release(&mut self, id: EndpointId, state: ApplyState) {
        self.application_states.insert(id, state);
    }
    /// 释放确认后重新核对目标及依赖，将无关端点的并发修改合并保留。
    /// # Errors
    /// 目标／依赖改变、存储失败时保持旧应用和值；不回滚停止事实。
    pub fn finish_release(&mut self, plan: ReleasePlan) -> Result<(Receipt, ConfigEvent), Error> {
        self.application_states
            .insert(plan.endpoint.id.clone(), ApplyState::Stopped);
        let _staging = self.repository.reserve_staging()?;
        let current = self.repository.load();
        if release_scope(&current, &plan.endpoint.id)
            != release_scope(&plan.authority, &plan.endpoint.id)
        {
            self.projection.replace_catalog(current);
            return Err(Error::new(
                "CONFIG_CHANGED",
                "释放期间目标或引用已改变，请重新确认",
            ));
        }
        self.execute_inner(plan.command, true)
    }
    fn execute_inner(
        &mut self,
        command: Command,
        released: bool,
    ) -> Result<(Receipt, ConfigEvent), Error> {
        if command.command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "命令身份不能为空"));
        }
        let _staging = self.repository.reserve_staging()?;
        let mut catalog = self.repository.load();
        let mut observed_catalog = None;
        let mut lifecycle_context = lifecycle::LifecycleContext {
            boundary: &mut self.boundary,
            states: &mut self.application_states,
            repository: &self.repository,
            observed_catalog: &mut observed_catalog,
            released,
        };
        catalog.navigation.normalize_model_folders()?;
        let result = match command.action {
            Action::CreateInFolder{parent,resource}=>navigation::create_in_folder(&mut catalog,parent,resource),
            Action::MoveNavigation{nodes,scope,parent}=>navigation::move_nodes(&mut catalog,nodes,scope,parent),
            Action::CreateEndpointWithModel{endpoint,model}=>super::commands::overrides::create(&mut catalog,endpoint,model),
            Action::PromoteOverrides{plan}=>super::commands::overrides::promote(&mut catalog,plan),
            Action::ExtractUnitModel{endpoint,unit,id,name}=>super::commands::overrides::extract(&mut catalog,endpoint,unit,id,name),
            Action::CopyModel { source, id, name } => {
                model::copy_model(&mut catalog, source, id, name)
            }
            Action::DeleteModel { id } => model::delete_model(&mut catalog, id),
            Action::SaveModel { model } => model::save_model(&mut catalog, model),
            Action::CreateEndpoint { endpoint } => {
                endpoint::create_endpoint(&mut catalog, endpoint)
            }
            Action::SaveEndpoint { endpoint } => endpoint::save_endpoint(&mut catalog, endpoint),
            Action::CopyEndpoint { source, id } => {
                endpoint::copy_endpoint(&mut catalog, source, id)
            }
            Action::DeleteEndpoint { id } => {
                lifecycle::delete_endpoint(&mut catalog, id, &mut lifecycle_context)
            }
            Action::ResetUnit { id, unit } => {
                lifecycle::reset_unit(&mut catalog, id, unit, &mut lifecycle_context)
            }
            Action::SaveFolder { folder } => navigation::save_folder(&mut catalog, folder),
            Action::DeleteFolder { id } => navigation::delete_folder(&mut catalog, id),
            Action::MoveEndpoint { id, parent } => {
                navigation::move_endpoint(&mut catalog, id, parent)
            }
            Action::CreateDevice { device } => device::create_device(&mut catalog, device),
            Action::Apply {
                id,
                reset_incompatible,
            } => lifecycle::apply(&mut catalog, id, reset_incompatible, &mut lifecycle_context),
            Action::Start { id } => operations::start(&catalog, id),
            Action::ReadRemote { .. } | Action::WriteRemote { .. } | Action::Replay { .. } => {
                operations::unavailable()
            }
        };
        let handled = result.map_err(|error| {
            if let Some(authority) = observed_catalog {
                self.projection.replace_catalog(authority);
            }
            error
        })?;
        let Handled { object_id, fact } = handled;
        let sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| Error::new("SEQUENCE_OVERFLOW", "配置序号已耗尽"))?;
        catalog.navigation.normalize_model_folders()?;
        self.repository.commit(catalog.clone())?;
        let event = ConfigEvent {
            schema: 1,
            fact,
            sequence,
            command_id: command.command_id,
            object_id: object_id.clone(),
            catalog,
        };
        self.sequence = sequence;
        WorkspaceProjectionListener::handle(&mut self.projection, &event)?;
        Ok((
            Receipt {
                object_id,
                sequence,
            },
            event,
        ))
    }
    /// 仅复制目标端点的已应用配置与值，避免启动时读取整个工作区。
    /// # Errors
    /// 端点未应用或缺少Unit快照时失败。
    pub fn applied_values(
        &self,
        id: &EndpointId,
    ) -> Result<
        (
            EndpointConfig,
            BTreeMap<crate::domain::identity::UnitId, crate::domain::values::UnitValues>,
        ),
        Error,
    > {
        let catalog = self.projection.query_catalog();
        let endpoint = catalog
            .applied
            .iter()
            .find(|endpoint| &endpoint.id == id)
            .cloned()
            .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用模拟器"))?;
        let values = endpoint
            .units
            .iter()
            .map(|unit| {
                catalog
                    .values
                    .get(&unit.id)
                    .cloned()
                    .map(|value| (unit.id.clone(), value))
                    .ok_or_else(|| Error::new("NOT_APPLIED", "缺少Unit已应用值"))
            })
            .collect::<Result<_, _>>()?;
        Ok((endpoint, values))
    }
    /// 在执行器确认停止后接回实际运行值；不改变配置身份或发布配置事实。
    /// # Errors
    /// 目标未应用、Unit不属于目标、模型快照改变或存储失败时拒绝整组同步。
    pub fn retain_stopped_values(
        &mut self,
        id: &EndpointId,
        values: BTreeMap<crate::domain::identity::UnitId, crate::domain::values::UnitValues>,
    ) -> Result<(), Error> {
        let _staging = self.repository.reserve_staging()?;
        let mut catalog = self.repository.load();
        let endpoint = catalog
            .applied
            .iter()
            .find(|endpoint| &endpoint.id == id)
            .ok_or_else(|| Error::new("NOT_APPLIED", "端点没有已应用快照"))?;
        for (unit, value) in &values {
            if !endpoint.units.iter().any(|candidate| &candidate.id == unit)
                || catalog
                    .values
                    .get(unit)
                    .and_then(|current| current.model.as_ref())
                    != value.model.as_ref()
            {
                return Err(Error::new("CONFIG_CHANGED", "运行值与已应用模型不一致"));
            }
        }
        catalog.values.extend(values);
        catalog.navigation.normalize_model_folders()?;
        self.repository.commit(catalog.clone())?;
        self.projection.replace_catalog(catalog);
        self.application_states
            .insert(id.clone(), ApplyState::Stopped);
        Ok(())
    }
    /// 各端点独立应用并返回逐项结果，不承诺跨端点回滚。
    pub fn execute_batch(&mut self, commands: Vec<Command>) -> Vec<Result<Receipt, Error>> {
        commands
            .into_iter()
            .map(|command| self.execute(command).map(|(receipt, _)| receipt))
            .collect()
    }
}
fn release_scope(catalog: &Catalog, id: &EndpointId) -> Catalog {
    let endpoints: Vec<_> = catalog
        .endpoints
        .iter()
        .filter(|e| &e.id == id)
        .cloned()
        .collect();
    let applied: Vec<_> = catalog
        .applied
        .iter()
        .filter(|e| &e.id == id)
        .cloned()
        .collect();
    let units: std::collections::BTreeSet<_> = endpoints
        .iter()
        .chain(&applied)
        .flat_map(|e| e.units.iter().map(|u| &u.id))
        .collect();
    let models = endpoints
        .iter()
        .chain(&applied)
        .flat_map(|e| e.units.iter().map(|u| &u.model.id))
        .collect::<std::collections::BTreeSet<_>>();
    let devices = endpoints
        .iter()
        .chain(&applied)
        .flat_map(|e| e.units.iter().filter_map(|u| u.device.as_ref()))
        .collect::<std::collections::BTreeSet<_>>();
    let values = catalog
        .values
        .iter()
        .filter(|(id, _)| units.contains(id))
        .map(|(id, v)| (id.clone(), v.clone()))
        .collect();
    Catalog {
        models: catalog
            .models
            .iter()
            .filter(|m| models.contains(&m.id))
            .cloned()
            .collect(),
        devices: catalog
            .devices
            .iter()
            .filter(|d| devices.contains(&d.id))
            .cloned()
            .collect(),
        values,
        endpoints,
        applied,
        navigation: Navigation::default(),
    }
}
