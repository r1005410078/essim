//! 固定执行线程与受控完成信号；不是协议互操作或性能测试。
use essim_core::{
    application::{
        catalog::Catalog,
        commands::batch_targets::{prepare_batch, BatchPlan, BatchScope},
    },
    domain::{
        aggregates::batch::{BatchOperation, Outcome},
        identity::EndpointId,
    },
};
use essim_infrastructure::batch_runner::{BatchExecutor, BatchRunner, WaitLimits};
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
fn limits(d: Duration) -> WaitLimits {
    WaitLimits {
        apply: d,
        start: d,
        stop: d,
    }
}
fn plan(id: &str, n: usize, concurrency: usize) -> BatchPlan {
    let mut c = Catalog::default();
    for i in 0..n {
        c.endpoints.push(serde_json::from_value(serde_json::json!({"id":format!("e{i}"),"name":format!("E{i}"),"role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":1080},"units":[]})).unwrap());
    }
    prepare_batch(
        &c,
        id.into(),
        BatchOperation::Stop,
        BatchScope::All {},
        concurrency,
    )
    .unwrap()
}
#[derive(Default)]
struct Controlled {
    released: Mutex<bool>,
    release: Condvar,
    started: AtomicUsize,
    active: AtomicUsize,
    peak: AtomicUsize,
    threads: Mutex<HashSet<std::thread::ThreadId>>,
}
impl Controlled {
    fn release(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
}
impl BatchExecutor for Controlled {
    fn execute(
        &self,
        _: &BatchPlan,
        id: &EndpointId,
        _permit: &essim_infrastructure::operation_gates::OperationPermit,
    ) -> Outcome {
        self.threads
            .lock()
            .unwrap()
            .insert(std::thread::current().id());
        self.started.fetch_add(1, Ordering::SeqCst);
        let now = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(now, Ordering::SeqCst);
        let mut released = self.released.lock().unwrap();
        while !*released {
            released = self.release.wait(released).unwrap();
        }
        self.active.fetch_sub(1, Ordering::SeqCst);
        if id.0 == "e1" {
            Outcome::Failed("注入失败".into())
        } else {
            Outcome::Succeeded
        }
    }
}
struct ReleaseOnDrop(Arc<Controlled>);
impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        self.0.release();
    }
}
fn wait_until(f: impl Fn() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < deadline, "等待受控状态超时");
        std::thread::yield_now();
    }
}
#[test]
fn unconfirmed_occupies_slots_rejects_second_task_and_cancel_preserves_started() {
    let executor = Arc::new(Controlled::default());
    let runner = BatchRunner::new(
        executor.clone(),
        limits(Duration::from_millis(5)),
        1024,
        8 * 1024 * 1024,
    )
    .unwrap();
    let _release = ReleaseOnDrop(executor.clone());
    runner.submit(plan("b", 12, 2)).unwrap();
    wait_until(|| executor.started.load(Ordering::SeqCst) == 2);
    wait_until(|| runner.query().unwrap().unwrap().counts.uncertain == 2);
    let q = runner.query().unwrap().unwrap();
    assert_eq!(q.occupied, 2);
    assert_eq!(q.counts.pending, 10);
    assert!(runner.submit(plan("other", 1, 1)).is_err());
    runner.cancel("b").unwrap();
    assert_eq!(runner.query().unwrap().unwrap().counts.cancelled, 10);
    assert_eq!(executor.started.load(Ordering::SeqCst), 2);
    executor.release();
    wait_until(|| runner.query().unwrap().unwrap().finished);
    assert_eq!(executor.peak.load(Ordering::SeqCst), 2);
    let q = runner.query().unwrap().unwrap();
    assert_eq!(q.counts.succeeded + q.counts.failed, 2);
    assert_eq!(q.counts.total(), 12);
}
#[test]
fn many_targets_use_fixed_workers_and_failure_does_not_stop_following_items() {
    let executor = Arc::new(Controlled::default());
    executor.release();
    let runner = BatchRunner::new(
        executor.clone(),
        limits(Duration::from_secs(10)),
        1024,
        8 * 1024 * 1024,
    )
    .unwrap();
    assert_eq!(runner.worker_count(), 8);
    runner.submit(plan("b", 100, 4)).unwrap();
    wait_until(|| runner.query().unwrap().unwrap().finished);
    assert!(executor.peak.load(Ordering::SeqCst) <= 4);
    assert!(executor.threads.lock().unwrap().len() <= 8);
    let q = runner.query().unwrap().unwrap();
    assert_eq!(q.counts.failed, 1);
    assert_eq!(q.counts.succeeded, 99);
    runner.submit(plan("next", 1, 1)).unwrap();
    wait_until(|| runner.query().unwrap().unwrap().finished);
    assert_eq!(runner.worker_count(), 8);
}
#[test]
fn manifest_count_and_bytes_refuse_before_execution() {
    let executor = Arc::new(Controlled::default());
    executor.release();
    let runner =
        BatchRunner::new(executor.clone(), limits(Duration::from_secs(1)), 2, 128).unwrap();
    assert!(runner.submit(plan("count", 3, 1)).is_err());
    assert!(runner.submit(plan("bytes", 2, 1)).is_err());
    assert_eq!(executor.started.load(Ordering::SeqCst), 0);
    assert!(runner.query().unwrap().is_none());
}

