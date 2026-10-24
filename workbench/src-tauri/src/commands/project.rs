//! 工程生命周期 IPC；路径选择保留系统对话框。
use super::{dto::ErrorDto, Engine};
use essim_core::domain::error::Error;
use essim_infrastructure::project::{ProjectInfo, RecentProject};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;
/// 启动恢复错误保留给前端展示，不伪装成新空工程。
pub struct ProjectSession {
    pub recent: RecentProject,
    pub startup_error: Mutex<Option<String>>,
    pub operation: std::sync::atomic::AtomicBool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    project: Option<ProjectInfo>,
    startup_error: Option<String>,
    runtime_data: Option<essim_infrastructure::project::RuntimeDataView>,
}
#[tauri::command]
pub fn query_project(
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
) -> Result<ProjectStatus, ErrorDto> {
    Ok(ProjectStatus {
        runtime_data: engine.project_runtime_data(),
        project: engine.project_info()?,
        startup_error: session
            .startup_error
            .lock()
            .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))?
            .clone(),
    })
}
#[tauri::command]
pub async fn choose_project(
    action: String,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<Option<ProjectInfo>, ErrorDto> {
    let _guard = acquire(&session.operation)?;
    // 系统选择器异步执行；取消不触发工程切换或最近路径修改。
    let selected = match action.as_str() {
        "create" => {
            rfd::AsyncFileDialog::new()
                .set_title("选择空目录新建工程")
                .pick_folder()
                .await
        }
        "open" => {
            rfd::AsyncFileDialog::new()
                .set_title("打开工程 project.workspace")
                .add_filter("工程文件", &["workspace"])
                .pick_file()
                .await
        }
        _ => return Err(Error::new("PROJECT_ACTION", "未知工程操作")),
    };
    let Some(selected) = selected else {
        return Ok(None);
    };
    let path = selected.path().to_path_buf();
    let engine = engine.inner().clone();
    let batches = batches.inner().clone();
    let info = tauri::async_runtime::spawn_blocking(move || {
        batches.change_project(|| {
            if action == "create" {
                let name = path
                    .file_name()
                    .and_then(|v| v.to_str())
                    .unwrap_or("新工程");
                engine.create_project(&path, name)
            } else {
                engine.open_project(&path)
            }
        })
    })
    .await
    .map_err(|_| Error::new("PROJECT_TASK", "工程任务失败"))??;
    // 最近路径写入失败必须显式报告，工程已打开事实保留。
    let warning = session
        .recent
        .save(Some(&info.entry))
        .err()
        .map(|e| format!("工程已打开，但最近工程路径未保存：{e}"));
    *session
        .startup_error
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))? = warning;
    Ok(Some(info))
}
#[tauri::command]
pub async fn close_project(
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<(), ErrorDto> {
    let _guard = acquire(&session.operation)?;
    let e = engine.inner().clone();
    let b = batches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || b.change_project(|| e.close_project()))
        .await
        .map_err(|_| Error::new("PROJECT_TASK", "工程关闭任务失败"))??;
    let warning = session
        .recent
        .save(None)
        .err()
        .map(|e| format!("工程已关闭，但最近工程设置未清空：{e}"));
    *session
        .startup_error
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))? = warning;
    Ok(())
}
#[tauri::command]
pub async fn read_project_local(
    id: String,
    kind: String,
    engine: State<'_, Engine>,
) -> Result<Option<serde_json::Value>, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.read_project_local(&id, &kind))
        .await
        .map_err(|_| Error::new("PROJECT_TASK", "本机状态读取失败"))?
}
#[tauri::command]
pub async fn write_project_local(
    id: String,
    kind: String,
    value: serde_json::Value,
    engine: State<'_, Engine>,
) -> Result<(), ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.write_project_local(&id, &kind, &value))
        .await
        .map_err(|_| Error::new("PROJECT_TASK", "本机状态保存失败"))?
}

struct BusyReset<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for BusyReset<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
fn acquire(flag: &std::sync::atomic::AtomicBool) -> Result<BusyReset<'_>, Error> {
    flag.compare_exchange(
        false,
        true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    )
    .map_err(|_| Error::new("PROJECT_BUSY", "工程操作正在进行"))?;
    Ok(BusyReset(flag))
}

/// 工程文档只接受稳定身份和当前会话凭证，路径由基础设施解析。
#[tauri::command]
pub async fn load_project_document(
    session: String,
    id: String,
    engine: State<'_, Engine>,
) -> Result<String, ErrorDto> {
    let e = engine.inner().clone();
    tauri::async_runtime::spawn_blocking(move || e.read_project_document(&session, &id))
        .await
        .map_err(|_| Error::new("PROJECT_TASK", "文档读取任务失败"))?
}
