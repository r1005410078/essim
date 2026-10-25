//! 会话级批量命令／查询适配；窗口关闭不取消任务。
use super::dto::ErrorDto;
use crate::window_context::WindowState as State;
use essim_infrastructure::batch_lifecycle::{BatchCoordinator, DraftDisposition, LifecyclePreview};
use std::sync::Arc;
pub type Batches = Arc<BatchCoordinator>;
/// 固定范围并生成有界预览。
#[tauri::command]
pub async fn preview_batch(
    _access: super::project_access::ProjectAccess,
    request: essim_core::application::commands::batch_targets::BatchRequest,
    drafts: DraftDisposition,
    batches: State<'_, Batches>,
) -> Result<LifecyclePreview, ErrorDto> {
    let b = batches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || b.preview(request, drafts))
        .await
        .map_err(|_| ErrorDto::new("BATCH_FAILED", "批量预览失败"))?
}
/// 确认后开始后台执行。
#[tauri::command]
pub fn start_batch(
    _access: super::project_access::ProjectAccess,
    id: String,
    batches: State<'_, Batches>,
) -> Result<(), ErrorDto> {
    batches.start(&id)
}
/// 只读进度。
#[tauri::command]
pub fn query_batch(
    _access: super::project_access::ProjectAccess,
    batches: State<'_, Batches>,
) -> Result<Option<essim_core::application::queries::batch::BatchView>, ErrorDto> {
    batches.query()
}
/// 取消后续未开始项。
#[tauri::command]
pub fn cancel_batch(
    _access: super::project_access::ProjectAccess,
    id: String,
    batches: State<'_, Batches>,
) -> Result<(), ErrorDto> {
    batches.cancel(&id)
}
/// 仅对实际结束任务的失败项重新预览。
#[tauri::command]
pub async fn retry_batch(
    _access: super::project_access::ProjectAccess,
    previous: String,
    id: String,
    drafts: DraftDisposition,
    batches: State<'_, Batches>,
) -> Result<LifecyclePreview, ErrorDto> {
    let b = batches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || b.retry(&previous, id, drafts))
        .await
        .map_err(|_| ErrorDto::new("BATCH_FAILED", "重试预览失败"))?
}
/// 查询资源估计及当前进程RSS。
#[tauri::command]
pub async fn query_resources(
    _access: super::project_access::ProjectAccess,
    engine: State<'_, super::Engine>,
) -> Result<essim_infrastructure::resources::ResourceView, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.query_resources())
        .await
        .map_err(|_| ErrorDto::new("RESOURCE_QUERY_FAILED", "资源采样失败"))?
}

/// 关闭未确认预览只归还暂存，不取消已确认任务。
#[tauri::command]
pub fn discard_batch(
    _access: super::project_access::ProjectAccess,
    id: String,
    batches: State<'_, Batches>,
) -> Result<(), ErrorDto> {
    batches.discard_preview(&id)
}
