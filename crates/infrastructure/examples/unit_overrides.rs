//! 单层覆盖验证用无界面入口；仅接受调用者显式命令，不自行连接设备。
#[cfg(feature = "tcp-runtime")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use essim_core::domain::identity::{EndpointId, UnitId};
    use essim_infrastructure::workspace::WorkspaceEngine;
    use std::io::{self, BufRead, Write};
    let root = std::env::args().nth(1).ok_or("需要验证记录目录")?;
    let engine = WorkspaceEngine::with_tcp_runtime(root.into())?;
    for line in io::stdin().lock().lines() {
        let line = line?;
        if line == "quit" {
            break;
        }
        let result = (|| -> Result<serde_json::Value, Box<dyn std::error::Error>> {
            let v: serde_json::Value = serde_json::from_str(&line)?;
            let e = || EndpointId(v["endpoint"].as_str().unwrap_or("").into());
            let u = || UnitId(v["unit"].as_str().unwrap_or("").into());
            Ok(match v["op"].as_str().unwrap_or("") {
                "execute" => serde_json::to_value(
                    engine.execute(serde_json::from_value(v["command"].clone())?)?,
                )?,
                "query" => serde_json::to_value(engine.query()?)?,
                "slave" => serde_json::to_value(
                    engine.execute_slave(serde_json::from_value(v["command"].clone())?)?,
                )?,
                "master" => serde_json::to_value(
                    engine.execute_master(serde_json::from_value(v["command"].clone())?)?,
                )?,
                "masterQuery" => serde_json::to_value(
                    engine.query_master(serde_json::from_value(v["query"].clone())?)?,
                )?,
                "preview" => serde_json::to_value(engine.preview_unit_promotion(
                    e(),
                    u(),
                    serde_json::from_value(v["selection"].clone())?,
                )?)?,
                "capture" => serde_json::to_value(engine.capture_initials(
                    e(),
                    u(),
                    serde_json::from_value(v["points"].clone())?,
                )?)?,
                _ => return Err("未知操作".into()),
            })
        })()
        .map_err(|e| e.to_string());
        println!("{}", serde_json::to_string(&result)?);
        io::stdout().flush()?;
    }
    engine.shutdown_slave();
    Ok(())
}
#[cfg(not(feature = "tcp-runtime"))]
fn main() {
    eprintln!("需要 tcp-runtime feature");
}
