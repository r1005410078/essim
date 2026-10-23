//! 桌面组装：会话配置及真实TCP从站；核心模型不依赖Tauri。
mod commands;
mod initial_window;
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
        .setup(|app| {
            let record_root = app.path().app_data_dir()?.join("runs");
            let engine=Arc::new(WorkspaceEngine::with_tcp_runtime(record_root.clone())?);
            app.manage(Arc::new(essim_infrastructure::batch_lifecycle::BatchCoordinator::new(engine.clone())?));
            app.manage(engine);
            if let Some(window) = app.get_webview_window("main") {
                if let Err(error) = initial_window::configure(&window) {
                    // 系统不提供工作区信息时仍以配置尺寸显示，不让工作台停留在隐藏状态。
                    eprintln!("无法调整首次窗口尺寸：{error}");
                }
                window.show()?;
            }
            Ok(())
        })
        .manage(commands::subscriptions::Subscriptions::default())
        .manage(commands::point_table::PointFileJobs::default())
        .invoke_handler(tauri::generate_handler![
            commands::batch::preview_batch,
            commands::batch::start_batch,
            commands::batch::query_batch,
            commands::batch::query_resources,
            commands::batch::cancel_batch,
            commands::batch::discard_batch,
            commands::batch::retry_batch,
            commands::simulator::query_workspace,
            commands::simulator::load_model,
            commands::simulator::load_endpoint_values,
            commands::serial::list_serial_ports,
            commands::partition::preview_partition,
            commands::partition::create_partition,
            commands::partition::discard_partition,
            commands::master::execute_master,
            commands::master::query_master,
            commands::slave::execute_slave,
            commands::slave::query_slave,
            commands::simulator::exit_workbench,
            commands::simulator::set_exit_guard_ready,
            commands::simulator::preview_apply,
            commands::simulator::resolve_unit,
            commands::simulator::preview_unit_promotion,
            commands::simulator::capture_initials,
            commands::point_table::export_unit_points,
            commands::simulator::inspect_endpoint,
            commands::simulator::execute_workspace,
            commands::point_table::preview_point_table,
            commands::point_table::open_point_file,
            commands::point_table::cancel_point_file,
            commands::slave::export_observations,
            commands::point_table::export_point_file,
            commands::point_table::export_point_template,
            commands::subscriptions::subscribe_workspace,
            commands::subscriptions::unsubscribe_workspace
        ])
        .build(tauri::generate_context!())
        .expect("无法启动ESSim工作台")
        .run(|app, event| {
            if matches!(&event, tauri::RunEvent::Exit) {
                let batches=app.state::<commands::batch::Batches>();
                let _=batches.request_shutdown();
                batches.wait_idle();
                app.state::<Arc<WorkspaceEngine>>().shutdown_slave();
            }
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
