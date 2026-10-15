//! 单串口单从站教学入口，使用与工作台相同的应用服务。
use essim_core::{application::simulator::SimulatorService, domain::modbus::RtuEndpoint};
use essim_infrastructure::ModbusSimulator;
use std::io::{self, BufRead, Write};
fn main() {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 8 {
        eprintln!("用法：rtu_demo <串口路径> <波特率> <N|E|O> <数据位8> <停止位1|2> <站号1–247> <接收超时10–100ms>\n例如：rtu_demo /dev/serial-example 19200 N 8 2 7 50；示例路径为教学占位。");
        std::process::exit(2);
    }
    let config = (|| {
        RtuEndpoint::new(
            &args[1],
            args[2].parse().map_err(|_| "波特率无效")?,
            args[3].parse().map_err(|_| "校验位无效")?,
            args[4].parse().map_err(|_| "数据位无效")?,
            args[5].parse().map_err(|_| "停止位无效")?,
            args[6].parse().map_err(|_| "站号无效")?,
            args[7].parse().map_err(|_| "超时无效")?,
            2,
        )
    })();
    let config = match config {
        Ok(c) => c,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(2);
        }
    };
    let slave = config.slave.raw() as u16;
    let app = SimulatorService(ModbusSimulator::new());
    if let Err(e) = app.start_rtu(config) {
        eprintln!("{}: {}", e.code, e.message);
        std::process::exit(1);
    }
    println!(
        "OPEN RTU slave={slave}; address=100, SOC=650; 输入 0–1000 整数更新，quit 或 EOF 停机。"
    );
    io::stdout().flush().unwrap();
    for line in io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        if line.trim() == "quit" {
            break;
        }
        match line
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(|v| app.update_unit(slave, "soc", v).ok())
        {
            Some(_) => println!("ACK SOC={}", app.snapshot().rtu_soc.raw()),
            None => println!("拒绝：SOC 必须为 0–1000 的整数"),
        }
        io::stdout().flush().unwrap();
    }
    app.stop().unwrap();
    println!("STOPPED");
}
