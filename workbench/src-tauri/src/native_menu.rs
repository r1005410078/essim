//! 原生菜单负责把退出及历史快捷键交回工作台，避免绕过草稿保护。
use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem, Submenu},
    AppHandle, Emitter, Manager,
};

pub(crate) fn build(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let quit = MenuItem::with_id(
        app,
        "workbench-quit",
        "退出 ESSim",
        true,
        Some("CmdOrCtrl+Q"),
    )?;
    #[cfg(target_os = "macos")]
    let application = Submenu::with_items(
        app,
        "ESSim",
        true,
        &[
            &PredefinedMenuItem::about(app, None, None)?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::hide(app, None)?,
            &PredefinedMenuItem::hide_others(app, None)?,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;
    let file = Submenu::with_items(
        app,
        "文件",
        true,
        &[
            &MenuItem::with_id(app, "workbench-save", "保存", true, Some("CmdOrCtrl+S"))?,
            &PredefinedMenuItem::close_window(app, Some("关闭窗口"))?,
            #[cfg(not(target_os = "macos"))]
            &quit,
        ],
    )?;
    let edit = Submenu::with_items(
        app,
        "编辑",
        true,
        &[
            &MenuItem::with_id(app, "workbench-undo", "撤销", true, Some("CmdOrCtrl+Z"))?,
            &MenuItem::with_id(
                app,
                "workbench-redo",
                "重做",
                true,
                Some("CmdOrCtrl+Shift+Z"),
            )?,
            &PredefinedMenuItem::separator(app)?,
            &PredefinedMenuItem::cut(app, Some("剪切"))?,
            &PredefinedMenuItem::copy(app, Some("复制"))?,
            &PredefinedMenuItem::paste(app, Some("粘贴"))?,
            &MenuItem::with_id(
                app,
                "workbench-selectAll",
                "全选",
                true,
                Some("CmdOrCtrl+A"),
            )?,
        ],
    )?;
    Menu::with_items(
        app,
        &[
            #[cfg(target_os = "macos")]
            &application,
            &file,
            &edit,
        ],
    )
}
pub(crate) fn dispatch(app: &AppHandle, event: tauri::menu::MenuEvent) {
    match event.id.as_ref() {
        "workbench-quit" => {
            if app
                .state::<crate::ExitPermission>()
                .armed
                .load(std::sync::atomic::Ordering::Acquire)
            {
                let _ = app.emit_to("main", "workbench-exit-requested", ());
            } else {
                app.exit(0);
            }
        }
        "workbench-save" => {
            let _ = app.emit_to("main", "workbench-save", ());
        }
        "workbench-undo" => {
            let _ = app.emit_to("main", "workbench-edit", "undo");
        }
        "workbench-redo" => {
            let _ = app.emit_to("main", "workbench-edit", "redo");
        }
        "workbench-selectAll" => {
            let _ = app.emit_to("main", "workbench-edit", "selectAll");
        }
        _ => {}
    }
}
