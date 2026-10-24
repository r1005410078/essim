//! 原子资源预留；RAII在真实操作结束时归还，等待超时不能丢弃操作句柄。
use essim_core::domain::{error::Error, resources::ResourceUsage};
use serde::Serialize;
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex, Weak},
};
/// 流式计算序列化字节，准入前不额外分配一份完整JSON。
/// # Errors
/// 序列化失败或长度溢出。
pub(crate) fn serialized_size(value: &impl Serialize) -> Result<u64, Error> {
    struct Counter(u64);
    impl std::io::Write for Counter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0 = self
                .0
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| std::io::Error::other("序列化长度溢出"))?;
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut counter = Counter(0);
    serde_json::to_writer(&mut counter, value)
        .map_err(|e| Error::new("SERIALIZE_FAILED", e.to_string()))?;
    Ok(counter.0)
}
/// 基线后确定的首批工程限制，不是规模承诺。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourcePolicy {
    /// 合计资源上限；内存为保守估计预算，不含全系统内存。
    pub limit: ResourceUsage,
    /// 单次批量目标数。
    pub batch_targets: usize,
    /// 最大预览JSON大小。
    pub preview_bytes: usize,
    /// 单端点录制字节上限。
    pub endpoint_record_bytes: usize,
    /// 单次录制合批字节上限。
    pub record_batch_bytes: usize,
    /// 最大合批等待毫秒。
    pub record_wait_ms: u64,
}
impl Default for ResourcePolicy {
    fn default() -> Self {
        Self {
            limit: ResourceUsage {
                memory_bytes: 2 * 1024 * 1024 * 1024,
                endpoints: 1000,
                connections: 10000,
                queue_bytes: 64 * 1024 * 1024,
            },
            batch_targets: 1024,
            preview_bytes: 8 * 1024 * 1024,
            endpoint_record_bytes: 1024 * 1024,
            record_batch_bytes: 64 * 1024,
            record_wait_ms: 5,
        }
    }
}
#[derive(Default)]
struct Ledger {
    next: u64,
    entries: BTreeMap<u64, (ResourceUsage, bool)>,
    used: ResourceUsage,
}
/// 共享准入入口，预留不依赖前端页面。
#[derive(Clone)]
pub struct ResourceManager {
    inner: Arc<Mutex<Ledger>>,
    policy: ResourcePolicy,
}
impl Default for ResourceManager {
    fn default() -> Self {
        Self {
            inner: Default::default(),
            policy: Default::default(),
        }
    }
}
/// 唯一拥有的额度；不能clone产生重复归还。
pub struct Reservation {
    inner: Weak<Mutex<Ledger>>,
    id: u64,
}
/// 只读资源投影。
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceView {
    /// 配置的工程预算。
    pub policy: ResourcePolicy,
    /// 当前尚未释放的预留。
    pub reserved: ResourceUsage,
    /// 已确认分配的工程估计，不是RSS。
    pub current: ResourceUsage,
    /// 共享录制池的实际积压与同步次数；未装配时为空。
    pub recording: Option<crate::recording::RecordingView>,
    /// 独立 SQLite 运行数据投影，不影响原有可靠录制门禁。
    pub runtime_data: Option<crate::project::RuntimeDataView>,
    /// 采样时间，Unix毫秒。
    pub sampled_ms: String,
    /// 当前进程RSS字节，无法采样时为空。
    pub process_rss_bytes: Option<u64>,
    /// 配置端点数，由工作区查询填充。
    pub configured_endpoints: usize,
    /// 活跃预留数。
    pub reservations: usize,
    /// 对资源覆盖范围的说明。
    pub scope: String,
}
impl ResourceManager {
    /// 注入工程预算，用于独立运行及受控边界验证。
    /// # Errors
    /// 任务或录制队列参数非法。
    pub fn with_policy(policy: ResourcePolicy) -> Result<Self, Error> {
        if policy.batch_targets == 0
            || policy.batch_targets > 1024
            || policy.preview_bytes == 0
            || policy.record_batch_bytes == 0
            || policy.record_batch_bytes > policy.endpoint_record_bytes
            || policy.record_wait_ms == 0
        {
            return Err(Error::new("INVALID_RESOURCE_POLICY", "资源策略非法"));
        }
        Ok(Self {
            inner: Default::default(),
            policy,
        })
    }

