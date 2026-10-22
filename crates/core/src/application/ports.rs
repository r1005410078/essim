//! 应用所需的仓储与资源释放窄端口。
use super::catalog::Catalog;
use crate::domain::{endpoint::EndpointConfig, error::Error, identity::EndpointId};
use serde::{Deserialize, Serialize};
/// 配置存储窄端口；提交必须原子成功或保持旧值。
pub trait ConfigRepository {
    /// 在复制事务目录前预留暂存；缺省实现不计费，基础设施可注入真实准入。
    /// # Errors
    /// 暂存预算不足时拒绝，尚未分配目录副本。
    fn reserve_staging(&self) -> Result<Box<dyn Send>,Error> {Ok(Box::new(()))}

    /// 获取权威目录副本。
    fn load(&self) -> Catalog;
    /// 原子提交完整目录；不得在此持有协议资源。
    /// # Errors
    /// 存储不可用时失败，不能半写入。
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error>;
}
/// 配置应用时的整组停机边界；一个TCP监听端点或RTU串口通道是最小范围。
pub trait ApplyBoundary {
    /// 当前资源状态。缺省无法确认；无资源装配显式返回停止。
    fn state(&self, _id: &EndpointId) -> ApplyState {
        ApplyState::Failed
    }
    /// 确认整个端点停止，不能只停一个Unit。成功后不自动启动。
    /// # Errors
    /// 未确认停止时失败，调用者不得提交新映射。
    fn stop(&mut self, endpoint: &EndpointConfig) -> Result<(), Error>;
}
/// 无生产执行器时所有端点本来就未运行；不声明获得真实停机证据。
#[derive(Default)]
pub struct StoppedBoundary;
impl ApplyBoundary for StoppedBoundary {
    fn state(&self, _: &EndpointId) -> ApplyState {
        ApplyState::Stopped
    }
    fn stop(&mut self, _: &EndpointConfig) -> Result<(), Error> {
        Ok(())
    }
}
/// 停机确认状态与配置提交结果分开，后续提交失败不恢复成运行中。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ApplyState {
    /// 执行器仍持有资源。
    Running,
    /// 已发出停止请求，尚未确认释放。
    Releasing,
    /// 资源状态失败或不能确认。
    Failed,
    /// 整个端点已经由边界确认停止。
    Stopped,
}
