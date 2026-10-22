//! 服务→命令→聚合→事实→监听投影→查询的批量任务最小闭环。
use essim_core::{
    application::{
        batch_service::BatchService, commands::batch::BatchCommand,
        event_handlers::batch_projection::BatchProjectionListener,
    },
    domain::{
        aggregates::batch::{BatchOperation, Outcome},
        identity::EndpointId,
    },
};
#[test]
fn command_fact_listener_query_preserve_unconfirmed_occupancy() {
    let mut service = BatchService::default();
    let started = service
        .execute(
            BatchCommand::Start {
                operation: BatchOperation::Start,
                id: "b".into(),
                targets: vec![EndpointId("e".into())],
                concurrency: 1,
            },
            0,
        )
        .unwrap()
        .unwrap();
    let mut listener = BatchProjectionListener::default();
    listener.handle(&started).unwrap();
    assert_eq!(listener.query().unwrap().counts.pending, 1);
    let claimed = service
        .execute(BatchCommand::Claim { id: "b".into() }, 10)
        .unwrap()
        .unwrap();
    listener.handle(&claimed).unwrap();
    let uncertain = service
        .execute(
            BatchCommand::MarkUncertain {
                id: "b".into(),
                endpoint: EndpointId("e".into()),
                reason: "等待超时".into(),
            },
            20,
        )
        .unwrap()
        .unwrap();
    listener.handle(&uncertain).unwrap();
    assert_eq!(listener.query().unwrap().occupied, 1);
    assert!(!listener.query().unwrap().finished);
    assert!(
        service
            .execute(
                BatchCommand::Start {
                    operation: BatchOperation::Start,
                    id: "new".into(),
                    targets: vec![EndpointId("e".into())],
                    concurrency: 1
                },
                21
            )
            .is_err()
    );
    assert!(
        service
            .execute(BatchCommand::Claim { id: "b".into() }, 22)
            .unwrap()
            .is_none()
    );
    let complete = service
        .execute(
            BatchCommand::Complete {
                id: "b".into(),
                endpoint: EndpointId("e".into()),
                outcome: Outcome::Succeeded,
            },
            30,
        )
        .unwrap()
        .unwrap();
    listener.handle(&complete).unwrap();
    assert!(listener.query().unwrap().finished);
    assert_eq!(listener.query().unwrap().items[0].execution_ms, Some(20));
    assert!(listener.handle(&uncertain).is_err());
    assert!(listener.query().unwrap().finished);
}