    /// 原子预留额度；失败不改变已有占用。
    /// # Errors
    /// 额度耗尽、身份耗尽或内部状态不可用。
    pub fn reserve(&self, usage: ResourceUsage) -> Result<Reservation, Error> {
        let mut ledger = self
            .inner
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "资源账本不可用"))?;
        let next = ledger
            .next
            .checked_add(1)
            .ok_or_else(|| Error::new("RESOURCE_OVERFLOW", "资源令牌耗尽"))?;
        let total = ledger.used.admit(usage, self.policy.limit)?;
        ledger.next = next;
        ledger.used = total;
        ledger.entries.insert(next, (usage, false));
        Ok(Reservation {
            inner: Arc::downgrade(&self.inner),
            id: next,
        })
    }
    /// 查询预留，不复制模型和值。
    /// # Errors
    /// 资源状态不可用。
    pub fn query(&self) -> Result<ResourceView, Error> {
        let ledger = self
            .inner
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "资源账本不可用"))?;
        let mut current = ResourceUsage::default();
        let mut reserved = ResourceUsage::default();
        for (usage, committed) in ledger.entries.values() {
            let sum = if *committed {
                &mut current
            } else {
                &mut reserved
            };
            sum.memory_bytes += usage.memory_bytes;
            sum.endpoints += usage.endpoints;
            sum.connections += usage.connections;
            sum.queue_bytes += usage.queue_bytes;
        }
        Ok(ResourceView {
            current,
            recording:None,
            runtime_data:None,
            sampled_ms: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis().to_string(),
            process_rss_bytes: None,
            configured_endpoints: 0,
            policy: self.policy.clone(),
            reserved,
            reservations: ledger.entries.len(),
            scope:
                "工程估计：已接入的暂存、运行映射及连接槽预留；RSS为当前进程（桌面含Tauri，WebView和独立驱动器不含），不能与估计相加"
                    .into(),
        })
    }
    /// 取得当前工程策略。
    pub fn policy(&self) -> &ResourcePolicy {
        &self.policy
    }
}
impl Reservation {
    /// 真实分配成功后标为当前用量；等待超时不能调用此方法冒充完成。
    pub fn commit(&self) {
        if let Some(inner) = self.inner.upgrade() {
            if let Ok(mut ledger) = inner.lock() {
                if let Some(entry) = ledger.entries.get_mut(&self.id) {
                    entry.1 = true;
                }
            }
        }
    }
}
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.upgrade() {
            if let Ok(mut ledger) = inner.lock() {
                if let Some((u, _)) = ledger.entries.remove(&self.id) {
                    ledger.used.memory_bytes -= u.memory_bytes;
                    ledger.used.endpoints -= u.endpoints;
                    ledger.used.connections -= u.connections;
                    ledger.used.queue_bytes -= u.queue_bytes;
                }
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn concurrent_claims_do_not_overbook_and_drop_releases_once() {
        let manager = ResourceManager::default();
        let mut threads = vec![];
        let barrier = Arc::new(std::sync::Barrier::new(3));
        for _ in 0..2 {
            let m = manager.clone();
            let b = barrier.clone();
            threads.push(std::thread::spawn(move || {
                let lease = m.reserve(ResourceUsage {
                    memory_bytes: 2 * 1024 * 1024 * 1024,
                    ..Default::default()
                });
                b.wait();
                lease
            }));
        }
        barrier.wait();
        let leases = threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(leases.iter().filter(|r| r.is_ok()).count(), 1);
        assert_eq!(manager.query().unwrap().reservations, 1);
        drop(leases);
        assert_eq!(manager.query().unwrap().reserved, ResourceUsage::default());
    }
}

/// 估算目录实际保留内容：模型内容按共享指针去重，Unit值分别计费。
/// 系数4覆盖字符串／树节点及权威目录、事件、投影副本；不是RSS测量。
/// # Errors
/// 序列化计数失败。
pub fn catalog_memory(catalog: &essim_core::application::catalog::Catalog) -> Result<u64, Error> {
    let mut bytes = serialized_size(&catalog.endpoints)?
        .saturating_add(serialized_size(&catalog.applied)?)
        .saturating_add(serialized_size(&catalog.navigation)?)
        .saturating_add(serialized_size(&catalog.devices)?);
    for model in &catalog.models {
        bytes = bytes.saturating_add(serialized_size(model)?);
    }
    let mut seen = std::collections::BTreeSet::new();
    for values in catalog.values.values() {
        bytes = bytes.saturating_add(serialized_size(&values.raw)?);
        if let Some(model) = &values.model {
            if seen.insert(Arc::as_ptr(model) as usize) {
                bytes = bytes.saturating_add(serialized_size(model)?);
            }
        }
    }
    Ok(bytes.saturating_mul(4))
}

#[cfg(test)]
mod lifecycle_tests {
    use super::*;
    #[test]
    fn committed_and_inflight_charges_are_distinct_and_failure_rolls_back() {
        let policy = ResourcePolicy {
            limit: ResourceUsage {
                memory_bytes: 10,
                ..Default::default()
            },
            ..Default::default()
        };
        let m = ResourceManager::with_policy(policy).unwrap();
        let active = m
            .reserve(ResourceUsage {
                memory_bytes: 6,
                ..Default::default()
            })
            .unwrap();
        active.commit();
        let pending = m
            .reserve(ResourceUsage {
                memory_bytes: 4,
                ..Default::default()
            })
            .unwrap();
        let q = m.query().unwrap();
        assert_eq!(q.current.memory_bytes, 6);
        assert_eq!(q.reserved.memory_bytes, 4);
        assert!(m
            .reserve(ResourceUsage {
                memory_bytes: 1,
                ..Default::default()
            })
            .is_err());
        // 调用观察超时不会销毁此令牌，额度仍满；只有真实回调归还后可申请。
        drop(pending);
        assert_eq!(m.query().unwrap().reserved.memory_bytes, 0);
        drop(active);
        assert_eq!(m.query().unwrap().current.memory_bytes, 0);
    }
}

/// 按 Unit 分别预留有效点定义与值迁移空间，不假定同 ModelId 内容相同。
/// # Errors
/// 端点／模型缺失或计数失败。
pub(crate) fn effective_staging_memory(
    catalog: &essim_core::application::catalog::Catalog,
    id: &essim_core::domain::identity::EndpointId,
) -> Result<u64, Error> {
    let endpoint = catalog
        .endpoints
        .iter()
        .find(|e| &e.id == id)
        .ok_or_else(|| Error::new("NOT_FOUND", "端点不存在"))?;
    let mut bytes = 0u64;
    for unit in &endpoint.units {
        let model = catalog
            .models
            .iter()
            .find(|m| m.id == unit.model.id)
            .ok_or_else(|| Error::new("MODEL_NOT_FOUND", "模型不存在"))?;
        bytes = bytes
            .saturating_add(serialized_size(model)?)
            .saturating_add(serialized_size(&unit.overrides)?);
    }
    Ok(bytes.saturating_mul(8))
}
