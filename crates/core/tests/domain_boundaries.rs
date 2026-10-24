//! 直接领域调用维护局部不变量，不依赖应用处理器或窗口。
use essim_core::domain::{
    endpoint::EndpointConfig,
    identity::{EndpointId, FolderId},
    navigation::{Folder, Navigation},
};
fn folder(id: &str, parent: Option<&str>) -> Folder {
    Folder {
        id: FolderId(id.into()),
        name: id.into(),
        parent: parent.map(|id| FolderId(id.into())),
        scope: Default::default(),
        icon: "folder".into(),
        color: "#477fbc".into(),
    }
}
#[test]
fn navigation_rejects_cycles_and_nonempty_delete_without_partial_mutation() {
    let mut nav = Navigation::default();
    nav.save_folder(folder("a", None)).unwrap();
    nav.save_folder(folder("b", Some("a"))).unwrap();
    let before = nav.clone();
    assert_eq!(
        nav.save_folder(folder("a", Some("b"))).unwrap_err().code,
        "FOLDER_CYCLE"
    );
    assert_eq!(nav, before);
    assert_eq!(
        nav.delete_folder(&FolderId("a".into())).unwrap_err().code,
        "FOLDER_NOT_EMPTY"
    );
    nav.move_endpoint(EndpointId("e".into()), Some(FolderId("b".into())))
        .unwrap();
    let before = nav.clone();
    assert!(nav.delete_folder(&FolderId("b".into())).is_err());
    assert!(nav
        .move_endpoint(EndpointId("e".into()), Some(FolderId("missing".into())))
        .is_err());
    assert_eq!(nav, before);
}
#[test]
fn endpoint_type_invariant_is_available_without_a_handler() {
    let original:EndpointConfig=serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[]})).unwrap();
    let mut changed = original.clone();
    changed.role = essim_core::domain::endpoint::Role::Master;
    assert_eq!(
        original.ensure_same_type(&changed).unwrap_err().code,
        "ENDPOINT_TYPE_IMMUTABLE"
    );
    assert_eq!(original.role, essim_core::domain::endpoint::Role::Slave);
}
