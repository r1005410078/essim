use essim_core::application::workspace::{Folder, Navigation};
use essim_core::domain::identity::{EndpointId, FolderId, ModelId};
use essim_core::domain::navigation::{FolderScope, NavigationNode};
fn folder(id: &str, parent: Option<&str>, scope: FolderScope) -> Folder {
    Folder {
        id: FolderId(id.into()),
        name: id.into(),
        parent: parent.map(|x| FolderId(x.into())),
        scope,
        icon: "folder".into(),
        color: "#477fbc".into(),
    }
}
#[test]
fn legacy_and_styles() {
    let old: Folder =
        serde_json::from_value(serde_json::json!({"id":"a","name":"A","parent":null})).unwrap();
    assert_eq!(old.scope, FolderScope::Simulators);
    assert_eq!(old.icon, "folder");
    let mut n = Navigation::default();
    n.save_folder(old).unwrap();
    assert!(n
        .save_folder(folder("m", Some("a"), FolderScope::Models))
        .is_err());
    let mut invalid = folder("b", None, FolderScope::Simulators);
    invalid.color = "url(x)".into();
    assert!(n.save_folder(invalid).is_err());
}
#[test]
fn mixed_move_is_atomic_and_deduplicates_descendants() {
    let mut n = Navigation::default();
    for f in [
        folder("a", None, FolderScope::Simulators),
        folder("child", Some("a"), FolderScope::Simulators),
        folder("b", None, FolderScope::Simulators),
    ] {
        n.save_folder(f).unwrap();
    }
    n.move_endpoint(EndpointId("e".into()), Some(FolderId("child".into())))
        .unwrap();
    let nodes = vec![
        NavigationNode::Folder {
            id: FolderId("a".into()),
        },
        NavigationNode::Endpoint {
            id: EndpointId("e".into()),
        },
    ];
    n.move_nodes(&nodes, FolderScope::Simulators, Some(FolderId("b".into())))
        .unwrap();
    assert_eq!(n.folders[0].parent, Some(FolderId("b".into())));
    assert_eq!(n.members[&EndpointId("e".into())], FolderId("child".into()));
    let before = n.clone();
    assert!(n
        .move_nodes(
            &[NavigationNode::Folder {
                id: FolderId("b".into())
            }],
            FolderScope::Simulators,
            Some(FolderId("child".into()))
        )
        .is_err());
    assert_eq!(n, before);
    n.move_nodes(
        &[NavigationNode::Folder {
            id: FolderId("a".into()),
        }],
        FolderScope::Simulators,
        None,
    )
    .unwrap();
    assert!(n.folders[0].parent.is_none());
}
#[test]
fn models_and_legacy_shared_folders() {
    let mut n = Navigation::default();
    n.save_folder(folder("a", None, FolderScope::Simulators))
        .unwrap();
    n.model_members
        .insert(ModelId("m".into()), FolderId("a".into()));
    n.normalize_model_folders().unwrap();
    let id = n.model_members[&ModelId("m".into())].clone();
    assert_ne!(id, FolderId("a".into()));
    assert_eq!(
        n.folders.iter().find(|f| f.id == id).unwrap().scope,
        FolderScope::Models
    );
    let before = n.clone();
    n.normalize_model_folders().unwrap();
    assert_eq!(before, n);
    assert!(n
        .move_nodes(
            &[NavigationNode::Model {
                id: ModelId("m".into())
            }],
            FolderScope::Simulators,
            None
        )
        .is_err());
}

#[derive(Default)]
struct Memory(essim_core::application::workspace::Catalog);
impl essim_core::application::workspace::ConfigRepository for Memory {
    fn load(&self) -> essim_core::application::workspace::Catalog {
        self.0.clone()
    }
    fn commit(
        &mut self,
        c: essim_core::application::workspace::Catalog,
    ) -> Result<(), essim_core::domain::error::Error> {
        self.0 = c;
        Ok(())
    }
}
#[test]
fn invalid_batch_and_creation_leave_no_partial_commit() {
    use essim_core::application::workspace::*;
    let mut w = Workspace::new(Memory::default());
    let mut run = |value| {
        w.execute(Command {
            command_id: "test".into(),
            action: serde_json::from_value(value).unwrap(),
        })
    };
    run(serde_json::json!({"kind":"saveFolder","folder":{"id":"m","name":"Models","scope":"models","parent":null}})).unwrap();
    run(serde_json::json!({"kind":"createInFolder","parent":"m","resource":{"kind":"model","model":{"id":"a","name":"A","points":[]}}})).unwrap();
    assert!(run(serde_json::json!({"kind":"moveNavigation","scope":"models","parent":null,"nodes":[{"kind":"model","id":"a"},{"kind":"model","id":"missing"}]})).is_err());
    assert!(run(serde_json::json!({"kind":"createInFolder","parent":"missing","resource":{"kind":"model","model":{"id":"b","name":"B","points":[]}}})).is_err());
    assert_eq!(w.query().catalog.models.len(), 1);
    assert_eq!(
        w.query().catalog.navigation.model_members[&ModelId("a".into())],
        FolderId("m".into())
    );
}
#[test]
fn move_limits_noop_missing_and_style_isolation() {
    let mut n = Navigation::default();
    n.save_folder(folder("a", None, FolderScope::Models))
        .unwrap();
    n.model_members
        .insert(ModelId("m".into()), FolderId("a".into()));
    let before = n.clone();
    let node = NavigationNode::Model {
        id: ModelId("m".into()),
    };
    n.move_nodes(
        &[node.clone(), node],
        FolderScope::Models,
        Some(FolderId("a".into())),
    )
    .unwrap();
    assert_eq!(n, before);
    let nodes: Vec<_> = (0..257)
        .map(|i| NavigationNode::Model {
            id: ModelId(i.to_string()),
        })
        .collect();
    assert!(n.move_nodes(&nodes, FolderScope::Models, None).is_err());
    assert_eq!(n, before);
    assert!(n
        .move_nodes(
            &[NavigationNode::Folder {
                id: FolderId("missing".into())
            }],
            FolderScope::Models,
            None
        )
        .is_err());
    assert_eq!(n, before);
    let mut f = n.folders[0].clone();
    f.name = "Renamed".into();
    f.icon = "pcs".into();
    f.color = "#399f91".into();
    n.save_folder(f).unwrap();
    assert_eq!(n.model_members, before.model_members);
    n.move_nodes(&nodes[..256], FolderScope::Models, None)
        .unwrap();
}
#[test]
fn navigation_commands_preserve_saved_and_applied_business_state() {
    use essim_core::application::workspace::*;
    let mut c = Catalog::default();
    c.models.push(
        serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[]})).unwrap(),
    );
    c.endpoints.push(serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null}]})).unwrap());
    c.applied = c.endpoints.clone();
    let before = c.clone();
    let mut w = Workspace::new(Memory(c));
    w.execute(Command{command_id:"folder".into(),action:serde_json::from_value(serde_json::json!({"kind":"saveFolder","folder":{"id":"f","name":"PCS","parent":null,"icon":"pcs","color":"#399f91"}})).unwrap()}).unwrap();
    w.execute(Command{command_id:"move".into(),action:serde_json::from_value(serde_json::json!({"kind":"moveNavigation","nodes":[{"kind":"endpoint","id":"e"}],"scope":"simulators","parent":"f"})).unwrap()}).unwrap();
    let after = w.query().catalog;
    assert_eq!(before.models, after.models);
    assert_eq!(before.endpoints, after.endpoints);
    assert_eq!(before.applied, after.applied);
    assert_eq!(before.values, after.values);
}
