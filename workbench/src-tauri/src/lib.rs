mod commands;
use essim_core::application::simulator::SimulatorService;
use essim_infrastructure::ModbusSimulator;
use std::sync::Arc;
pub fn run() {
    let engine = Arc::new(SimulatorService(ModbusSimulator::new()));
    let exit_engine = engine.clone();
    tauri::Builder::default()
        .manage(engine)
        .manage(commands::Subscriptions::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_backend_info,
            commands::get_simulator_snapshot,
            commands::start_simulator,
            commands::start_rtu_simulator,
            commands::configure_simulator,
            commands::stop_simulator,
            commands::update_simulator_soc,
            commands::add_simulator_unit,
            commands::subscribe_simulator,
            commands::unsubscribe_simulator
        ])
        .build(tauri::generate_context!())
        .expect("无法启动 ESSim 工作台")
        .run(move |_, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                let _ = exit_engine.stop();
            }
        });
}
