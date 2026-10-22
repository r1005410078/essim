//! 跨调用持有的端点操作门；回调仍持有子凭证时，调用等待超时不能释放门。
use essim_core::domain::{error::Error, identity::EndpointId};
use std::{
    collections::BTreeMap,
    sync::{Arc, Condvar, Mutex},
};
#[derive(Default)]
struct State {
    next: u64,
    held: BTreeMap<EndpointId, (u64, usize)>,
}
#[derive(Default)]
struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}
/// 会话所有者共享的非阻塞端点准入门；查询不申请门。
#[derive(Clone, Default)]
pub struct OperationGates {
    shared: Arc<Shared>,
}
/// 只能由准入门创建的能力凭证，最后一个回调凭证释放后归还端点。
pub struct OperationPermit {
    shared: Arc<Shared>,
    endpoint: EndpointId,
    token: u64,
}
fn poisoned() -> Error {
    Error::new("STATE_POISONED", "端点操作门不可用")
}
impl OperationGates {
    /// 申请一个端点；已有操作时立即拒绝，不在前端调用线程等待。
    /// # Errors
    /// 端点已有操作、令牌溢出或锁状态不可用。
    pub fn acquire(&self, id: &EndpointId) -> Result<OperationPermit, Error> {
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        if state.held.contains_key(id) {
            return Err(Error::new(
                "ENDPOINT_BUSY",
                "端点操作尚未真实结束，禁止重复提交；仍可查询状态",
            ));
        }
        let token = state
            .next
            .checked_add(1)
            .ok_or_else(|| Error::new("OPERATION_OVERFLOW", "操作令牌耗尽"))?;
        state.next = token;
        state.held.insert(id.clone(), (token, 1));
        Ok(OperationPermit {
            shared: self.shared.clone(),
            endpoint: id.clone(),
            token,
        })
    }
    /// 只检查是否允许其他操作；不占门或复制配置。
    /// # Errors
    /// 目标端点仍被占用或状态不可用。
    pub fn ensure_available(&self, id: &EndpointId) -> Result<(), Error> {
        if self
            .shared
            .state
            .lock()
            .map_err(|_| poisoned())?
            .held
            .contains_key(id)
        {
            Err(Error::new(
                "ENDPOINT_BUSY",
                "端点操作尚未真实结束，禁止重复提交；仍可查询状态",
            ))
        } else {
            Ok(())
        }
    }
    pub(crate) fn enter(
        &self,
        id: &EndpointId,
        parent: Option<&OperationPermit>,
    ) -> Result<OperationPermit, Error> {
        if let Some(parent) = parent {
            parent.retain(self, id)
        } else {
            self.acquire(id)
        }
    }
}
impl OperationPermit {
    #[cfg(feature="tcp-runtime")]
    pub(crate) fn retain_callback(&self) -> Result<Self, Error> {
        self.retain(
            &OperationGates {
                shared: self.shared.clone(),
            },
            &self.endpoint,
        )
    }
    fn retain(&self, gates: &OperationGates, id: &EndpointId) -> Result<Self, Error> {
        if !Arc::ptr_eq(&self.shared, &gates.shared) || &self.endpoint != id {
            return Err(Error::new(
                "OPERATION_PERMIT_MISMATCH",
                "操作凭证不属于当前会话或端点",
            ));
        }
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        let entry = state
            .held
            .get_mut(id)
            .filter(|e| e.0 == self.token)
            .ok_or_else(|| Error::new("OPERATION_PERMIT_EXPIRED", "操作凭证已失效"))?;
        entry.1 = entry
            .1
            .checked_add(1)
            .ok_or_else(|| Error::new("OPERATION_OVERFLOW", "操作持有数溢出"))?;
        Ok(Self {
            shared: self.shared.clone(),
            endpoint: id.clone(),
            token: self.token,
        })
    }
    /// 等待本次已提交回调归还其子凭证；不持有配置锁，不创建等待线程。
    /// # Errors
    /// 操作门状态不可用。
    pub fn wait_for_callbacks(&self) -> Result<(), Error> {
        let mut state = self.shared.state.lock().map_err(|_| poisoned())?;
        while state
            .held
            .get(&self.endpoint)
            .is_some_and(|v| v.0 == self.token && v.1 > 1)
        {
            state = self.shared.changed.wait(state).map_err(|_| poisoned())?;
        }
        Ok(())
    }
}
impl Drop for OperationPermit {
    fn drop(&mut self) {
        if let Ok(mut state) = self.shared.state.lock() {
            if let Some(entry) = state.held.get_mut(&self.endpoint) {
                if entry.0 == self.token {
                    entry.1 -= 1;
                    if entry.1 == 0 {
                        state.held.remove(&self.endpoint);
                    }
                }
            }
            self.shared.changed.notify_all();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_keeps_gate_after_caller_leaves_and_other_endpoint_queries_are_free() {
        let gates = OperationGates::default();
        let id = EndpointId("a".into());
        let parent = gates.acquire(&id).unwrap();
        let callback = gates.enter(&id, Some(&parent)).unwrap();
        drop(parent);
        assert_eq!(gates.acquire(&id).err().unwrap().code, "ENDPOINT_BUSY");
        assert!(gates.ensure_available(&EndpointId("b".into())).is_ok());
        drop(callback);
        assert!(gates.acquire(&id).is_ok());
    }
    #[test]
    fn permit_cannot_cross_endpoint_or_session() {
        let gates = OperationGates::default();
        let id = EndpointId("a".into());
        let p = gates.acquire(&id).unwrap();
        assert!(gates.enter(&EndpointId("b".into()), Some(&p)).is_err());
        assert!(OperationGates::default().enter(&id, Some(&p)).is_err());
    }
}
