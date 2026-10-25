//! 桌面组装：会话配置及真实TCP从站；核心模型不依赖Tauri。
mod commands;
mod initial_window;
mod native_menu;
mod window_context;
mod window_index;
mod window_lifecycle;
use tauri::{Emitter, Manager};
/// 工作台完成工程退出保护后才允许退出；无工程引导页可直接关闭。
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
        .setup(|app| {
            let registry = window_context::Windows::new(app.path().app_data_dir()?);
            let context = window_context::ProjectContext::new(
                registry.root.join("runs"),
                registry.recent.clone(),
            )?;
            let restored = match registry.recent.load() {
                Ok(Some(path)) => match context.engine.open_project(&path) {
                    Ok(info) => {
                        registry
                            .index
                            .lock()
                            .unwrap()
                            .publish(info.entry.clone(), "main");
                        registry
                            .recent
                            .save_named(Some(&info.entry), Some(&info.name))
                            .err()
                            .map(|e| e.to_string())
                    }
                    Err(e) => Some(e.to_string()),
                },
                Ok(None) => None,
                Err(e) => Some(e.to_string()),
            };
            *context.session.startup_error.lock().unwrap() = restored;
            let launcher = context.engine.project_info()?.is_none();
            registry
                .contexts
                .lock()
                .unwrap()
                .insert("main".into(), context.clone());
            *registry.active.lock().unwrap() = Some("main".into());
            app.manage(registry);
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(not(target_os = "macos"))]
                window.set_decorations(false)?;
                initial_window::configure(&window)?;
                context.layout.apply(&window, launcher)?;
                window.show()?;
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            let app = window.app_handle();
            match event {
                tauri::WindowEvent::Focused(true) => window_lifecycle::focused(app, window.label()),
                tauri::WindowEvent::CloseRequested { api, .. } => {
                    if let Ok(c) = app
                        .state::<window_context::Windows>()
                        .context(window.label())
                    {
                        if !c
                            .permission
                            .confirmed
                            .load(std::sync::atomic::Ordering::Acquire)
                        {
                            api.prevent_close();
                            if !app
                                .state::<window_context::Windows>()
                                .quitting
                                .load(std::sync::atomic::Ordering::Acquire)
                            {
                                let _ = app.emit_to(window.label(), "workbench-exit-requested", ());
                            }
                        }
                    }
                }
                tauri::WindowEvent::Destroyed => {
                    let windows = app.state::<window_context::Windows>();
                    let removed = windows.contexts.lock().unwrap().remove(window.label());
                    windows.index.lock().unwrap().release(window.label());
                    if let Some(c) = removed {
                        // 异常销毁也仅回收自身；不在窗口事件线程等待后台协议。
                        tauri::async_runtime::spawn_blocking(move || {
                            c.jobs.cancel_all();
                            c.subscriptions.shutdown();
                            let _ = c.batches.request_shutdown();
                            c.batches.wait_idle();
                            c.engine.shutdown_slave();
                            let _ = c.engine.close_project();
                        });
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            initial_window::sync_project_window,
            commands::project::query_project,
            commands::project::load_project_document,
            commands::project::prepare_project,
            commands::project::commit_project,
            commands::project::cancel_prepared_project,
            commands::project::remove_recent_project,
            commands::project::close_project,
            commands::project::read_project_local,
            commands::project::write_project_local,
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
            window_lifecycle::approve_window_exit,
            window_lifecycle::cancel_application_exit,
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
            #[cfg(target_os = "macos")]
            if let tauri::RunEvent::Reopen {
                has_visible_windows: false,
                ..
            } = &event
            {
                if let Some(label) = app.state::<window_context::Windows>().active_label() {
                    let _ = window_lifecycle::activate(app, &label);
                }
            }
            if let tauri::RunEvent::ExitRequested { api, .. } = event {
                let windows = app.state::<window_context::Windows>();
                let contexts = windows.contexts.lock().unwrap();
                let allowed = contexts.is_empty()
                    || contexts.values().all(|c| {
                        c.permission
                            .confirmed
                            .load(std::sync::atomic::Ordering::Acquire)
                    });
                drop(contexts);
                if !allowed {
                    api.prevent_exit();
                    window_lifecycle::request_quit(app);
                }
            }
        });
}
