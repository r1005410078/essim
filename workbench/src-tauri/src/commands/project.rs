//! 工程生命周期 IPC；路径选择保留系统对话框。
use super::{dto::ErrorDto, Engine};
use essim_core::domain::error::Error;
use essim_infrastructure::project::{ProjectInfo, ProjectRepository, RecentEntry, RecentProject};
use serde::Serialize;
use std::sync::Mutex;
use tauri::State;
/// 启动恢复错误保留给前端展示，不伪装成新空工程。
pub struct ProjectSession {
    pub recent: RecentProject,
    pub startup_error: Mutex<Option<String>>,
    pub operation: std::sync::atomic::AtomicBool,
    pub prepared: Mutex<Option<Prepared>>,
}
pub struct Prepared {
    token: String,
    target: PreparedTarget,
}
enum PreparedTarget {
    Open(ProjectRepository),
    Create(std::path::PathBuf),
    Same(ProjectInfo),
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Preparation {
    token: String,
    name: String,
    same: bool,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectStatus {
    project: Option<ProjectInfo>,
    recent: Vec<RecentEntry>,
    requires_stop: bool,
    startup_error: Option<String>,
    runtime_data: Option<essim_infrastructure::project::RuntimeDataView>,
}
#[tauri::command]
pub fn query_project(
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
) -> Result<ProjectStatus, ErrorDto> {
    let recent = session.recent.entries();
    let recent_error = recent.as_ref().err().map(ToString::to_string);
    Ok(ProjectStatus {
        recent: recent.unwrap_or_default(),
        requires_stop: requires_stop(&engine),
        runtime_data: engine.project_runtime_data(),
        project: engine.project_info()?,
        startup_error: session
            .startup_error
            .lock()
            .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))?
            .clone()
            .or(recent_error),
    })
}
#[tauri::command]
pub async fn prepare_project(
    window: tauri::WebviewWindow,
    action: String,
    entry: Option<std::path::PathBuf>,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
) -> Result<Option<Preparation>, ErrorDto> {
    let _guard = acquire(&session.operation)?;
    session
        .prepared
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))?
        .take();
    let path = if let Some(entry) = entry {
        if action != "open" || !session.recent.entries()?.iter().any(|r| r.entry == entry) {
            return Err(Error::new("PROJECT_ENTRY", "最近工程入口无效"));
        }
        entry
    } else {
        let selected = match action.as_str() {
            "create" => {
                rfd::AsyncFileDialog::new()
                    .set_parent(&window)
                    .set_can_create_directories(true)
                    .set_title("选择空目录新建工程")
                    .pick_folder()
                    .await
            }
            "open" => {
                rfd::AsyncFileDialog::new()
                    .set_parent(&window)
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
        selected.path().to_path_buf()
    };
    let current = engine.project_info()?;
    let (target, name, same) = tauri::async_runtime::spawn_blocking(move || -> Result<_, Error> {
        if action == "create" {
            let path = path
                .canonicalize()
                .map_err(|e| Error::new("PROJECT_IO", e.to_string()))?;
            if std::fs::read_dir(&path)
                .map_err(|e| Error::new("PROJECT_IO", e.to_string()))?
                .next()
                .is_some()
            {
                return Err(Error::new("PROJECT_NOT_EMPTY", "请选择空目录新建工程"));
            }
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            Ok((PreparedTarget::Create(path), name, false))
        } else {
            if let Some(info) = current {
                if path.canonicalize().ok().as_ref() == Some(&info.entry) {
                    let name = info.name.clone();
                    return Ok((PreparedTarget::Same(info), name, true));
                }
            }
            let repository = ProjectRepository::open(&path)?;
            let name = repository.manifest().display_name.clone();
            Ok((PreparedTarget::Open(repository), name, false))
        }
    })
    .await
    .map_err(|_| Error::new("PROJECT_TASK", "工程预检任务失败"))??;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    let token = format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    );
    *session
        .prepared
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))? = Some(Prepared {
        token: token.clone(),
        target,
    });
    Ok(Some(Preparation { token, name, same }))
}
#[tauri::command]
pub fn cancel_prepared_project(
    token: String,
    session: State<'_, ProjectSession>,
) -> Result<(), ErrorDto> {
    let _guard = acquire(&session.operation)?;
    let mut pending = session
        .prepared
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))?;
    if pending.as_ref().is_some_and(|p| p.token == token) {
        pending.take();
    }
    Ok(())
}
#[tauri::command]
pub async fn commit_project(
    token: String,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<ProjectInfo, ErrorDto> {
    let _guard = acquire(&session.operation)?;
    let prepared = {
        let mut pending = session
            .prepared
            .lock()
            .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))?;
        if pending.as_ref().is_none_or(|p| p.token != token) {
            return Err(Error::new("PROJECT_PREPARED", "目标预检已失效，请重新选择"));
        }
        pending
            .take()
            .ok_or_else(|| Error::new("PROJECT_PREPARED", "目标预检已失效"))?
    };
    let e = engine.inner().clone();
    let b = batches.inner().clone();
    let info = tauri::async_runtime::spawn_blocking(move || {
        b.change_project(|| match prepared.target {
            PreparedTarget::Open(repo) => e.activate_project(repo),
            PreparedTarget::Create(root) => {
                let name = root.file_name().unwrap_or_default().to_string_lossy();
                e.create_project(&root, &name)
            }
            PreparedTarget::Same(info) => Ok(info),
        })
    })
    .await
    .map_err(|_| Error::new("PROJECT_TASK", "工程切换任务失败"))??;
    let warning = session
        .recent
        .save_named(Some(&info.entry), Some(&info.name))
        .err()
        .map(|e| format!("工程已打开，但最近工程设置未保存：{e}"));
    *session
        .startup_error
        .lock()
        .map_err(|_| Error::new("PROJECT_STATE", "工程状态不可用"))? = warning;
    Ok(info)
}
#[tauri::command]
pub fn remove_recent_project(
    entry: std::path::PathBuf,
    session: State<'_, ProjectSession>,
) -> Result<(), ErrorDto> {
    session.recent.remove(&entry)
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

fn requires_stop(engine: &Engine) -> bool {
    engine.project_requires_stop().unwrap_or(true)
}
