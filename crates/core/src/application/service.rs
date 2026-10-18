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
        let catalog = repository.load();
        Self {
            repository,
            boundary,
            application_states: BTreeMap::new(),
            sequence: 0,
            projection: Projection::new(catalog),
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
    /// 模型非法或存在未确认的不兼容类型时拒绝。
    pub fn preview_apply(&self, id: &EndpointId, reset: bool) -> Result<ApplyPreview, Error> {
        prepare_apply(&self.repository.load(), id, reset)
    }
    /// 执行最小配置命令，校验后原子提交。
    /// # Errors
    /// 失效引用、非法配置、迁移失败或未接入能力时拒绝；失败不提交、不发布事件。
    pub fn execute(&mut self, command: Command) -> Result<(Receipt, ConfigEvent), Error> {
        self.execute_inner(command, false)
    }
    /// 释放前读取与纯校验；返回后可释放目录锁，等待外部边界。
    /// # Errors
    /// 非应用／删除命令、目标不存在或完整迁移校验失败。
    pub fn prepare_release(&mut self, command: Command) -> Result<ReleasePlan, Error> {
        if command.command_id.trim().is_empty() {
            return Err(Error::new("INVALID_COMMAND", "命令身份不能为空"));
        }
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
            authority,
            endpoint,
        })
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
        let mut catalog = self.repository.load();
        let mut observed_catalog = None;
        let mut lifecycle_context = lifecycle::LifecycleContext {
            boundary: &mut self.boundary,
            states: &mut self.application_states,
            repository: &self.repository,
            observed_catalog: &mut observed_catalog,
            released,
        };
        let result = match command.action {
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
