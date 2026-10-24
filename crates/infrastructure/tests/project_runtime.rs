use essim_infrastructure::{
    recording::{write_json, RecordingPool},
    resources::ResourceManager,
};
use std::time::{SystemTime, UNIX_EPOCH};
#[test]
fn reliable_recording_projects_static_context_and_selected_events() {
    let root = std::env::temp_dir().join(format!(
        "project-run-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let pool = RecordingPool::new(ResourceManager::default()).unwrap();
    pool.set_project(Some(root.clone())).unwrap();
    let stream = pool.open(root.join("unused/run-unique.jsonl")).unwrap();
    assert_eq!(
        stream.path().unwrap(),
        root.join("runs/run-unique/recordings/protocol.jsonl")
    );
    let mut sequence = 0;
    for value in [
        serde_json::json!({"kind":"snapshot","endpoint":{"id":"e"},"models":[],"units":[]}),
        serde_json::json!({"sequence":"1","unixMs":"100","fact":{"kind":"listening"}}),
        serde_json::json!({"sequence":"2","unixMs":"101","fact":{"kind":"request","bytes":[1,2]}}),
        serde_json::json!({"sequence":"3","unixMs":"102","fact":{"kind":"valuesCommitted","unit":"u"}}),
        serde_json::json!({"sequence":"4","unixMs":"103","fact":{"kind":"stopped"}}),
    ] {
        write_json(&stream, &mut sequence, &value).unwrap();
    }
    pool.flush_project().unwrap();
    let dir = root.join("runs/run-unique");
    let c = rusqlite::Connection::open(dir.join("runtime.sqlite")).unwrap();
    let count: i64 = c
        .query_row("SELECT count(*) FROM events", [], |r| r.get(0))
        .unwrap();
    assert_eq!(count, 3);
    let unit: String = c
        .query_row(
            "SELECT unit FROM events WHERE kind='valuesCommitted'",
            [],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(unit, "u");
    let context: serde_json::Value =
        serde_json::from_slice(&std::fs::read(dir.join("context.json")).unwrap()).unwrap();
    assert_eq!(context["complete"], true);
    assert!(context["createdUnixMs"].as_str().is_some());
    drop(stream);
    assert_eq!(pool.project_view().unwrap().failed_runs, 0);
    assert!(dir.join("applied.json").exists());
    drop(c);
    drop(pool);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn sqlite_failure_does_not_reject_reliable_recording() {
    let root = std::env::temp_dir().join(format!(
        "project-run-fail-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let dir = root.join("runs/fail");
    std::fs::create_dir_all(dir.join("runtime.sqlite")).unwrap();
    let pool = RecordingPool::new(ResourceManager::default()).unwrap();
    pool.set_project(Some(root.clone())).unwrap();
    let stream = pool.open(root.join("fail.jsonl")).unwrap();
    let mut sequence = 0;
    write_json(
        &stream,
        &mut sequence,
        &serde_json::json!({"kind":"snapshot","endpoint":{"id":"e"}}),
    )
    .unwrap();
    drop(stream);
    pool.flush_project().unwrap();
    assert_eq!(pool.project_view().unwrap().failed_runs, 1);
    assert!(dir.join("recordings/protocol.jsonl").exists());
    drop(pool);
    std::fs::remove_dir_all(root).unwrap();
}