#[test]
fn eight_slots_are_a_hard_limit_and_shutdown_waits_for_real_completion() {
    let executor = Arc::new(Controlled::default());
    let runner = BatchRunner::new(
        executor.clone(),
        limits(Duration::from_secs(10)),
        1024,
        8 * 1024 * 1024,
    )
    .unwrap();
    let release_guard = ReleaseOnDrop(executor.clone());
    runner.submit(plan("eight", 9, 8)).unwrap();
    wait_until(|| executor.started.load(Ordering::SeqCst) == 8);
    assert_eq!(runner.query().unwrap().unwrap().occupied, 8);
    assert_eq!(executor.peak.load(Ordering::SeqCst), 8);
    runner.request_shutdown().unwrap();
    assert_eq!(runner.query().unwrap().unwrap().counts.cancelled, 1);
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let shutdown = std::thread::spawn(move || {
        drop(runner);
        done_tx.send(()).unwrap();
    });
    assert!(done_rx.recv_timeout(Duration::from_millis(5)).is_err());
    executor.release();
    done_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    shutdown.join().unwrap();
    drop(release_guard);
    assert_eq!(executor.started.load(Ordering::SeqCst), 8);
}
#[test]
fn adapter_panic_keeps_uncertain_slot_and_does_not_create_replacement_thread() {
    struct PanicExecutor;
    impl BatchExecutor for PanicExecutor {
        fn execute(
            &self,
            _: &BatchPlan,
            _: &EndpointId,
            _permit: &essim_infrastructure::operation_gates::OperationPermit,
        ) -> Outcome {
            panic!("受控适配器故障")
        }
    }
    let runner = BatchRunner::new(
        Arc::new(PanicExecutor),
        limits(Duration::from_secs(10)),
        1024,
        8 * 1024 * 1024,
    )
    .unwrap();
    runner.submit(plan("panic", 2, 1)).unwrap();
    wait_until(|| runner.query().unwrap().unwrap().counts.uncertain == 1);
    let view = runner.query().unwrap().unwrap();
    assert_eq!(view.occupied, 1);
    assert_eq!(view.counts.pending, 1);
    assert_eq!(runner.worker_count(), 8);
    assert!(runner.submit(plan("new", 1, 1)).is_err());
    runner.cancel("panic").unwrap();
    assert!(!runner.query().unwrap().unwrap().finished);
}

