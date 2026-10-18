//! 同步事件消费者拒绝损坏通知，并保持查询投影的幂等性。
use essim_core::{
    application::{
        catalog::Catalog,
        event_handlers::WorkspaceProjectionListener,
        events::{ConfigEvent, ConfigurationFact},
        queries::Projection,
    },
    domain::{identity::ModelId, model::CollectionModel},
};

#[test]
fn listener_updates_snapshot_and_rejects_invalid_notifications_without_changes() {
    let mut projection = Projection::default();
    let event = ConfigEvent {
        schema: 1,
        fact: ConfigurationFact::ModelSaved {
            id: ModelId("m".into()),
        },
        sequence: 1,
        command_id: "save-model".into(),
        object_id: "m".into(),
        catalog: Catalog {
            models: vec![CollectionModel {
                id: ModelId("m".into()),
                name: "模型".into(),
                points: vec![],
            }],
            ..Default::default()
        },
    };
    assert!(WorkspaceProjectionListener::handle(&mut projection, &event).unwrap());
    assert_eq!(projection.query().catalog, event.catalog);
    assert_eq!(projection.query().sequence, 1);
    let mut altered = event.clone();
    altered.catalog = Catalog::default();
    assert!(!WorkspaceProjectionListener::handle(&mut projection, &altered).unwrap());
    altered.sequence = 3;
    assert_eq!(
        WorkspaceProjectionListener::handle(&mut projection, &altered)
            .unwrap_err()
            .code,
        "SEQUENCE_GAP"
    );
    altered.sequence = 2;
    altered.schema = 99;
    assert_eq!(
        WorkspaceProjectionListener::handle(&mut projection, &altered)
            .unwrap_err()
            .code,
        "UNKNOWN_SCHEMA"
    );
    assert_eq!(projection.query().catalog, event.catalog);
    assert_eq!(projection.query().sequence, 1);
    // 旧公开入口沿用同一消费者，避免出现两套投影更新算法。
    assert!(!projection.accept(&event).unwrap());
}
