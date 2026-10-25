//! 配置命令和查询；保存、应用、真实启动使用不同业务操作。
use super::{
    dto::{Command, ErrorDto, Receipt, WorkspaceView},
    Engine,
};
use crate::window_context::WindowState as State;
/// 用户确认工程草稿保护后退出；窗口销毁不替代配置保存。
#[tauri::command]
pub(crate) async fn exit_workbench(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), ErrorDto> {
    crate::window_lifecycle::approve_window_exit(window, app).await
}
/// 窗口保护处理器安装完成后启用；加载失败时仍可正常关闭窗口。
#[tauri::command]
pub(crate) fn set_exit_guard_ready(ready: bool, permission: State<'_, crate::ExitPermission>) {
    permission
        .armed
        .store(ready, std::sync::atomic::Ordering::Release);
}
/// 查询真实会话目录及实际能力，不打开连接。
/// # Errors
/// 状态锁故障时失败。
#[tauri::command]
pub fn query_workspace(
    _access: super::project_access::ProjectAccess,
    engine: State<'_, Engine>,
) -> Result<WorkspaceView, ErrorDto> {
    engine.query_navigation()
}
/// 执行强类型配置命令，校验权威配置，不接受前端伪造的应用状态。
/// # Errors
/// 参数、引用、存储或未接入能力错误原样返回。
#[tauri::command]
pub async fn execute_workspace(
    _access: super::project_access::ProjectAccess,
    command: Command,
    engine: State<'_, Engine>,
    log: tauri::State<'_, crate::diagnostics::ApplicationLog>,
) -> Result<Receipt, ErrorDto> {
    let engine = engine.inner().clone();
    let operation = command.command_id.clone();
    let event = match &command.action {
        essim_core::application::commands::Action::SaveModel { .. } => "model_saved",
        essim_core::application::commands::Action::SaveEndpoint { .. } => "endpoint_saved",
        essim_core::application::commands::Action::Apply { .. } => "configuration_applied",
        _ => "configuration_result",
    };

    let result = tauri::async_runtime::spawn_blocking(move || engine.execute(command))
        .await
        .map_err(|_| ErrorDto::new("COMMAND_TASK_FAILED", "配置任务执行失败"))
        .and_then(|result| result);
    crate::diagnostics::result(&log.log, event, &operation, None, &result);
    result
}
/// 预览显式应用范围和单元迁移，不触发停机或目录写入。
/// # Errors
/// 配置非法、引用失效或存在未确认不兼容点时失败。
#[tauri::command]
pub fn preview_apply(
    _access: super::project_access::ProjectAccess,
    id: String,
    reset: bool,
    engine: State<'_, Engine>,
) -> Result<essim_core::application::queries::ApplyPreview, ErrorDto> {
    engine.preview_apply(essim_core::domain::identity::EndpointId(id), reset)
}

/// 只读比较模拟器草稿，不保存、停止或应用。
/// # Errors
/// 参数解析或配置状态锁失败。
#[tauri::command]
pub fn inspect_endpoint(
    _access: super::project_access::ProjectAccess,
    endpoint: essim_core::domain::endpoint::EndpointConfig,
    engine: State<'_, Engine>,
) -> Result<std::collections::BTreeMap<essim_core::domain::identity::UnitId, bool>, ErrorDto> {
    engine.inspect_endpoint(endpoint)
}

/// 按需读取完整保存模型。
#[tauri::command]
pub async fn load_model(
    _access: super::project_access::ProjectAccess,
    id: String,
    engine: State<'_, Engine>,
) -> Result<essim_core::domain::model::CollectionModel, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        e.load_model(&essim_core::domain::identity::ModelId(id))
    })
    .await
    .map_err(|_| ErrorDto::new("MODEL_QUERY_FAILED", "模型读取失败"))?
}
/// 按需读取目标端点已应用模型和值。
#[tauri::command]
pub async fn load_endpoint_values(
    _access: super::project_access::ProjectAccess,
    id: String,
    engine: State<'_, Engine>,
) -> Result<
    std::collections::BTreeMap<
        essim_core::domain::identity::UnitId,
        essim_core::domain::values::UnitValues,
    >,
    ErrorDto,
> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        e.load_endpoint_values(&essim_core::domain::identity::EndpointId(id))
    })
    .await
    .map_err(|_| ErrorDto::new("ENDPOINT_QUERY_FAILED", "端点读取失败"))?
}

/// 校验并解析单元覆盖草稿，不保存。
/// # Errors
/// 引用、覆盖校验、预算或后台任务失败。
#[tauri::command]
pub async fn resolve_unit(
    _access: super::project_access::ProjectAccess,
    unit: essim_core::domain::endpoint::UnitConfig,
    engine: State<'_, Engine>,
) -> Result<essim_core::domain::model::CollectionModel, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.resolve_unit(unit))
        .await
        .map_err(|_| ErrorDto::new("PREVIEW_FAILED", "有效配置查询失败"))?
}
/// 生成共享回写内容预览。
/// # Errors
/// 差异失效、字段依赖、共享引用冲突或预算不足。
#[tauri::command]
pub async fn preview_unit_promotion(
    _access: super::project_access::ProjectAccess,
    endpoint: String,
    unit: String,
    selection: essim_core::application::commands::overrides::OverrideSelection,
    engine: State<'_, Engine>,
) -> Result<essim_core::application::commands::overrides::PromotionPlan, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        e.preview_unit_promotion(
            essim_core::domain::identity::EndpointId(endpoint),
            essim_core::domain::identity::UnitId(unit),
            selection,
        )
    })
    .await
    .map_err(|_| ErrorDto::new("PREVIEW_FAILED", "回写预览失败"))?
}
/// 固定所选点确认值为默认初值预览。
/// # Errors
/// 选区非法、映射不一致、确认值失效或配置在取样期间改变。
#[tauri::command]
pub async fn capture_initials(
    _access: super::project_access::ProjectAccess,
    endpoint: String,
    unit: String,
    points: Vec<String>,
    engine: State<'_, Engine>,
) -> Result<Vec<essim_infrastructure::unit_overrides::InitialSample>, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        e.capture_initials(
            essim_core::domain::identity::EndpointId(endpoint),
            essim_core::domain::identity::UnitId(unit),
            points
                .into_iter()
                .map(essim_core::domain::identity::PointId)
                .collect(),
        )
    })
    .await
    .map_err(|_| ErrorDto::new("CAPTURE_FAILED", "运行值捕获失败"))?
}
