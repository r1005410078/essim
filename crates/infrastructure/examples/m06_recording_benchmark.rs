//! 本机录制池微基准：独立事实可合批，因果阶段仍逐阶段等待；不是协议吞吐结论。
use essim_infrastructure::{recording::RecordingPool,resources::ResourceManager};
use std::{time::{Duration,Instant},path::PathBuf};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let root:PathBuf=std::env::temp_dir().join(format!("essim-record-bench-{}",std::process::id()));
 let pool=RecordingPool::new(ResourceManager::default())?;
 let sequential=pool.open(root.join("sequential.jsonl"))?;
 let mut times=Vec::new();let start=Instant::now();
 for sequence in 1..=100 {let t=Instant::now();let receipt=sequential.append(vec![b'x';512],sequence)?;receipt.wait_timeout(Duration::from_secs(5))?.ok_or("同步超时")??;times.push(t.elapsed().as_secs_f64()*1000.);}
 let sequential_ms=start.elapsed().as_secs_f64()*1000.;let first_syncs=pool.query()?.syncs;
 let batched=pool.open(root.join("independent.jsonl"))?;let start=Instant::now();let mut jobs=Vec::new();
 for sequence in 1..=100{jobs.push(batched.append(vec![b'x';512],sequence)?);}
 for job in jobs {job.wait_timeout(Duration::from_secs(5))?.ok_or("同步超时")??;}
 let batched_ms=start.elapsed().as_secs_f64()*1000.;let independent_syncs=pool.query()?.syncs-first_syncs;
 times.sort_by(f64::total_cmp);
 println!("{}",serde_json::to_string_pretty(&serde_json::json!({"build":"debug","records":100,"bytesPerRecord":512,"policy":{"workers":4,"batchBytes":65536,"waitMs":5},"sequentialMilliseconds":sequential_ms,"sequentialSyncs":first_syncs,"sequentialP50Ms":times[49],"sequentialP95Ms":times[94],"sequentialP99Ms":times[98],"independentMilliseconds":batched_ms,"independentSyncs":independent_syncs,"reliablePrefix":batched.reliable()?,"scope":"isolated file microbenchmark; independent synthetic bytes, not protocol success; runtime causal stages cannot be batched ahead of actions"}))?);
 drop(sequential);drop(batched);drop(pool);let _=std::fs::remove_dir_all(root);Ok(())
}
