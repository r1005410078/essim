//! 本机工程存储探测；生成独立临时资源，不连接外部设备。
use essim_core::{
    application::{catalog::Catalog, ports::ConfigRepository},
    domain::model::CollectionModel,
};
use essim_infrastructure::{
    project::ProjectRepository, recording::RecordingPool, resources::ResourceManager,
};
use serde_json::json;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = std::env::temp_dir().join(format!(
        "project-measure-{}-{}",
        std::process::id(),
        SystemTime::now().duration_since(UNIX_EPOCH)?.as_nanos()
    ));
    let mut c = Catalog::default();
    for m in 0..10 {
        let points=(0..1000).map(|p|json!({"id":format!("p-{p}"),"name":format!("点{p}"),"category":"measurement","area":"holding","address":p,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"0"},"initialEngineering":"0","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000})).collect::<Vec<_>>();
        let model: CollectionModel = serde_json::from_value(
            json!({"id":format!("model-{m}"),"name":format!("模型{m}"),"points":points}),
        )?;
        c.models.push(model);
    }
    let start = Instant::now();
    let r = ProjectRepository::create(&root, "测量", c)?;
    let create_ms = start.elapsed().as_millis();
    drop(r);
    let start = Instant::now();
    let mut r = ProjectRepository::open(&root.join("project.workspace"))?;
    let open_ms = start.elapsed().as_millis();
    let mut c = r.load();
    c.models[0].points[0].name = "修改点".into();
    let start = Instant::now();
    r.commit(c)?;
    let save_ms = start.elapsed().as_millis();
    drop(r);
    let pool = RecordingPool::new(ResourceManager::default())?;
    pool.set_project(Some(root.clone()))?;
    let stream = pool.open(root.join("measure.jsonl"))?;
    let mut bytes = serde_json::to_vec(&json!({"kind":"snapshot","endpoint":{"id":"e"}}))?;
    bytes.push(b'\n');
    for n in 1..=1000 {
        let kind = if n == 1000 {
            "stopped"
        } else {
            "valuesCommitted"
        };
        bytes.extend(serde_json::to_vec(&json!({"sequence":n.to_string(),"unixMs":SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis().to_string(),"fact":{"kind":kind,"unit":"u"}}))?);
        bytes.push(b'\n');
    }
    let start = Instant::now();
    let mut completions = vec![];
    for (i, chunk) in bytes.chunks(16384).enumerate() {
        completions.push(stream.append(chunk.to_vec(), (i + 1) as u64)?);
    }
    for c in completions {
        c.wait_timeout(Duration::from_secs(10))?
            .ok_or("录制等待超时")??;
    }
    drop(stream);
    pool.flush_project()?;
    let write_ms = start.elapsed().as_millis();
    let view = pool.project_view().ok_or("无数据库投影")?;
    let count: i64 = rusqlite::Connection::open(root.join("runs/measure/runtime.sqlite"))?
        .query_row("select count(*) from events", [], |r| r.get(0))?;
    println!(
        "{}",
        json!({"os":std::env::consts::OS,"arch":std::env::consts::ARCH,"models":10,"pointsPerModel":1000,"totalPoints":10000,"createMs":create_ms,"openMs":open_ms,"saveOneModelMs":save_ms,"runEvents":count,"recordBytes":bytes.len(),"jsonlAndSqliteFlushMs":write_ms,"sqliteFailures":view.failed_runs,"queuedBytesAfterFlush":view.queued_bytes,"peakQueuedBytes":view.peak_queued_bytes})
    );
    drop(pool);
    std::fs::remove_dir_all(root)?;
    Ok(())
}
