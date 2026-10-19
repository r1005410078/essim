//! 无界面主站验证入口；一行一个运行命令或查询，不依赖Tauri窗口。
#[cfg(feature = "tcp-runtime")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use essim_core::application::{commands::master::MasterCommand, queries::master::MasterQuery};
    use essim_infrastructure::tcp_master::{MasterSnapshot, TcpMasterRuntime};
    use std::io::{self, BufRead, Write};
    let mut args = std::env::args().skip(1);
    let snapshot: MasterSnapshot =
        serde_json::from_slice(&std::fs::read(args.next().ok_or("需要主站快照JSON路径")?)?)?;
    let runtime = TcpMasterRuntime::new(args.next().ok_or("需要记录目录")?.into());
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "quit" {
            break;
        }
        let v: serde_json::Value = serde_json::from_str(&line)?;
        let result = if let Some(q) = v.get("query") {
            runtime.query(serde_json::from_value::<MasterQuery>(q.clone())?)
        } else {
            let command: MasterCommand = serde_json::from_value(v)?;
            let start = matches!(
                command.action,
                essim_core::application::commands::master::MasterAction::Connect
            );
            runtime.execute(command, start.then(|| snapshot.clone()))
        };
        println!("{}", serde_json::to_string(&result)?);
        io::stdout().flush()?;
    }
    runtime.shutdown();
    Ok(())
}
#[cfg(not(feature = "tcp-runtime"))]
fn main() {
    eprintln!("需要--features tcp-runtime,vendored");
    std::process::exit(2);
}
