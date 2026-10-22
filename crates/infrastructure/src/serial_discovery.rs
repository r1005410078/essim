//! 串口发现适配器，只枚举设备，不打开或配置串口。
use serde::Serialize;
use serialport::{SerialPortInfo, SerialPortType};

/// 可供界面选择的串口路径及设备名称。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SerialDevice {
    /// 操作系统返回的完整设备路径。
    pub path: String,
    /// USB产品／厂商信息或串口类型名称；不是连通性保证。
    pub name: String,
}

fn describe(port: SerialPortInfo) -> SerialDevice {
    let name = match port.port_type {
        SerialPortType::UsbPort(info) => {
            let mut parts = Vec::new();
            for value in [info.manufacturer, info.product, info.serial_number]
                .into_iter()
                .flatten()
            {
                if !value.trim().is_empty() && !parts.contains(&value) {
                    parts.push(value);
                }
            }
            parts.push(format!("USB {:04X}:{:04X}", info.vid, info.pid));
            parts.join(" · ")
        }
        SerialPortType::BluetoothPort => "蓝牙串口".into(),
        SerialPortType::PciPort => "PCI串口".into(),
        SerialPortType::Unknown => "串口设备（名称未知）".into(),
    };
    SerialDevice {
        path: port.port_name,
        name,
    }
}

/// 枚举本机串口并按路径排序，不改变已有配置或正在运行的连接。
///
/// # Errors
/// 操作系统枚举失败时返回错误；空列表表示当前未发现设备。
pub fn available_serial_devices() -> Result<Vec<SerialDevice>, String> {
    let mut devices: Vec<_> = serialport::available_ports()
        .map_err(|error| format!("无法枚举串口：{error}"))?
        .into_iter()
        .map(describe)
        .collect();
    devices.sort_by(|a, b| a.path.cmp(&b.path));
    devices.dedup_by(|a, b| a.path == b.path);
    Ok(devices)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn usb_name_retains_path_and_identifying_information() {
        let device = describe(SerialPortInfo {
            port_name: "/dev/cu.fixture".into(),
            port_type: SerialPortType::UsbPort(serialport::UsbPortInfo {
                vid: 0x1234,
                pid: 0x5678,
                serial_number: Some("SN1".into()),
                manufacturer: Some("厂商".into()),
                product: Some("RS485".into()),
            }),
        });
        assert_eq!(device.path, "/dev/cu.fixture");
        assert_eq!(device.name, "厂商 · RS485 · SN1 · USB 1234:5678");
    }
    #[test]
    fn unavailable_name_is_explicit() {
        let device = describe(SerialPortInfo {
            port_name: "COM3".into(),
            port_type: SerialPortType::Unknown,
        });
        assert!(device.name.contains("未知"));
        assert_eq!(device.path, "COM3");
    }
}
