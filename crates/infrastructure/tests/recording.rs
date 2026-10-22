//! 注入式可靠屏障测试；不通过睡眠假定磁盘已完成。
use essim_infrastructure::{
    recording::{DurableSink, RecordingPool},
    resources::{ResourceManager, ResourcePolicy},
};
use std::{
    io::{self, Write},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    },
    time::{Duration, Instant},
};
#[derive(Default)]
struct Control {
    bytes: Mutex<Vec<u8>>,
    released: Mutex<bool>,
    wake: Condvar,
    syncs: AtomicUsize,
    fail: bool,
}
struct Sink(Arc<Control>);
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.bytes.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl DurableSink for Sink {
    fn sync_data(&mut self) -> io::Result<()> {
        self.0.syncs.fetch_add(1, Ordering::SeqCst);
        let mut r = self.0.released.lock().unwrap();
        while !*r {
            r = self.0.wake.wait(r).unwrap();
        }
        if self.0.fail {
            Err(io::Error::other("注入同步失败"))
        } else {
            Ok(())
        }
    }
}
fn release(c: &Control) {
    *c.released.lock().unwrap() = true;
    c.wake.notify_all();
}
struct Cleanup(Arc<Control>);
impl Drop for Cleanup {
    fn drop(&mut self) {
        release(&self.0)
    }
}
fn wait(f: impl Fn() -> bool) {
    let until = Instant::now() + Duration::from_secs(3);
    while !f() {
        assert!(Instant::now() < until);
        std::thread::yield_now();
    }
}
#[test]
fn sync_barrier_keeps_prefix_and_query_free_while_other_file_progresses() {
    let resources = ResourceManager::default();
    let pool = RecordingPool::new(resources.clone()).unwrap();
    let slow = Arc::new(Control::default());
    let _cleanup = Cleanup(slow.clone());
    let a = pool.stream(Box::new(Sink(slow.clone()))).unwrap();
    let pending = a.append(b"first\n".to_vec(), 1).unwrap();
    wait(|| slow.syncs.load(Ordering::SeqCst) == 1);
    assert!(pending
        .wait_timeout(Duration::from_millis(1))
        .unwrap()
        .is_none());
    assert_eq!(a.reliable().unwrap(), 0);
    assert_eq!(pool.query().unwrap().queued_bytes, 6);
    let healthy = Arc::new(Control::default());
    release(&healthy);
    let b = pool.stream(Box::new(Sink(healthy))).unwrap();
    let done = b.append(b"other\n".to_vec(), 1).unwrap();
    assert!(done
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(a.reliable().unwrap(), 0);
    release(&slow);
    assert!(pending
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .is_ok());
    assert_eq!(a.reliable().unwrap(), 1);
    wait(|| resources.query().unwrap().current.queue_bytes == 0);
}
#[test]
fn same_file_batches_keep_order_and_sync_failure_never_advances_prefix() {
    let pool = RecordingPool::new(ResourceManager::default()).unwrap();
    let c = Arc::new(Control {
        fail: true,
        ..Default::default()
    });
    let _cleanup = Cleanup(c.clone());
    let stream = pool.stream(Box::new(Sink(c.clone()))).unwrap();
    let a = stream.append(b"1\n".to_vec(), 1).unwrap();
    let b = stream.append(b"2\n".to_vec(), 2).unwrap();
    assert!(stream.append(b"gap\n".to_vec(), 4).is_err());
    assert_eq!(stream.reliable().unwrap(), 0);
    release(&c);
    assert!(a
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .is_err());
    assert!(b
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .is_err());
    assert_eq!(stream.reliable().unwrap(), 0);
    assert!(stream.append(b"3\n".to_vec(), 3).is_err());
}
#[test]
fn queue_limits_include_inflight_bytes_and_do_not_block_stop_or_query() {
    let manager = ResourceManager::with_policy(ResourcePolicy {
        endpoint_record_bytes: 8,
        record_batch_bytes: 8,
        record_wait_ms: 1,
        ..Default::default()
    })
    .unwrap();
    let pool = RecordingPool::new(manager.clone()).unwrap();
    let c = Arc::new(Control::default());
    let _cleanup = Cleanup(c.clone());
    let s = pool.stream(Box::new(Sink(c.clone()))).unwrap();
    let a = s.append(vec![1; 8], 1).unwrap();
    wait(|| c.syncs.load(Ordering::SeqCst) > 0);
    assert_eq!(
        s.append(vec![2], 2).err().unwrap().code,
        "RECORD_QUEUE_FULL"
    );
    assert_eq!(pool.query().unwrap().queued_bytes, 8);
    drop(s);
    release(&c);
    assert!(a
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .is_ok());
    wait(|| pool.query().unwrap().files == 0);
}
#[test]
fn cannot_report_later_causal_stage_before_its_own_sync() {
    let pool = RecordingPool::new(ResourceManager::default()).unwrap();
    let c = Arc::new(Control::default());
    let _cleanup = Cleanup(c.clone());
    let s = pool.stream(Box::new(Sink(c.clone()))).unwrap();
    let intent = s.append(b"intent\n".to_vec(), 1).unwrap();
    wait(|| c.syncs.load(Ordering::SeqCst) == 1);
    assert!(intent.wait_timeout(Duration::ZERO).unwrap().is_none());
    assert_eq!(s.reliable().unwrap(), 0);
    release(&c);
    intent
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .unwrap();
    // 动作只在意图可靠确认之后发生，再提交结果；不能提前将两阶段伪合批。
    *c.released.lock().unwrap() = false;
    let result = s.append(b"result\n".to_vec(), 2).unwrap();
    wait(|| c.syncs.load(Ordering::SeqCst) == 2);
    assert_eq!(s.reliable().unwrap(), 1);
    assert!(result.wait_timeout(Duration::ZERO).unwrap().is_none());
    release(&c);
    result
        .wait_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(s.reliable().unwrap(), 2);
}

#[test]
fn common_disk_failure_is_reported_per_file_without_advancing_any_prefix() {
    let resources = ResourceManager::default();
    let pool = RecordingPool::new(resources.clone()).unwrap();
    let mut streams = vec![];
    let mut jobs = vec![];
    for _ in 0..2 {
        let c = Arc::new(Control {
            fail: true,
            ..Default::default()
        });
        release(&c);
        let stream = pool.stream(Box::new(Sink(c))).unwrap();
        jobs.push(stream.append(b"fact\n".to_vec(), 1).unwrap());
        streams.push(stream);
    }
    for job in jobs {
        assert!(job
            .wait_timeout(Duration::from_secs(2))
            .unwrap()
            .unwrap()
            .is_err());
    }
    for stream in &streams {
        assert_eq!(stream.reliable().unwrap(), 0);
    }
    assert_eq!(pool.query().unwrap().failed_files, 2);
    wait(|| resources.query().unwrap().current.queue_bytes == 0);
}
