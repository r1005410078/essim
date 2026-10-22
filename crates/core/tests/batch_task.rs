//! 批量任务的状态守恒与未确认占位；与协议线程和墙钟解耦。
use essim_core::domain::{
    aggregates::batch::{BatchOperation, BatchTask, ItemState, Outcome},
    identity::EndpointId,
};
fn task() -> BatchTask {
    BatchTask::new(
        "batch".into(),
        BatchOperation::Start,
        vec![
            EndpointId("a".into()),
            EndpointId("b".into()),
            EndpointId("c".into()),
        ],
        2,
        10,
    )
    .unwrap()
}
#[test]
fn uncertain_result_keeps_slot_and_cannot_finish_task() {
    let mut t = task();
    assert_eq!(t.claim(20).unwrap().unwrap().0, "a");
    assert_eq!(t.claim(21).unwrap().unwrap().0, "b");
    t.mark_uncertain(&EndpointId("a".into()), 30, "等待超时".into())
        .unwrap();
    assert!(t.claim(31).unwrap().is_none());
    assert_eq!(t.occupied(), 2);
    assert!(!t.is_finished());
    t.complete(&EndpointId("a".into()), 40, Outcome::Succeeded)
        .unwrap();
    assert_eq!(t.claim(41).unwrap().unwrap().0, "c");
    assert_eq!(t.counts().total(), 3);
    assert_eq!(t.items()[0].wait_ms(), Some(10));
    assert_eq!(t.items()[0].execution_ms(99), Some(20));
}
#[test]
fn cancellation_does_not_stop_started_or_rollback_success() {
    let mut t = task();
    t.claim(20).unwrap();
    t.complete(&EndpointId("a".into()), 30, Outcome::Succeeded)
        .unwrap();
    t.claim(31).unwrap();
    t.cancel(32).unwrap();
    assert!(t.claim(33).unwrap().is_none());
    assert_eq!(t.items()[0].state, ItemState::Succeeded);
    assert_eq!(t.items()[1].state, ItemState::Running);
    assert_eq!(t.items()[2].state, ItemState::Cancelled);
    assert!(!t.is_finished());
    t.complete(
        &EndpointId("b".into()),
        40,
        Outcome::Failed("端口占用".into()),
    )
    .unwrap();
    assert!(t.is_finished());
    assert_eq!(t.counts().total(), 3);
}
#[test]
fn invalid_transitions_and_clock_regressions_leave_state_intact() {
    let mut t = task();
    assert!(
        t.complete(&EndpointId("a".into()), 20, Outcome::Succeeded)
            .is_err()
    );
    t.claim(20).unwrap();
    assert!(
        t.complete(&EndpointId("a".into()), 19, Outcome::Succeeded)
            .is_err()
    );
    assert_eq!(t.items()[0].state, ItemState::Running);
    t.complete(
        &EndpointId("a".into()),
        25,
        Outcome::Skipped("未应用".into()),
    )
    .unwrap();
    assert!(
        t.complete(&EndpointId("a".into()), 26, Outcome::Succeeded)
            .is_err()
    );
    assert!(
        t.mark_uncertain(&EndpointId("missing".into()), 26, "无对象".into())
            .is_err()
    );
}
#[test]
fn limits_and_duplicate_identities_are_rejected() {
    for concurrency in [0, 9] {
        assert!(
            BatchTask::new(
                "b".into(),
                BatchOperation::Start,
                vec![EndpointId("a".into())],
                concurrency,
                0
            )
            .is_err()
        );
    }
    assert!(BatchTask::new("b".into(), BatchOperation::Start, vec![], 4, 0).is_err());
    assert!(
        BatchTask::new(
            "b".into(),
            BatchOperation::Start,
            vec![EndpointId("a".into()), EndpointId("a".into())],
            4,
            0
        )
        .is_err()
    );
}
