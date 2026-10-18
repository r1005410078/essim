//! 桌面应用组装。核心模型不依赖Tauri，当前只接入会话内配置目录。
mod commands;
mod native_menu;
use essim_infrastructure::workspace::WorkspaceEngine;
use std::sync::Arc;
use tauri::{Emitter, Manager};
/// 只有前端明确确认放弃会话后才允许进程退出；退出码不是授权凭据。
#[derive(Default)]
pub(crate) struct ExitPermission {
    confirmed: std::sync::atomic::AtomicBool,
    armed: std::sync::atomic::AtomicBool,
}
/// 启动工作台，订阅管理器销毁时取消并回收推送线程。
pub fn run() {
    tauri::Builder::default()
        .menu(native_menu::build)
        .on_menu_event(native_menu::dispatch)
        .manage(ExitPermission::default())
        .manage(Arc::new(WorkspaceEngine::default()))
        .manage(commands::subscriptions::Subscriptions::default())
        .manage(commands::point_table::PointFileJobs::default())
        .invoke_handler(tauri::generate_handler![
            commands::simulator::query_workspace,
            commands::simulator::exit_workbench,
            commands::simulator::set_exit_guard_ready,
            commands::simulator::preview_apply,
            commands::simulator::inspect_endpoint,
            commands::simulator::execute_workspace,
            commands::point_table::preview_point_table,
            commands::point_table::open_point_file,
            commands::point_table::cancel_point_file,
            commands::point_table::export_point_file,
            commands::point_table::export_point_template,
            commands::subscriptions::subscribe_workspace,
            commands::subscriptions::unsubscribe_workspace
        ])
        .build(tauri::generate_context!())
        .expect("无法启动ESSim工作台")
        .run(|app, event| {
            // macOS Cmd+Q不经过Window::CloseRequested，必须单独送至前端确认。
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let permission = app.state::<ExitPermission>();
                if permission.armed.load(std::sync::atomic::Ordering::Acquire)
                    && !permission
                        .confirmed
                        .load(std::sync::atomic::Ordering::Acquire)
                    && app.get_webview_window("main").is_some()
                {
                    api.prevent_exit();
                    let _ = app.emit_to("main", "workbench-exit-requested", ());
                }
            }
        });
}
