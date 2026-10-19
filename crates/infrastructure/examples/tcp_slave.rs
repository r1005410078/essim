//! 无界面TCP从站验证入口，读取完整快照文件；示例点表由调用者提供。
#[cfg(feature = "tcp-runtime")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use essim_core::application::commands::slave::{SlaveAction, SlaveCommand};
    use essim_infrastructure::tcp_slave::{Snapshot, TcpSlaveRuntime};
    use std::io::{self, BufRead, Write};
    let mut args = std::env::args().skip(1);
    let file = args.next().ok_or("需要完整Snapshot JSON路径")?;
    let root = args.next().ok_or("需要运行记录目录")?;
    let snapshots: Vec<Snapshot> = serde_json::from_slice(&std::fs::read(file)?)?;
    let runtime = TcpSlaveRuntime::new(root.into())?;
    for snapshot in snapshots {
        let id = snapshot.endpoint.id.clone();
        let view = runtime
            .execute(
                SlaveCommand {
                    command_id: "headless-start".into(),
                    endpoint: id,
                    action: SlaveAction::Start { max_clients: 10 },
                },
                Some(snapshot),
            )
            .map_err(|e| e.message)?;
        println!("{}", serde_json::to_string(&view)?);
    }
    println!("READY");
    io::stdout().flush()?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "quit" {
            break;
        }
        let command: SlaveCommand = serde_json::from_str(&line)?;
        let result = runtime.execute(command, None).map_err(|e| e.message);
        println!("{}", serde_json::to_string(&result)?);
        io::stdout().flush()?;
    }
    drop(runtime);
    Ok(())
}
#[cfg(not(feature = "tcp-runtime"))]
fn main() {
    eprintln!("需要--features tcp-runtime");
    std::process::exit(2);
}
