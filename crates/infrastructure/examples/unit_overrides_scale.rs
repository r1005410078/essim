//! 单层覆盖代表性测量：参数为每个 Unit 的覆盖点数，不声明容量上限。
use essim_core::{
    application::{
        catalog::effective_model,
        commands::{Action, Command},
    },
    domain::identity::EndpointId,
};
use essim_infrastructure::workspace::WorkspaceEngine;
use serde_json::json;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let count: usize = std::env::args().nth(1).unwrap_or("0".into()).parse()?;
    let engine = WorkspaceEngine::default();
    let points:Vec<_>=(0..10_000).map(|i|json!({"id":format!("p{i}"),"name":format!("Point {i}"),"category":"measurement","area":"holding","address":i,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000})).collect();
    let model = json!({"id":"m","name":"测量模型","points":points});
    let units:Vec<_>=(0..8).map(|u|{let patches:serde_json::Map<String,serde_json::Value>=(0..count).map(|i|(format!("p{i}"),json!({"initialEngineering":format!("{}",u+20)}))).collect();json!({"id":format!("u{u}"),"name":format!("U{u}"),"station":u+1,"enabled":true,"model":{"id":"m"},"device":null,"overrides":{"pointOverrides":patches}})}).collect();
    let mut endpoint = json!({"id":"e","name":"测量端点","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[units[0].clone()]});
    let run = |action: serde_json::Value| {
        engine.execute(Command {
            command_id: "measure".into(),
            action: serde_json::from_value::<Action>(action).unwrap(),
        })
    };
    run(json!({"kind":"createEndpointWithModel","model":model,"endpoint":endpoint}))?;
    endpoint["units"] = json!(units);
    run(json!({"kind":"saveEndpoint","endpoint":endpoint}))?;
    let start = std::time::Instant::now();
    let preview = engine.preview_apply(EndpointId("e".into()), false)?;
    let preview_ms = start.elapsed().as_secs_f64() * 1000.;
    drop(preview);
    run(json!({"kind":"apply","id":"e","resetIncompatible":false}))?;
    let c = engine.query()?.catalog;
    let unique: std::collections::BTreeSet<_> = c
        .values
        .values()
        .map(|v| std::sync::Arc::as_ptr(v.model.as_ref().unwrap()) as usize)
        .collect();
    let effective_points: usize = c.endpoints[0]
        .units
        .iter()
        .map(|u| effective_model(&c, u).unwrap().points.len())
        .sum();
    println!(
        "{}",
        json!({"pointsPerUnit":10000,"units":8,"overriddenPointsPerUnit":count,"effectivePoints":effective_points,"uniqueAppliedModels":unique.len(),"previewMs":preview_ms,"resource":engine.query_resources()?})
    );
    Ok(())
}
