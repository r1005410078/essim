//! 工程生命周期 IPC；路径选择保留系统对话框。
use super::{dto::ErrorDto, Engine};
use crate::window_context::WindowState as State;
use crate::{
    window_context::{ProjectContext, Windows},
    window_index::Owner,
};
use essim_core::domain::error::Error;
use essim_infrastructure::project::{ProjectInfo, ProjectRepository, RecentEntry, RecentProject};
use serde::Serialize;
use std::sync::Mutex;
use tauri::{Emitter, Manager};
/// 启动恢复错误保留给前端展示，不伪装成新空工程。
pub struct ProjectSession {
    pub recent: std::sync::Arc<RecentProject>,
    pub startup_error: Mutex<Option<String>>,
    pub operation: std::sync::atomic::AtomicBool,
    pub prepared: Mutex<Option<Prepared>>,
}
pub struct Prepared {
    token: String,
    target: PreparedTarget,
    reservation: Option<Reservation>,
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
    handled: bool,
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
    current_window: Option<bool>,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
) -> Result<Option<Preparation>, ErrorDto> {
    let operation = crate::diagnostics::next_operation("prepare_project");
    let diagnostic_app = window.app_handle().clone();
    let result =
        prepare_project_inner(window, action, entry, current_window, engine, session).await;
    if result.is_err() {
        if let Some(log) = diagnostic_app.try_state::<crate::diagnostics::ApplicationLog>() {
            crate::diagnostics::result(&log.log, "prepare_project", &operation, None, &result);
        }
    }
    result
}
async fn prepare_project_inner(
    window: tauri::WebviewWindow,
    action: String,
    entry: Option<std::path::PathBuf>,
    current_window: Option<bool>,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
) -> Result<Option<Preparation>, ErrorDto> {
    let _guard = acquire(&session.operation)?;
    let app = window.app_handle().clone();
    app.state::<Windows>().ensure_open()?;
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
    if action == "create"
        && std::fs::read_dir(&path)
            .map_err(|e| Error::new("PROJECT_IO", e.to_string()))?
            .next()
            .is_some()
    {
        return Err(Error::new("PROJECT_NOT_EMPTY", "请选择空目录新建工程"));
    }
    let current = engine.project_info()?;
    let key = if action == "create" {
        path.canonicalize().map(|p| p.join("project.workspace"))
    } else {
        path.canonicalize()
    }
    .map_err(|e| Error::new("PROJECT_IO", e.to_string()))?;
    let new_window = current.is_some() && !current_window.unwrap_or(false);
    let label = if new_window {
        app.state::<Windows>().next_label()
    } else {
        window.label().to_string()
    };
    let reserved = app
        .state::<Windows>()
        .index
        .lock()
        .unwrap()
        .reserve(key.clone(), &label);
    if let Err(owner) = reserved {
        let owner_label = match owner {
            Owner::Open(label) => label,
            Owner::Closing(_) => return Err(Error::new("PROJECT_BUSY", "工程正在关闭")),
            Owner::Opening(_) => {
                let app = app.clone();
                let key = key.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    for _ in 0..600 {
                        let owner = app.state::<Windows>().index.lock().unwrap().owner(&key);
                        match owner {
                            Some(Owner::Open(label)) => return Ok(label),
                            Some(Owner::Opening(_)) => {
                                std::thread::sleep(std::time::Duration::from_millis(100))
                            }
                            _ => return Err(Error::new("PROJECT_OPEN", "工程打开未完成，请重试")),
                        }
                    }
                    Err(Error::new("PROJECT_BUSY", "等待工程窗口就绪超时"))
                })
                .await
                .map_err(|_| Error::new("PROJECT_OPEN", "等待窗口失败"))??
            }
        };
        crate::window_lifecycle::activate(&app, &owner_label)?;
        crate::window_lifecycle::focused(&app, &owner_label);
        return Ok(Some(Preparation {
            token: String::new(),
            name: String::new(),
            same: true,
            handled: true,
        }));
    }
    let mut reservation = Reservation {
        app: app.clone(),
        key: key.clone(),
        label: label.clone(),
        retained: false,
    };
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
    if new_window {
        app.state::<Windows>().ensure_open()?;
        let context = ProjectContext::new(
            app.state::<Windows>().root.join("runs"),
            app.state::<Windows>().recent.clone(),
        )?;
        let worker = context.clone();
        let info = tauri::async_runtime::spawn_blocking(move || match target {
            PreparedTarget::Open(repository) => worker.engine.activate_project(repository),
            PreparedTarget::Create(root) => {
                let name = root.file_name().unwrap_or_default().to_string_lossy();
                worker.engine.create_project(&root, &name)
            }
            PreparedTarget::Same(info) => Ok(info),
        })
        .await
        .map_err(|_| Error::new("PROJECT_OPEN", "打开工程失败"))??;
        let mut config = app
            .config()
            .app
            .windows
            .first()
            .cloned()
            .ok_or_else(|| Error::new("WINDOW_CONFIG", "缺少窗口配置"))?;
        context
            .pending_window
            .store(true, std::sync::atomic::Ordering::Release);
        {
            let windows = app.state::<Windows>();
            let mut contexts = windows.contexts.lock().unwrap();
            windows.ensure_open()?;
            contexts.insert(label.clone(), context.clone());
        }
        config.label = label.clone();
        let created = (|| -> tauri::Result<_> {
            let window = tauri::WebviewWindowBuilder::from_config(&app, &config)?
                .visible(false)
                .title(format!("ESSim · {}", info.name))
                .build()?;
            crate::initial_window::configure(&window)?;
            Ok(window)
        })();
        match created {
            Ok(_) => {
                reservation.retained = true;
                let cleanup_app = app.clone();
                let cleanup_label = label.clone();
                let source_label = window.label().to_string();
                let pending = context.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    for _ in 0..600 {
                        if !pending
                            .pending_window
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            return;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                    if !pending
                        .pending_window
                        .swap(false, std::sync::atomic::Ordering::AcqRel)
                    {
                        return;
                    }
                    cleanup_app
                        .state::<Windows>()
                        .index
                        .lock()
                        .unwrap()
                        .release(&cleanup_label);
                    cleanup_app
                        .state::<Windows>()
                        .contexts
                        .lock()
                        .unwrap()
                        .remove(&cleanup_label);
                    if let Some(window) = cleanup_app.get_webview_window(&cleanup_label) {
                        let _ = window.destroy();
                    }
                    let _ = cleanup_app.emit_to(
                        &source_label,
                        "workbench-window-error",
                        "新工程窗口未能就绪，请重试",
                    );
                });
                drop(reservation);
                return Ok(Some(Preparation {
                    token: String::new(),
                    name: info.name,
                    same: false,
                    handled: true,
                }));
            }
            Err(error) => {
                app.state::<Windows>()
                    .contexts
                    .lock()
                    .unwrap()
                    .remove(&label);
                if let Some(window) = app.get_webview_window(&label) {
                    let _ = window.destroy();
                }
                return Err(Error::new("WINDOW_CREATE", error.to_string()));
            }
        }
    }
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
        reservation: Some(reservation),
    });
    Ok(Some(Preparation {
        token,
        name,
        same,
        handled: false,
    }))
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
    app: tauri::AppHandle,
    token: String,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<ProjectInfo, ErrorDto> {
    let operation = crate::diagnostics::next_operation("commit_project");
    let diagnostic_app = app.clone();
    let result = commit_project_inner(app, token, engine, session, batches).await;
    if result.is_err() {
        if let Some(log) = diagnostic_app.try_state::<crate::diagnostics::ApplicationLog>() {
            crate::diagnostics::result(&log.log, "commit_project", &operation, None, &result);
        }
    }
    result
}
async fn commit_project_inner(
    app: tauri::AppHandle,
    token: String,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<ProjectInfo, ErrorDto> {
    app.state::<Windows>().ensure_open()?;
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
    let reservation = prepared.reservation;
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
    if let Some(reservation) = reservation {
        let windows = reservation.app.state::<Windows>();
        windows.index.lock().unwrap().release(&reservation.label);
        windows
            .index
            .lock()
            .unwrap()
            .publish(info.entry.clone(), &reservation.label);
        crate::window_lifecycle::recent_changed(&reservation.app);
    }
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
    app: tauri::AppHandle,
    entry: std::path::PathBuf,
    session: State<'_, ProjectSession>,
) -> Result<(), ErrorDto> {
    session.recent.remove(&entry)?;
    crate::window_lifecycle::recent_changed(&app);
    Ok(())
}
#[tauri::command]
pub async fn close_project(
    window: tauri::WebviewWindow,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<(), ErrorDto> {
    let operation = crate::diagnostics::next_operation("close_project");
    let diagnostic_app = window.app_handle().clone();
    let result = close_project_inner(window, engine, session, batches).await;
    if result.is_err() {
        if let Some(log) = diagnostic_app.try_state::<crate::diagnostics::ApplicationLog>() {
            crate::diagnostics::result(&log.log, "close_project", &operation, None, &result);
        }
    }
    result
}
async fn close_project_inner(
    window: tauri::WebviewWindow,
    engine: State<'_, Engine>,
    session: State<'_, ProjectSession>,
    batches: State<'_, super::batch::Batches>,
) -> Result<(), ErrorDto> {
    window.state::<Windows>().ensure_open()?;
    let _guard = acquire(&session.operation)?;
    let e = engine.inner().clone();
    let b = batches.inner().clone();
    tauri::async_runtime::spawn_blocking(move || b.change_project(|| e.close_project()))
        .await
        .map_err(|_| Error::new("PROJECT_TASK", "工程关闭任务失败"))??;
    crate::window_lifecycle::forget_project(window.app_handle(), window.label());
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

/// 未发布的路径预约随取消、错误及准备状态销毁自动释放。
struct Reservation {
    app: tauri::AppHandle,
    key: std::path::PathBuf,
    label: String,
    retained: bool,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.retained {
            return;
        }
        self.app
            .state::<Windows>()
            .index
            .lock()
            .unwrap()
            .release_path(&self.key, &self.label);
    }
}
