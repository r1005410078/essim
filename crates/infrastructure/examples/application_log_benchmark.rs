//! 应用诊断入队微基准；不代表协议吞吐或真实磁盘故障性能。
use essim_infrastructure::diagnostics::{Diagnostics, Level};
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
fn main() {
    let directory = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("传入专用测试目录");
    let (log, guard) = Diagnostics::start(directory, "diagnostic-benchmark");
    let mut latencies = Vec::with_capacity(50000);
    let message = "x".repeat(1000);
    let start = Instant::now();
    for _ in 0..50000 {
        let at = Instant::now();
        log.event(Level::Info, "essim", "benchmark", &message, None);
        latencies.push(at.elapsed().as_nanos());
    }
    let elapsed = start.elapsed();
    latencies.sort_unstable();
    let shutdown = guard.shutdown(Duration::from_secs(2));
    println!(
        "{}",
        serde_json::json!({"events":50000,"messageBytes":1000,"durationMs":elapsed.as_millis(),"p50Ns":latencies[25000],"p95Ns":latencies[47500],"p99Ns":latencies[49500],"flushAcknowledged":shutdown,"status":log.status()})
    );
}
