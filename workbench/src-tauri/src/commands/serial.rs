//! 串口发现查询；不涉及串口所有权和协议执行。
/// 在阻塞工作线程枚举设备，避免占用窗口事件线程。
#[tauri::command]
pub async fn list_serial_ports(
) -> Result<Vec<essim_infrastructure::serial_discovery::SerialDevice>, String> {
    tauri::async_runtime::spawn_blocking(
        essim_infrastructure::serial_discovery::available_serial_devices,
    )
    .await
    .map_err(|error| format!("串口查询未完成：{error}"))?
}
