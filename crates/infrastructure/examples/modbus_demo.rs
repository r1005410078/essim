use essim_core::application::simulator::SimulatorService;
use essim_infrastructure::ModbusSimulator;
use std::io::{self, BufRead};
fn main() {
    let app = SimulatorService(ModbusSimulator::new());
    let port = std::env::args()
        .nth(1)
        .map(|s| s.parse::<u16>().expect("端口必须为整数"))
        .unwrap_or(1502);
    match app.start("127.0.0.1", port) {
        Ok(_) => println!(
            "LISTENING 127.0.0.1:{port}; Unit=1, address=100, SOC=650. 输入整数更新，quit 停机。"
        ),
        Err(e) => {
            eprintln!("{}: {}", e.code, e.message);
            std::process::exit(1);
        }
    }
    for line in io::stdin().lock().lines() {
        let line = line.unwrap();
        if line.trim() == "quit" {
            break;
        }
        match line
            .trim()
            .parse::<f64>()
            .ok()
            .and_then(|v| app.update("soc", v).ok())
        {
            Some(s) => println!("ACK SOC={}", s.soc.raw()),
            None => println!("拒绝：请输入 0–1000 的整数"),
        }
    }
    app.stop().unwrap();
    println!("STOPPED");
}
