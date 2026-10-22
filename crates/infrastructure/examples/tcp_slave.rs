//! 无界面TCP从站验证入口，读取完整快照文件；示例点表由调用者提供。
#[cfg(feature = "tcp-runtime")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use essim_core::application::commands::slave::{SlaveAction, SlaveCommand};
    use essim_infrastructure::tcp_slave::{Snapshot, TcpSlaveRuntime};
    use std::io::{self, BufRead, Write};
    let mut args = std::env::args().skip(1);
    let file = args.next().ok_or("需要完整Snapshot JSON路径")?;
    let root = args.next().ok_or("需要运行记录目录")?;
    let policy = essim_infrastructure::resources::ResourcePolicy::default();
    if std::fs::metadata(&file)?.len().saturating_mul(8) > policy.limit.memory_bytes {
        return Err("RESOURCE_LIMIT: 输入暂存长度×8超过2GiB工程预算".into());
    }
    let mut snapshots: Vec<Snapshot> = serde_json::from_slice(&std::fs::read(file)?)?;
    let mut models: std::collections::BTreeMap<
        String,
        Vec<std::sync::Arc<essim_core::domain::model::CollectionModel>>,
    > = Default::default();
    for snapshot in &mut snapshots {
        for values in snapshot.values.values_mut() {
            if let Some(model) = values.model.as_mut() {
                let pool = models.entry(model.id.0.clone()).or_default();
                if let Some(shared) = pool.iter().find(|m| m.as_ref() == model.as_ref()) {
                    *model = shared.clone();
                } else {
                    pool.push(model.clone());
                }
            }
        }
    }

    let runtime = TcpSlaveRuntime::new(root.into())?;
    for snapshot in snapshots.iter().cloned() {
        let id = snapshot.endpoint.id.clone();
        let mut completion = runtime.execute_pending(
            SlaveCommand {
                command_id: "headless-start".into(),
                endpoint: id,
                action: SlaveAction::Start { max_clients: 10 },
            },
            Some(snapshot),
        )?;
        let view = loop {
            if let Some(result) = completion.wait_timeout(std::time::Duration::from_secs(1))? {
                break result?;
            }
        };
        println!("{}", serde_json::to_string(&view)?);
    }
    println!("READY");
    io::stdout().flush()?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "quit" {
            break;
        }
        let value: serde_json::Value = serde_json::from_str(&line)?;
        let result = if let Some(query) = value.get("query") {
            runtime
                .query(serde_json::from_value(query.clone())?)
                .map_err(|e| e.message)
        } else {
            let command: SlaveCommand = serde_json::from_value(value)?;
            let snapshot = if matches!(command.action, SlaveAction::Start { .. }) {
                snapshots
                    .iter()
                    .find(|s| s.endpoint.id == command.endpoint)
                    .cloned()
            } else {
                None
            };
            runtime.execute(command, snapshot).map_err(|e| e.message)
        };
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
