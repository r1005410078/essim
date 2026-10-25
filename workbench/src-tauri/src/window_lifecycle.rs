//! 窗口关闭与应用退出的协调；草稿确认全部完成后才停止工程。
use crate::window_context::{QuitTransaction, Windows};
use essim_core::domain::error::Error;
use std::sync::atomic::Ordering;
use tauri::{Emitter, Manager};

pub(crate) fn activate(app: &tauri::AppHandle, label: &str) -> Result<(), Error> {
    let window = app
        .get_webview_window(label)
        .ok_or_else(|| Error::new("WINDOW_CLOSED", "目标窗口已关闭"))?;
    window
        .unminimize()
        .and_then(|_| window.show())
        .and_then(|_| window.set_focus())
        .map_err(|e| Error::new("WINDOW_FOCUS", e.to_string()))
}
pub(crate) fn recent_changed(app: &tauri::AppHandle) {
    let _ = app.emit("recent-changed", ());
}
pub(crate) fn focused(app: &tauri::AppHandle, label: &str) {
    let windows = app.state::<Windows>();
    if windows.quitting.load(Ordering::Acquire) {
        return;
    }
    if let Ok(ctx) = windows.context(label) {
        *windows.active.lock().unwrap() = Some(label.into());
        if let Ok(Some(info)) = ctx.engine.project_info() {
            let _ = windows
                .recent
                .save_named(Some(&info.entry), Some(&info.name));
            recent_changed(app);
        }
    }
}
pub(crate) fn forget_project(app: &tauri::AppHandle, label: &str) {
    let windows = app.state::<Windows>();
    windows.index.lock().unwrap().release(label);
    if windows.quitting.load(Ordering::Acquire) {
        return;
    }
    let last = windows.recent.load().ok().flatten();
    let remaining: Vec<_> = windows
        .contexts
        .lock()
        .unwrap()
        .iter()
        .filter(|(l, _)| l.as_str() != label)
        .filter_map(|(l, c)| {
            c.engine
                .project_info()
                .ok()
                .flatten()
                .map(|p| (l.clone(), p))
        })
        .collect();
    if remaining
        .iter()
        .any(|(_, p)| Some(&p.entry) == last.as_ref())
    {
        return;
    }
    let active = windows.active_label();
    let recents = windows.recent.entries().unwrap_or_default();
    let chosen = remaining
        .iter()
        .find(|(l, _)| Some(l) == active.as_ref())
        .or_else(|| {
            recents
                .iter()
                .find_map(|r| remaining.iter().find(|(_, p)| p.entry == r.entry))
        });
    if let Some((_, p)) = chosen {
        let _ = windows.recent.save_named(Some(&p.entry), Some(&p.name));
    } else {
        let _ = windows.recent.save(None);
    }
    recent_changed(app);
}
pub(crate) fn cancel_quit(app: &tauri::AppHandle) {
    let windows = app.state::<Windows>();
    windows.quit.lock().unwrap().take();
    windows.quitting.store(false, Ordering::Release);
    let _ = app.emit("workbench-exit-cancelled", ());
}
pub(crate) fn request_quit(app: &tauri::AppHandle) {
    let windows = app.state::<Windows>();
    if windows.quitting.swap(true, Ordering::AcqRel) {
        return;
    }
    let contexts = windows.contexts.lock().unwrap();
    if contexts.values().any(|c| c.closing.load(Ordering::Acquire)) {
        drop(contexts);
        windows.quitting.store(false, Ordering::Release);
        let _ = app.emit("workbench-window-error", "有窗口正在关闭，请稍后重试退出");
        return;
    }
    let remaining = contexts.keys().cloned().collect();
    drop(contexts);
    *windows.quit.lock().unwrap() = Some(QuitTransaction {
        id: windows.quit_epoch.fetch_add(1, Ordering::Relaxed),
        remaining,
        current: None,
    });
    next_confirmation(app);
}
fn next_confirmation(app: &tauri::AppHandle) {
    let windows = app.state::<Windows>();
    let next = {
        let mut transaction = windows.quit.lock().unwrap();
        let Some(q) = transaction.as_mut() else {
            return;
        };
        let next = q.remaining.pop();
        q.current = next.clone();
        next
    };
    if let Some(label) = next {
        let epoch = windows.quit.lock().unwrap().as_ref().map(|q| q.id);
        let _ = activate(app, &label);
        if app.emit_to(&label, "workbench-exit-requested", ()).is_err() {
            cancel_quit(app);
        }
        // 无响应不能当作同意。超时解除退出门禁，并把错误送回仍存活的窗口。
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            std::thread::sleep(std::time::Duration::from_secs(60));
            let waiting = app
                .state::<Windows>()
                .quit
                .lock()
                .unwrap()
                .as_ref()
                .is_some_and(|q| q.waiting_for(epoch, &label));
            if waiting {
                cancel_quit(&app);
                let _ = app.emit("workbench-window-error", "退出确认超时，应用保持打开");
            }
        });
    } else {
        let app = app.clone();
        let failure_app = app.clone();
        let worker = tauri::async_runtime::spawn_blocking(move || {
            let contexts: Vec<_> = app
                .state::<Windows>()
                .contexts
                .lock()
                .unwrap()
                .values()
                .cloned()
                .collect();
            if let Err(error) = prepare_all(&contexts) {
                cancel_quit(&app);
                let _ = app.emit("workbench-window-error", error.to_string());
                return;
            }
            // 仅在全部保护、停机与刷新完成后允许退出；不通过单窗关闭路径清空恢复入口。
            app.state::<Windows>().quit.lock().unwrap().take();
            for c in &contexts {
                c.permission.confirmed.store(true, Ordering::Release);
            }
            app.exit(0);
        });
        tauri::async_runtime::spawn(async move {
            if worker.await.is_err() {
                cancel_quit(&failure_app);
                let _ =
                    failure_app.emit("workbench-window-error", "退出后台任务异常，应用保持打开");
            }
        });
    }
}
#[tauri::command]
pub(crate) fn cancel_application_exit(app: tauri::AppHandle) {
    let confirming = app
        .state::<Windows>()
        .quit
        .lock()
        .unwrap()
        .as_ref()
        .is_some_and(|q| q.current.is_some());
    if confirming {
        cancel_quit(&app);
    }
}
#[tauri::command]
pub(crate) async fn approve_window_exit(
    window: tauri::WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), Error> {
    let windows = app.state::<Windows>();
    if windows.quitting.load(Ordering::Acquire) {
        let valid = {
            let mut transaction = windows.quit.lock().unwrap();
            if let Some(q) = transaction.as_mut() {
                q.acknowledge(window.label())
            } else {
                false
            }
        };
        if !valid {
            return Err(Error::new("APPLICATION_EXITING", "其他窗口正在确认退出"));
        }
        next_confirmation(&app);
        return Ok(());
    }
    let c = windows.context(window.label())?;
    if c.closing.swap(true, Ordering::AcqRel) {
        return Err(Error::new("WINDOW_CLOSING", "窗口正在关闭"));
    }
    windows.index.lock().unwrap().closing(window.label());
    let worker = c.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        worker
            .batches
            .change_project(|| worker.engine.close_project())
    })
    .await
    .unwrap_or_else(|_| Err(Error::new("WINDOW_CLOSE", "工程关闭任务失败")));
    if let Err(error) = result {
        c.closing.store(false, Ordering::Release);
        if let Some(p) = c.engine.project_info()? {
            windows
                .index
                .lock()
                .unwrap()
                .publish(p.entry, window.label());
        }
        return Err(error);
    }
    forget_project(&app, window.label());
    c.permission.confirmed.store(true, Ordering::Release);
    let next = windows
        .contexts
        .lock()
        .unwrap()
        .keys()
        .find(|label| label.as_str() != window.label())
        .cloned();
    window
        .destroy()
        .map_err(|e| Error::new("WINDOW_CLOSE", e.to_string()))?;
    // macOS 强制销毁已保护窗口后不会自动选择下一个 key window。
    if let Some(label) = next {
        activate(&app, &label)?;
    }
    Ok(())
}

/// 停机准备失败时不释放任何工程，便于仍打开的窗口继续检查和重试。
pub(crate) fn prepare_all(
    contexts: &[std::sync::Arc<crate::window_context::ProjectContext>],
) -> Result<(), Error> {
    for context in contexts {
        context
            .batches
            .change_project(|| context.engine.prepare_project_exit())?;
    }
    Ok(())
}
