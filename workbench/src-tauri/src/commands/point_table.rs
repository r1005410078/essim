//! 点表预览及本机文件适配；选择对话框与后台格式转换不进入业务组件。
use super::{dto::ErrorDto, Engine};
use crate::window_context::WindowState as State;
use essim_core::domain::error::Error;
use essim_infrastructure::point_exchange::{self, DraftInput, DraftPreview, InputFile, MAX_BYTES};
use std::{
    collections::HashMap,
    io::Read,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

/// 当前会话文件任务的取消令牌，不携带模型或协议状态。
#[derive(Default)]
pub struct PointFileJobs(Mutex<HashMap<String, Arc<AtomicBool>>>);
impl PointFileJobs {
    pub(crate) fn cancel_all(&self) {
        for flag in self.0.lock().unwrap().values() {
            flag.store(true, Ordering::Release);
        }
    }
}

/// 中止指定文件任务；后台在安全的行／表边界退出。
/// # Errors
/// 任务注册锁中毒时失败。
#[tauri::command]
pub fn cancel_point_file(job_id: String, jobs: State<'_, PointFileJobs>) -> Result<(), ErrorDto> {
    let jobs = jobs
        .0
        .lock()
        .map_err(|_| Error::new("FILE_TASK", "文件任务状态不可用"))?;
    if let Some(flag) = jobs.get(&job_id) {
        flag.store(true, Ordering::Release);
    }
    Ok(())
}
/// 在后台精确校验点表草稿，不改变配置目录。
/// # Errors
/// 后台任务无法执行时失败；字段错误放入预览结果。
#[tauri::command]
pub async fn preview_point_table(
    _access: super::project_access::ProjectAccess,
    input: DraftInput,
) -> Result<DraftPreview, ErrorDto> {
    tauri::async_runtime::spawn_blocking(move || point_exchange::preview(input))
        .await
        .map_err(|e| Error::new("PREVIEW_FAILED", e.to_string()))
}
/// 由系统选择本机点表文件，在后台有界读取和解析；取消返回None。
/// # Errors
/// 文件过大、读取或格式错误时失败。
#[tauri::command]
pub async fn open_point_file(
    window: tauri::WebviewWindow,
    _access: super::project_access::ProjectAccess,
    job_id: String,
    jobs: State<'_, PointFileJobs>,
) -> Result<Option<InputFile>, ErrorDto> {
    let flag = Arc::new(AtomicBool::new(false));
    {
        let mut entries = jobs
            .0
            .lock()
            .map_err(|_| Error::new("FILE_TASK", "文件任务状态不可用"))?;
        if entries.len() >= 8 || entries.contains_key(&job_id) {
            return Err(Error::new("FILE_TASK", "文件任务已存在或过多"));
        }
        entries.insert(job_id.clone(), flag.clone());
    }
    let result = async {
        let Some(file) = rfd::AsyncFileDialog::new()
            .set_parent(&window)
            .add_filter("点表", &["json", "csv", "xlsx"])
            .pick_file()
            .await
        else {
            return Ok(None);
        };
        let path = file.path().to_owned();
        tauri::async_runtime::spawn_blocking(move || {
            if flag.load(Ordering::Acquire) {
                return Err(Error::new("IMPORT_CANCELLED", "导入已取消"));
            }
            let metadata =
                std::fs::metadata(&path).map_err(|e| Error::new("FILE_READ", e.to_string()))?;
            if metadata.len() > MAX_BYTES as u64 {
                return Err(Error::new("IMPORT_LIMIT", "文件超过16MiB"));
            }
            let mut bytes = Vec::new();
            std::fs::File::open(&path)
                .map_err(|e| Error::new("FILE_READ", e.to_string()))?
                .take(MAX_BYTES as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| Error::new("FILE_READ", e.to_string()))?;
            point_exchange::read_file_cancellable(
                &path.file_name().unwrap_or_default().to_string_lossy(),
                &bytes,
                || flag.load(Ordering::Acquire),
            )
            .map(Some)
        })
        .await
        .map_err(|e| Error::new("FILE_READ", e.to_string()))?
    }
    .await;
    jobs.0
        .lock()
        .map_err(|_| Error::new("FILE_TASK", "文件任务状态不可用"))?
        .remove(&job_id);
    result
}
/// 导出权威目录中的当前已保存模型，不导出未保存草稿。
/// # Errors
/// 模型不存在、格式写出或文件系统操作失败。
#[tauri::command]
pub async fn export_point_file(
    window: tauri::WebviewWindow,
    _access: super::project_access::ProjectAccess,
    id: String,
    format: String,
    engine: State<'_, Engine>,
) -> Result<bool, ErrorDto> {
    let model = engine
        .query()?
        .catalog
        .models
        .into_iter()
        .find(|m| m.id.0 == id)
        .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "保存模型不存在"))?;
    save_exchange(&window, model, format).await
}
/// 显式输出空点表模板，不读取或修改模型目录。
/// # Errors
/// 格式非法或文件写出失败时返回错误；系统取消返回false。
#[tauri::command]
pub async fn export_point_template(
    window: tauri::WebviewWindow,
    _access: super::project_access::ProjectAccess,
    format: String,
) -> Result<bool, ErrorDto> {
    use essim_core::domain::{identity::ModelId, model::CollectionModel};
    save_exchange(
        &window,
        CollectionModel {
            id: ModelId("template".into()),
            name: "点表模板".into(),
            points: Vec::new(),
        },
        format,
    )
    .await
}
async fn save_exchange(
    window: &tauri::WebviewWindow,
    model: essim_core::domain::model::CollectionModel,
    format: String,
) -> Result<bool, ErrorDto> {
    if !["json", "csv", "xlsx"].contains(&format.as_str()) {
        return Err(Error::new("FILE_FORMAT", "未知导出格式"));
    }
    let Some(target) = rfd::AsyncFileDialog::new()
        .set_parent(&window)
        .add_filter("点表", &[&format])
        .set_file_name(format!("点表.{format}"))
        .save_file()
        .await
    else {
        return Ok(false);
    };
    let path = target.path().to_owned();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = point_exchange::export(&model, &format)?;
        point_exchange::write_export(&path, &bytes)?;
        Ok(true)
    })
    .await
    .map_err(|e| Error::new("FILE_WRITE", e.to_string()))?
}

/// 显式展平保存的 Unit 有效点表，不改变引用或覆盖。
/// # Errors
/// 引用缺失、覆盖非法、格式不支持、编码或文件写入失败。
#[tauri::command]
pub async fn export_unit_points(
    window: tauri::WebviewWindow,
    _access: super::project_access::ProjectAccess,
    endpoint: String,
    unit: String,
    format: String,
    engine: State<'_, Engine>,
) -> Result<bool, ErrorDto> {
    let e = engine.inner().clone();
    let model = tauri::async_runtime::spawn_blocking(move || {
        e.saved_effective_model(
            &essim_core::domain::identity::EndpointId(endpoint),
            &essim_core::domain::identity::UnitId(unit),
        )
    })
    .await
    .map_err(|_| ErrorDto::new("EXPORT_FAILED", "有效点表读取失败"))??;
    save_exchange(&window, model, format).await
}
