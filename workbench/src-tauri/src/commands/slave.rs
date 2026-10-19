//! 真实TCP从站IPC，运行操作不冒充配置提交。
use super::{dto::ErrorDto, Engine};
use essim_core::application::{
    commands::slave::SlaveCommand,
    queries::slave::{SlaveQuery, SlaveView},
};
use tauri::State;
/// 将真实运行命令交给固定线程所有者，不阻塞窗口线程。
/// # Errors
/// 参数、配置、监听、记录或执行器故障。
#[tauri::command]
pub async fn execute_slave(
    command: SlaveCommand,
    engine: State<'_, Engine>,
) -> Result<SlaveView, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.execute_slave(command))
        .await
        .map_err(|_| ErrorDto::new("RUNTIME_TASK_FAILED", "运行任务执行失败"))?
}
/// 按需查询运行摘要及有限点值，不返回全量配置目录。
/// # Errors
/// 查询过大、点非法或执行器故障。
#[tauri::command]
pub async fn query_slave(
    query: SlaveQuery,
    engine: State<'_, Engine>,
) -> Result<SlaveView, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.query_slave(query))
        .await
        .map_err(|_| ErrorDto::new("RUNTIME_TASK_FAILED", "运行查询任务失败"))?
}

/// 导出用户当前筛选的观测视图，区别于完整可靠运行记录。
/// # Errors
/// 超过8MiB、不是JSON或文件写入失败。
#[tauri::command]
pub async fn export_observations(content: String) -> Result<bool, ErrorDto> {
    if content.len() > 8 * 1024 * 1024
        || serde_json::from_str::<serde_json::Value>(&content).is_err()
    {
        return Err(ErrorDto::new(
            "OBSERVATION_EXPORT",
            "观测导出内容无效或超过8MiB",
        ));
    }
    let Some(target) = rfd::AsyncFileDialog::new()
        .add_filter("观测视图", &["json"])
        .set_file_name("通信观测.json")
        .save_file()
        .await
    else {
        return Ok(false);
    };
    tauri::async_runtime::spawn_blocking(move || std::fs::write(target.path(), content))
        .await
        .map_err(|_| ErrorDto::new("OBSERVATION_EXPORT", "导出任务失败"))?
        .map_err(|e| ErrorDto::new("OBSERVATION_EXPORT", e.to_string()))?;
    Ok(true)
}