#[test]
fn concurrent_submissions_cannot_create_two_active_tasks() {
    let executor = Arc::new(Controlled::default());
    let runner = Arc::new(
        BatchRunner::new(
            executor.clone(),
            limits(Duration::from_secs(10)),
            1024,
            8 * 1024 * 1024,
        )
        .unwrap(),
    );
    let _release = ReleaseOnDrop(executor.clone());
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let mut threads = vec![];
    for id in ["one", "two"] {
        let r = runner.clone();
        let b = barrier.clone();
        threads.push(std::thread::spawn(move || {
            let p = plan(id, 2, 1);
            b.wait();
            r.submit(p)
        }));
    }
    barrier.wait();
    let results = threads
        .into_iter()
        .map(|t| t.join().unwrap())
        .collect::<Vec<_>>();
    assert_eq!(results.iter().filter(|r| r.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|r| r.as_ref().err())
            .next()
            .unwrap()
            .code,
        "BATCH_BUSY"
    );
    executor.release();
    wait_until(|| runner.query().unwrap().unwrap().finished);
}

#[test]
fn cancellation_racing_completion_preserves_success_and_never_claims_after_cancel_returns() {
    let executor = Arc::new(Controlled::default());
    let runner = Arc::new(
        BatchRunner::new(
            executor.clone(),
            limits(Duration::from_secs(10)),
            1024,
            8 * 1024 * 1024,
        )
        .unwrap(),
    );
    let _release = ReleaseOnDrop(executor.clone());
    runner.submit(plan("race", 30, 1)).unwrap();
    wait_until(|| executor.started.load(Ordering::SeqCst) == 1);
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let r = runner.clone();
    let b = barrier.clone();
    let cancel = std::thread::spawn(move || {
        b.wait();
        r.cancel("race").unwrap();
    });
    let e = executor.clone();
    let b = barrier.clone();
    let finish = std::thread::spawn(move || {
        b.wait();
        e.release();
    });
    barrier.wait();
    cancel.join().unwrap();
    finish.join().unwrap();
    let after_cancel = runner.query().unwrap().unwrap();
    assert_eq!(after_cancel.counts.pending, 0);
    wait_until(|| runner.query().unwrap().unwrap().finished);
    let q = runner.query().unwrap().unwrap();
    assert_eq!(q.counts.total(), 30);
    assert!(q.counts.succeeded >= 1);
    assert_eq!(q.counts.cancelled, after_cancel.counts.cancelled);
    assert_eq!(
        q.counts.succeeded + q.counts.failed,
        executor.started.load(Ordering::SeqCst)
    );
    // 重复取消不能将已成功项改为取消，也不能影响下一个任务。
    runner.cancel("race").unwrap();
    assert_eq!(
        runner.query().unwrap().unwrap().counts.succeeded,
        q.counts.succeeded
    );
    runner.submit(plan("next", 1, 1)).unwrap();
    assert!(runner.cancel("race").is_err());
    wait_until(|| runner.query().unwrap().unwrap().finished);
    assert_eq!(runner.query().unwrap().unwrap().counts.succeeded, 1);
}

#[test]
fn lost_completion_cannot_release_gate_or_be_retried_as_failure() {
    struct Lost;
    impl BatchExecutor for Lost {
        fn execute(
            &self,
            _: &BatchPlan,
            _: &EndpointId,
            _: &essim_infrastructure::operation_gates::OperationPermit,
        ) -> Outcome {
            Outcome::Unconfirmed("完成通道丢失".into())
        }
    }
    let gates = essim_infrastructure::operation_gates::OperationGates::default();
    let runner = BatchRunner::with_operations(
        Arc::new(Lost),
        limits(Duration::from_secs(1)),
        1024,
        8 * 1024 * 1024,
        gates.clone(),
    )
    .unwrap();
    runner.submit(plan("lost", 2, 1)).unwrap();
    wait_until(|| runner.query().unwrap().unwrap().counts.uncertain == 1);
    assert!(gates.acquire(&EndpointId("e0".into())).is_err());
    assert!(runner.submit(plan("retry", 1, 1)).is_err());
    runner.cancel("lost").unwrap();
    assert_eq!(runner.query().unwrap().unwrap().counts.cancelled, 1);
}
