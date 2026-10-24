use essim_core::{
    application::{catalog::Catalog, ports::ConfigRepository},
    domain::{identity::ModelId, model::CollectionModel},
};
use essim_infrastructure::project::ProjectRepository;
use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!(
            "project-storage-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Self(p)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn catalog() -> Catalog {
    let mut c = Catalog::default();
    c.models.push(CollectionModel {
        id: ModelId("stable/model".into()),
        name: "PCS 点表".into(),
        points: vec![],
    });
    c
}
#[test]
fn saved_resources_roundtrip_and_manifest_owns_names() {
    let t = Temp::new();
    let mut r = ProjectRepository::create(&t.0, "电站", catalog()).unwrap();
    let path = r.manifest().resources[0].path.clone();
    let body: serde_json::Value =
        serde_json::from_slice(&std::fs::read(t.0.join(&path)).unwrap()).unwrap();
    assert!(body.get("name").is_none());
    let mut c = r.load();
    c.models[0].name = "改名".into();
    r.commit(c).unwrap();
    assert_eq!(r.manifest().resources[0].path, path);
    drop(r);
    let r = ProjectRepository::open(&t.0.join("project.workspace")).unwrap();
    assert_eq!(r.load().models[0].name, "改名");
}
#[test]
fn rejects_duplicate_ids_before_creating_project() {
    let t = Temp::new();
    let mut c = catalog();
    c.models.push(c.models[0].clone());
    assert!(ProjectRepository::create(&t.0, "电站", c).is_err());
    assert!(!t.0.join("project.workspace").exists());
}
#[test]
fn concurrent_writer_and_external_changes_are_rejected() {
    let t = Temp::new();
    let mut r = ProjectRepository::create(&t.0, "电站", catalog()).unwrap();
    assert!(ProjectRepository::open(&t.0.join("project.workspace")).is_err());
    let p = t.0.join(&r.manifest().resources[0].path);
    std::fs::write(&p, b"external").unwrap();
    assert!(r.commit(catalog()).is_err());
    assert_eq!(std::fs::read(p).unwrap(), b"external");
}
#[test]
fn rejects_path_escape_and_future_format() {
    for (key, value) in [
        ("path", serde_json::json!("../escape.json")),
        ("formatVersion", serde_json::json!(999)),
    ] {
        let t = Temp::new();
        let r = ProjectRepository::create(&t.0, "电站", catalog()).unwrap();
        drop(r);
        let p = t.0.join("project.workspace");
        let mut m: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        if key == "path" {
            m["resources"][0][key] = value;
        } else {
            m[key] = value;
        }
        std::fs::write(&p, serde_json::to_vec(&m).unwrap()).unwrap();
        assert!(ProjectRepository::open(&p).is_err());
    }
}
#[cfg(unix)]
#[test]
fn rejects_symlink_resource() {
    let t = Temp::new();
    let other = Temp::new();
    let r = ProjectRepository::create(&t.0, "电站", catalog()).unwrap();
    let p = t.0.join(&r.manifest().resources[0].path);
    drop(r);
    std::fs::rename(&p, other.0.join("data")).unwrap();
    std::os::unix::fs::symlink(other.0.join("data"), &p).unwrap();
    assert!(ProjectRepository::open(&t.0.join("project.workspace")).is_err());
}
#[test]
fn applied_definition_and_unit_overrides_survive_without_runtime_values() {
    use essim_core::domain::{
        identity::{PointId, UnitId},
        point_table::RawValue,
        values::{migrate, UnitValues},
    };
    let t = Temp::new();
    let mut c = Catalog::default();
    c.models.push(serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"V","initial":{"kind":"unsigned","value":"12"},"initialEngineering":"12","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":false,"masterPeriodMs":1000}]})).unwrap());
    c.endpoints.push(serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"Unit 1","station":1,"enabled":true,"model":{"id":"m"},"device":null,"overrides":{"pointOverrides":{"p":{"address":10,"initialEngineering":"20"}}}}]})).unwrap());
    let model =
        essim_core::application::catalog::effective_model(&c, &c.endpoints[0].units[0]).unwrap();
    let mut values = migrate(None, &UnitValues::default(), &model, true)
        .unwrap()
        .values;
    values
        .raw
        .insert(PointId("p".into()), RawValue::Unsigned(99));
    c.values.insert(UnitId("u".into()), values);
    c.applied = c.endpoints.clone();
    c.models[0].points[0].address = 30;
    let r = ProjectRepository::create(&t.0, "P", c.clone()).unwrap();
    drop(r);
    let r = ProjectRepository::open(&t.0.join("project.workspace")).unwrap();
    let restored = r.load();
    assert_eq!(restored.models, c.models);
    assert_eq!(restored.endpoints, c.endpoints);
    let v = &restored.values[&UnitId("u".into())];
    assert_eq!(v.model.as_ref().unwrap().points[0].address, 10);
    assert_eq!(v.raw[&PointId("p".into())], RawValue::Unsigned(20));
}
#[test]
fn malformed_navigation_and_missing_model_do_not_create_files() {
    use essim_core::domain::{
        identity::FolderId,
        navigation::{Folder, FolderScope},
    };
    for cross_scope in [false, true] {
        let t = Temp::new();
        let mut c = catalog();
        c.navigation.folders = vec![
            Folder {
                id: FolderId("a".into()),
                name: "A".into(),
                parent: Some(FolderId("b".into())),
                scope: FolderScope::Models,
                icon: "folder".into(),
                color: "#123456".into(),
            },
            Folder {
                id: FolderId("b".into()),
                name: "B".into(),
                parent: if cross_scope {
                    None
                } else {
                    Some(FolderId("a".into()))
                },
                scope: if cross_scope {
                    FolderScope::Simulators
                } else {
                    FolderScope::Models
                },
                icon: "folder".into(),
                color: "#123456".into(),
            },
        ];
        assert!(ProjectRepository::create(&t.0, "P", c).is_err());
        assert!(!t.0.join("project.workspace").exists());
    }
    let t = Temp::new();
    let mut c = Catalog::default();
    c.endpoints.push(serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"missing"}}]})).unwrap());
    assert!(ProjectRepository::create(&t.0, "P", c).is_err());
}
#[test]
fn recent_project_is_separate_and_clearing_does_not_create_a_project() {
    use essim_infrastructure::project::RecentProject;
    let t = Temp::new();
    let recent = RecentProject::new(&t.0);
    assert_eq!(recent.load().unwrap(), None);
    let entry = t.0.join("portable/project.workspace");
    recent.save(Some(&entry)).unwrap();
    assert_eq!(recent.load().unwrap(), Some(entry));
    recent.save(None).unwrap();
    assert_eq!(recent.load().unwrap(), None);
    assert!(!t.0.join("project.workspace").exists());
}
#[test]
fn engine_switches_repository_without_resetting_notification_sequence() {
    use essim_infrastructure::workspace::WorkspaceEngine;
    let a = Temp::new();
    let b = Temp::new();
    let engine = WorkspaceEngine::default();
    let first = engine.create_project(&a.0, "A").unwrap();
    let n = engine.sequence().unwrap();
    engine.close_project().unwrap();
    engine.create_project(&b.0, "B").unwrap();
    let reopened = engine.open_project(&first.entry).unwrap();
    assert!(engine.sequence().unwrap() > n);
    assert_eq!(engine.project_info().unwrap().unwrap().id, first.id);
    assert!(engine
        .write_project_local("old", "workspace", &serde_json::json!({}))
        .is_err());
    engine
        .write_project_local(&reopened.session, "workspace", &serde_json::json!({"x":1}))
        .unwrap();
    assert_eq!(
        engine
            .read_project_local(&reopened.session, "workspace")
            .unwrap()
            .unwrap()["x"],
        1
    );
}
#[test]
fn documents_and_binary_assets_remain_unchanged_when_configuration_is_saved() {
    let t = Temp::new();
    let mut r = ProjectRepository::create(&t.0, "P", catalog()).unwrap();
    let doc = "# 接入说明\n\n![设备](../../assets/example.png)\n";
    let asset = [0, 255, 128, 2];
    r.save_document("doc", "说明", doc).unwrap();
    r.save_asset("asset", "设备.PNG", &asset).unwrap();
    let path = r
        .manifest()
        .resources
        .iter()
        .find(|r| r.id == "doc")
        .unwrap()
        .path
        .clone();
    let ap = r.manifest().assets[0].path.clone();
    let mut c = r.load();
    c.models[0].name = "重命名".into();
    r.commit(c).unwrap();
    drop(r);
    let r = ProjectRepository::open(&t.0.join("project.workspace")).unwrap();
    assert_eq!(std::fs::read_to_string(t.0.join(path)).unwrap(), doc);
    assert_eq!(std::fs::read(t.0.join(ap)).unwrap(), asset);
    assert_eq!(r.load().models[0].name, "重命名");
}
#[test]
fn first_command_after_switch_does_not_commit_then_report_sequence_gap() {
    use essim_core::application::commands::{Action, Command};
    use essim_infrastructure::workspace::WorkspaceEngine;
    let t = Temp::new();
    let engine = WorkspaceEngine::default();
    engine.create_project(&t.0, "P").unwrap();
    engine
        .execute(Command {
            command_id: "first".into(),
            action: Action::SaveModel {
                model: catalog().models.remove(0),
            },
        })
        .unwrap();
    assert_eq!(engine.query().unwrap().catalog.models.len(), 1);
}
#[test]
fn copied_project_identity_cannot_accept_local_writes_from_previous_open() {
    use essim_infrastructure::workspace::WorkspaceEngine;
    let a = Temp::new();
    let b = Temp::new();
    let engine = WorkspaceEngine::default();
    let first = engine.create_project(&a.0, "P").unwrap();
    std::fs::copy(a.0.join("project.workspace"), b.0.join("project.workspace")).unwrap();
    let second = engine.open_project(&b.0.join("project.workspace")).unwrap();
    assert_eq!(first.id, second.id);
    assert_ne!(first.session, second.session);
    assert!(engine
        .write_project_local(
            &first.session,
            "workspace",
            &serde_json::json!({"old":true})
        )
        .is_err());
    engine
        .write_project_local(
            &second.session,
            "workspace",
            &serde_json::json!({"new":true}),
        )
        .unwrap();
    assert!(engine
        .read_project_local(&second.session, "workspace")
        .unwrap()
        .unwrap()["new"]
        .as_bool()
        .unwrap());
}
#[test]
fn failed_target_open_keeps_current_project_and_configuration() {
    use essim_infrastructure::workspace::WorkspaceEngine;
    let a = Temp::new();
    let b = Temp::new();
    let engine = WorkspaceEngine::default();
    let first = engine.create_project(&a.0, "P").unwrap();
    std::fs::write(b.0.join("project.workspace"), b"broken").unwrap();
    assert!(engine.open_project(&b.0.join("project.workspace")).is_err());
    assert_eq!(
        engine.project_info().unwrap().unwrap().session,
        first.session
    );
}
#[test]
fn project_stop_failure_keeps_old_workspace_and_queries_remain_available() {
    use essim_core::{
        application::{
            commands::{Action, Command},
            ports::ApplyBoundary,
        },
        domain::{endpoint::EndpointConfig, error::Error},
    };
    use essim_infrastructure::workspace::WorkspaceEngine;
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};
    struct Boundary {
        entered: mpsc::Sender<()>,
        release: mpsc::Receiver<()>,
    }
    impl ApplyBoundary for Boundary {
        fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
            self.entered.send(()).unwrap();
            let _ = self.release.recv_timeout(Duration::from_secs(5));
            Err(Error::new("TEST_RELEASE_TIMEOUT", "注入停机未确认"))
        }
    }
    let target = Temp::new();
    let r = ProjectRepository::create(&target.0, "目标", Catalog::default()).unwrap();
    drop(r);
    let (tx, entered) = mpsc::channel();
    let (release, rx) = mpsc::channel();
    let engine = Arc::new(WorkspaceEngine::with_boundary(Box::new(Boundary {
        entered: tx,
        release: rx,
    })));
    engine
        .execute(Command {
            command_id: "model".into(),
            action: Action::SaveModel {
                model: catalog().models.remove(0),
            },
        })
        .unwrap();
    let endpoint=serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"stable/model"}}]})).unwrap();
    engine
        .execute(Command {
            command_id: "endpoint".into(),
            action: Action::CreateEndpoint { endpoint },
        })
        .unwrap();
    let e = engine.clone();
    let entry = target.0.join("project.workspace");
    let worker = std::thread::spawn(move || e.open_project(&entry));
    entered.recv_timeout(Duration::from_secs(2)).unwrap();
    let start = Instant::now();
    let query = engine.query().unwrap();
    let elapsed = start.elapsed();
    let _ = release.send(());
    let error = worker.join().unwrap().unwrap_err();
    assert_eq!(error.code, "TEST_RELEASE_TIMEOUT");
    assert!(elapsed < Duration::from_secs(1));
    assert_eq!(query.catalog.endpoints.len(), 1);
    assert_eq!(engine.query().unwrap().catalog.endpoints.len(), 1);
    assert!(engine.project_info().unwrap().is_none());
}
#[test]
fn engine_loads_documents_by_stable_identity_and_rejects_stale_handles() {
    use essim_infrastructure::workspace::WorkspaceEngine;
    let t = Temp::new();
    let mut r = ProjectRepository::create(&t.0, "P", Catalog::default()).unwrap();
    r.save_document("doc", "接入说明", "# 接入说明\n\n工程正文")
        .unwrap();
    drop(r);
    let e = WorkspaceEngine::default();
    let info = e.open_project(&t.0.join("project.workspace")).unwrap();
    assert_eq!(info.documents[0].name, "接入说明");
    assert_eq!(
        e.read_project_document(&info.session, "doc").unwrap(),
        "# 接入说明\n\n工程正文"
    );
    assert!(e.read_project_document("old", "doc").is_err());
    assert!(e
        .read_project_document(&info.session, "../project.workspace")
        .is_err());
}
#[test]
fn concurrent_recent_settings_writers_never_publish_partial_json() {
    use essim_infrastructure::project::RecentProject;
    let t = Temp::new();
    let writers = (0..8)
        .map(|i| {
            let root = t.0.clone();
            std::thread::spawn(move || {
                let r = RecentProject::new(&root);
                for n in 0..10 {
                    r.save(Some(&root.join(format!("{i}-{n}/project.workspace"))))
                        .unwrap();
                }
            })
        })
        .collect::<Vec<_>>();
    for writer in writers {
        writer.join().unwrap();
    }
    assert!(RecentProject::new(&t.0).load().unwrap().is_some());
}
