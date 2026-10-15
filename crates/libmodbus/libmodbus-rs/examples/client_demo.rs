//! 独立主站互操作示例，地址和值均为教学约定；会写入目标设备，仅用于测试从站。
use libmodbus_rs::{exception_code, ModbusClient, RtuConfig};
use std::{io, time::Duration};
fn main() -> io::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let wait = Duration::from_millis(500);
    let mut c = match args.as_slice() {
        [mode, target] if mode == "tcp" => ModbusClient::tcp(
            target
                .parse()
                .map_err(|_| io::Error::other("需要 IP:port"))?,
            7,
            wait,
            wait,
        )?,
        [mode, target] if mode == "rtu" => {
            ModbusClient::rtu(&RtuConfig::new(target, 19200, 'N', 8, 2, 7, 50, 2)?, wait)?
        }
        _ => {
            return Err(io::Error::other(
                "用法：client_demo tcp IP:port | rtu 串口路径；目标站号7",
            ))
        }
    };
    assert_eq!(c.read_coil(100, 2)?, [true, false]);
    assert_eq!(c.read_discrete(100, 2)?, [false, true]);
    assert_eq!(c.read_holding(100, 2)?, [42, 43]);
    assert_eq!(c.read_input(100, 2)?, [99, 100]);
    c.write_coil(100, false)?;
    c.write_holding(100, 12)?;
    assert_eq!(c.read_coil(100, 1)?, [false]);
    assert_eq!(c.read_holding(100, 1)?, [12]);
    c.write_coils(100, &[true, true])?;
    c.write_holdings(100, &[55, 56])?;
    assert_eq!(c.read_coil(100, 2)?, [true, true]);
    assert_eq!(c.read_holding(100, 2)?, [55, 56]);
    assert_eq!(
        exception_code(&c.read_holding(500, 1).unwrap_err()),
        Some(2)
    );
    assert_eq!(c.read_input(100, 2)?, [99, 100]);
    println!("PASS 四区读取、单/多写、异常后继续请求");
    Ok(())
}
