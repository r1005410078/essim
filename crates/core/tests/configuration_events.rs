//! 配置提交事实的身份、原子性与只读边界；不代表持久化事件溯源。
use essim_core::{application::workspace::*, domain::foundation::*};
#[derive(Default)]
struct Memory {
    catalog: Catalog,
    fail: bool,
}
impl ConfigRepository for Memory {
    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, c: Catalog) -> Result<(), Error> {
        if self.fail {
            return Err(Error::new("STORE_FAILED", "提交失败"));
        }
        self.catalog = c;
        Ok(())
    }
}
fn command(action: serde_json::Value) -> Command {
    serde_json::from_value(serde_json::json!({"commandId":"cause","action":action})).unwrap()
}
fn endpoint() -> serde_json::Value {
    serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1502},"units":[{"id":"u","name":"U","station":1,"enabled":true,"device":null,"model":{"id":"m"}}]})
}
#[test]
fn committed_actions_have_specific_facts_and_matching_snapshots() {
    let mut w = Workspace::new(Memory::default());
    let cases = vec![
        (
            serde_json::json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}}),
            serde_json::json!({"kind":"modelSaved","id":"m"}),
        ),
        (
            serde_json::json!({"kind":"copyModel","source":{"id":"m"},"id":"copy-m","name":"Copy"}),
            serde_json::json!({"kind":"modelCopied","source":"m","id":"copy-m"}),
        ),
        (
            serde_json::json!({"kind":"deleteModel","id":"copy-m"}),
            serde_json::json!({"kind":"modelDeleted","id":"copy-m"}),
        ),
        (
            serde_json::json!({"kind":"createDevice","device":{"id":"d","name":"D","kind":"PCS"}}),
            serde_json::json!({"kind":"deviceCreated","id":"d"}),
        ),
        (
            serde_json::json!({"kind":"createEndpoint","endpoint":endpoint()}),
            serde_json::json!({"kind":"endpointCreated","id":"e"}),
        ),
        (
            serde_json::json!({"kind":"saveEndpoint","endpoint":endpoint()}),
            serde_json::json!({"kind":"endpointSaved","id":"e"}),
        ),
        (
            serde_json::json!({"kind":"copyEndpoint","source":"e","id":"copy-e"}),
            serde_json::json!({"kind":"endpointCopied","source":"e","id":"copy-e"}),
        ),
        (
            serde_json::json!({"kind":"apply","id":"e","resetIncompatible":false}),
            serde_json::json!({"kind":"endpointApplied","id":"e"}),
        ),
        (
            serde_json::json!({"kind":"resetUnit","id":"e","unit":"u"}),
            serde_json::json!({"kind":"unitValuesReset","id":"e","unit":"u"}),
        ),
        (
            serde_json::json!({"kind":"saveFolder","folder":{"id":"f","name":"F","parent":null}}),
            serde_json::json!({"kind":"folderSaved","id":"f"}),
        ),
        (
            serde_json::json!({"kind":"moveEndpoint","id":"copy-e","parent":"f"}),
            serde_json::json!({"kind":"endpointMoved","id":"copy-e","parent":"f"}),
        ),
        (
            serde_json::json!({"kind":"deleteEndpoint","id":"copy-e"}),
            serde_json::json!({"kind":"endpointDeleted","id":"copy-e"}),
        ),
        (
            serde_json::json!({"kind":"deleteFolder","id":"f"}),
            serde_json::json!({"kind":"folderDeleted","id":"f"}),
        ),
    ];
    for (action, fact) in cases {
        let (receipt, event) = w.execute(command(action)).unwrap();
        assert_eq!(serde_json::to_value(&event).unwrap()["fact"], fact);
        assert_eq!(receipt.object_id, event.object_id);
        assert_eq!(event.command_id, "cause");
        assert_eq!(event.catalog, w.query().catalog);
        assert_eq!(receipt.sequence, w.sequence());
    }
}
#[test]
fn invalid_commands_and_failed_commit_have_no_success_event_or_sequence_advance() {
    let mut w = Workspace::new(Memory {
        fail: true,
        ..Default::default()
    });
    let before = w.query();
    let result = w.execute(command(
        serde_json::json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}}),
    ));
    assert_eq!(result.unwrap_err().code, "STORE_FAILED");
    assert_eq!(w.sequence(), before.sequence);
    assert_eq!(w.query().catalog, before.catalog);
    let mut w = Workspace::new(Memory::default());
    let before = w.query();
    assert!(w
        .execute(command(
            serde_json::json!({"kind":"createEndpoint","endpoint":endpoint()})
        ))
        .is_err());
    assert_eq!(w.sequence(), before.sequence);
    assert_eq!(w.query().catalog, before.catalog);
}
#[test]
fn query_preview_and_inspection_do_not_commit_or_advance_sequence() {
    let mut w = Workspace::new(Memory::default());
    w.execute(command(
        serde_json::json!({"kind":"saveModel","model":{"id":"m","name":"M","points":[]}}),
    ))
    .unwrap();
    w.execute(command(
        serde_json::json!({"kind":"createEndpoint","endpoint":endpoint()}),
    ))
    .unwrap();
    let before = serde_json::to_value(w.query()).unwrap();
    w.preview_apply(&EndpointId("e".into()), false).unwrap();
    let catalog = w.query().catalog;
    essim_core::application::queries::inspect_endpoint(&catalog, &catalog.endpoints[0]);
    assert_eq!(serde_json::to_value(w.query()).unwrap(), before);
}
struct FailedBoundary;
impl ApplyBoundary for FailedBoundary {
    fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
        Err(Error::new("STOP_FAILED", "没有释放确认"))
    }
    fn state(&self, _: &EndpointId) -> ApplyState {
        ApplyState::Running
    }
}
#[test]
fn failed_release_has_no_success_fact_and_keeps_original_config_sequence() {
    let mut memory = Memory::default();
    memory.catalog.models.push(
        serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[]})).unwrap(),
    );
    memory
        .catalog
        .endpoints
        .push(serde_json::from_value(endpoint()).unwrap());
    let mut w = Workspace::with_boundary(memory, FailedBoundary);
    let before = w.query().catalog;
    for kind in ["apply", "deleteEndpoint"] {
        let result = w.execute(command(
            serde_json::json!({"kind":kind,"id":"e","resetIncompatible":false}),
        ));
        assert_eq!(result.unwrap_err().code, "STOP_FAILED");
        assert_eq!(w.sequence(), 0);
        assert_eq!(w.query().catalog, before);
        assert_eq!(
            w.application_state(&EndpointId("e".into())),
            Some(ApplyState::Running)
        );
    }
}
