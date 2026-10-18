//! 配置命令和查询；保存、应用、真实启动使用不同业务操作。
use super::{
    dto::{Command, ErrorDto, Receipt, WorkspaceView},
    Engine,
};
use tauri::State;
/// 用户确认放弃本次会话后退出，不能把窗口销毁误当成配置已经落盘。
#[tauri::command]
pub(crate) fn exit_workbench(app: tauri::AppHandle, permission: State<'_, crate::ExitPermission>) {
    permission
        .confirmed
        .store(true, std::sync::atomic::Ordering::Release);
    app.exit(0);
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
pub fn query_workspace(engine: State<'_, Engine>) -> Result<WorkspaceView, ErrorDto> {
    engine.query()
}
/// 执行强类型配置命令，校验权威配置，不接受前端伪造的应用状态。
/// # Errors
/// 参数、引用、存储或未接入能力错误原样返回。
#[tauri::command]
pub async fn execute_workspace(
    command: Command,
    engine: State<'_, Engine>,
) -> Result<Receipt, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.execute(command))
        .await
        .map_err(|_| ErrorDto::new("COMMAND_TASK_FAILED", "配置任务执行失败"))?
}
/// 预览显式应用范围和单元迁移，不触发停机或目录写入。
/// # Errors
/// 配置非法、引用失效或存在未确认不兼容点时失败。
#[tauri::command]
pub fn preview_apply(
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
    endpoint: essim_core::domain::endpoint::EndpointConfig,
    engine: State<'_, Engine>,
) -> Result<std::collections::BTreeMap<essim_core::domain::identity::UnitId, bool>, ErrorDto> {
    engine.inspect_endpoint(endpoint)
}
