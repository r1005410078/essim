//! 点位分组预览及一次创建IPC；不自动应用或运行。
use super::{Engine, dto::ErrorDto};
use tauri::State;
/// 使用实际接口枚举和当前已保存模型生成预览。
#[tauri::command]
pub async fn preview_partition(
    request: essim_core::application::commands::partition::PartitionRequest,
    engine: State<'_, Engine>,
) -> Result<essim_infrastructure::partition::Preview, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.preview_partition(request))
        .await
        .map_err(|_| ErrorDto::new("PARTITION_FAILED", "分组预览任务失败"))?
}
/// 原子提交一次预览；过期或源内容改变则要求重新规划。
#[tauri::command]
pub async fn create_partition(
    token: String,
    engine: State<'_, Engine>,
) -> Result<essim_core::application::commands::Receipt, ErrorDto> {
    let engine = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || engine.create_partition(&token))
        .await
        .map_err(|_| ErrorDto::new("PARTITION_FAILED", "分组创建任务失败"))?
}

/// 关闭预览时归还暂存资源。
#[tauri::command]
pub fn discard_partition(token: String, engine: State<'_, Engine>) -> Result<(), ErrorDto> {
    engine.discard_partition(&token)
}
