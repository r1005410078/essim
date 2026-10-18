//! 资源释放窄适配边界；不绑定socket、不打开串口，也不提供生产执行器。
use super::ports::{ApplyBoundary, ApplyState};
use crate::domain::{endpoint::EndpointConfig, error::Error, identity::EndpointId};
/// 由资源所有者实现停止请求和有界等待；请求成功不等于资源释放。
pub trait ReleaseDriver {
    /// 查询资源所有者当前确认的状态。
    fn state(&self, id: &EndpointId) -> ApplyState;
    /// 请求完整端点／串口通道停止，须幂等。
    /// # Errors
    /// 请求无法送达资源所有者时失败。
    fn request_stop(&mut self, endpoint: &EndpointConfig) -> Result<(), Error>;
    /// 有界等待资源释放；成功必须表示句柄、连接与回调全部释放。
    /// # Errors
    /// 超过timeout_ms或释放失败；不得提前返回成功。
    fn wait_released(&mut self, id: &EndpointId, timeout_ms: u64) -> Result<(), Error>;
}
/// 将请求及释放确认适配为配置应用需要的整组边界。
pub struct ConfirmedBoundary<D: ReleaseDriver> {
    driver: D,
    timeout_ms: u64,
}
impl<D: ReleaseDriver> ConfirmedBoundary<D> {
    /// 装配资源所有者及等待上限（毫秒），不自行取得协议资源。
    pub fn new(driver: D, timeout_ms: u64) -> Self {
        Self { driver, timeout_ms }
    }
}
impl<D: ReleaseDriver> ApplyBoundary for ConfirmedBoundary<D> {
    fn state(&self, id: &EndpointId) -> ApplyState {
        self.driver.state(id)
    }
    fn stop(&mut self, endpoint: &EndpointConfig) -> Result<(), Error> {
        if self.driver.state(&endpoint.id) == ApplyState::Stopped {
            return Ok(());
        }
        self.driver.request_stop(endpoint)?;
        self.driver.wait_released(&endpoint.id, self.timeout_ms)?;
        if self.driver.state(&endpoint.id) != ApplyState::Stopped {
            return Err(Error::new("RELEASE_UNCONFIRMED", "资源所有者尚未确认释放"));
        }
        Ok(())
    }
}
