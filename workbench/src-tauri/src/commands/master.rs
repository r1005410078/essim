//! TCP主站IPC适配，业务规则属于core及运行所有者。
use super::{dto::ErrorDto, Engine};
use crate::window_context::WindowState as State;
use essim_core::application::{
    commands::master::MasterCommand,
    queries::master::{MasterQuery, MasterView},
};
/// 提交主站操作，不阻塞窗口线程。
/// # Errors
/// 参数、配置或运行服务不可用。
#[tauri::command]
pub async fn execute_master(
    _access: super::project_access::ProjectAccess,
    command: MasterCommand,
    engine: State<'_, Engine>,
    log: tauri::State<'_, crate::diagnostics::ApplicationLog>,
) -> Result<MasterView, ErrorDto> {
    let engine = engine.inner().clone();
    let operation = command.command_id.clone();
    let endpoint = command.endpoint.0.clone();

    let result = tauri::async_runtime::spawn_blocking(move || {
        engine.execute_master(command).map_err(ErrorDto::from)
    })
    .await
    .map_err(|_| ErrorDto::new("MASTER_TASK_FAILED", "主站任务失败"))
    .and_then(|result| result);
    crate::diagnostics::result(
        &log.log,
        "master_operation",
        &operation,
        Some(&endpoint),
        &result,
    );
    result
}
/// 查询主站轻量投影。
/// # Errors
/// 查询非法或服务不可用。
#[tauri::command]
pub async fn query_master(
    _access: super::project_access::ProjectAccess,
    query: MasterQuery,
    engine: State<'_, Engine>,
) -> Result<MasterView, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.query_master(query).map_err(ErrorDto::from))
        .await
        .map_err(|_| ErrorDto::new("MASTER_TASK_FAILED", "主站查询失败"))?
}
