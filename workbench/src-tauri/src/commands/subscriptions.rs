//! 配置投影订阅、取消与线程回收；完整快照可跨过合并的界面通知。
use super::{
    dto::{ErrorDto, WorkspaceView},
    Engine,
};
use essim_core::domain::error::Error;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use tauri::{ipc::Channel, State as Managed};

/// 一个订阅的停止信号与线程句柄，由 Subscriptions 持有直至取消或回收。
struct Subscription {
    /// 命令侧发出停止信号，线程侧在循环中检查。
    cancel: Arc<AtomicBool>,
    /// 取消时等待线程退出，避免遗留后台推送。
    join: JoinHandle<()>,
}
/// 由 Tauri 管理的订阅集合，负责分配标识并在销毁时停止、回收剩余线程。
#[derive(Default)]
pub struct Subscriptions {
    /// 单调分配订阅 ID；Relaxed 仅用于取号，不承担数据同步。
    next: AtomicU64,
    /// 订阅 ID 到停止信号、线程句柄的对应关系。
    items: Mutex<HashMap<String, Subscription>>,
}
impl Drop for Subscriptions {
    fn drop(&mut self) {
        // 集合销毁时已独占访问；对所有剩余订阅发出停止信号并等待线程结束。
        for (_, s) in self.items.get_mut().unwrap().drain() {
            s.cancel.store(true, Ordering::Release);
            let _ = s.join.join();
        }
    }
}
/// 订阅会话配置目录，通过 IPC Channel 推送完整快照，并返回用于取消的订阅 ID。
/// 每个订阅拥有独立线程；仅配置序号变化时推送最新完整快照，通道发送失败后线程退出。
///
/// # Errors
/// 当前活跃订阅已达到 8 个时返回错误。
///
/// # Panics
/// 订阅集合锁已中毒或系统无法创建订阅线程时可能 panic。
#[tauri::command]
pub fn subscribe_workspace(
    channel: Channel<WorkspaceView>,
    engine: Managed<'_, Engine>,
    subscriptions: Managed<'_, Subscriptions>,
) -> Result<String, ErrorDto> {
    let mut items = subscriptions.items.lock().unwrap();
    // 先清理已退出的线程记录，再计算订阅上限；此时线程已结束，无需等待。
    items.retain(|_, s| !s.join.is_finished());
    if items.len() >= 8 {
        return Err(Error::new("SUBSCRIPTION_LIMIT", "最多允许 8 个订阅，请先关闭旧面板").into());
    }
    let id = subscriptions
        .next
        .fetch_add(1, Ordering::Relaxed)
        .to_string();
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = cancel.clone();
    // 线程持有 Arc 克隆，避免超出本次命令的 State 借用生命周期。
    let engine = engine.inner().clone();
    let join = thread::spawn(move || {
        let mut last = None;
        while !flag.load(Ordering::Acquire) {
            let sequence = match engine.sequence() {
                Ok(sequence) => sequence,
                Err(_) => break,
            };
            if last != Some(sequence) {
                let snapshot = match engine.query() {
                    Ok(snapshot) => snapshot,
                    Err(_) => break,
                };
                let sequence = snapshot.sequence;
                if channel.send(snapshot).is_err() {
                    break;
                }
                last = Some(sequence);
            }
            // M01仅订阅小型配置目录，无高频点值流。最多100ms观察取消。
            thread::sleep(Duration::from_millis(100));
        }
    });
    items.insert(id.clone(), Subscription { cancel, join });
    Ok(id)
}
/// 取消订阅并等待推送线程退出；订阅 ID 不存在时也返回成功。
///
/// # Errors
/// blocking 任务执行失败或订阅线程发生 panic 时返回 UNSUBSCRIBE_FAILED。
///
/// # Panics
/// 订阅集合锁已中毒时可能 panic。
#[tauri::command]
pub async fn unsubscribe_workspace(
    subscription_id: String,
    subscriptions: Managed<'_, Subscriptions>,
) -> Result<(), ErrorDto> {
    // 取出记录后立即释放集合锁，避免持锁等待线程结束或跨越 await。
    let item = subscriptions.items.lock().unwrap().remove(&subscription_id);
    if let Some(s) = item {
        // 线程最多按 轮询的检查间隔观察到取消；join 放入 blocking 线程池。
        s.cancel.store(true, Ordering::Release);
        tauri::async_runtime::spawn_blocking(move || s.join.join())
            .await
            .map_err(|e| ErrorDto::new("UNSUBSCRIBE_FAILED", e.to_string()))?
            .map_err(|_| ErrorDto::new("UNSUBSCRIBE_FAILED", "订阅线程异常退出"))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dropping_manager_cancels_and_joins_every_subscription() {
        let subscriptions = Subscriptions::default();
        let completed = Arc::new(AtomicU64::new(0));
        for id in 0..3 {
            let flag = Arc::new(AtomicBool::new(false));
            let worker_flag = flag.clone();
            let completed = completed.clone();
            let join = thread::spawn(move || {
                while !worker_flag.load(Ordering::Acquire) {
                    thread::sleep(Duration::from_millis(1));
                }
                completed.fetch_add(1, Ordering::Release);
            });
            subscriptions
                .items
                .lock()
                .unwrap()
                .insert(id.to_string(), Subscription { cancel: flag, join });
        }
        drop(subscriptions);
        assert_eq!(completed.load(Ordering::Acquire), 3);
    }
}
