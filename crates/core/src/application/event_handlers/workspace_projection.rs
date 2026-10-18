//! 成功提交后的同步查询投影消费者。
use crate::application::{events::ConfigEvent, queries::Projection};
use crate::domain::error::Error;
/// 消费配置事件并更新查询快照；消费整个信封，不按具体fact路由。
pub struct WorkspaceProjectionListener;
impl WorkspaceProjectionListener {
    /// 应用有序事件，重复事件幂等忽略。
    /// # Errors
    /// 未知schema或序号缺口时拒绝更新，调用方需要完整快照恢复。
    pub fn handle(projection: &mut Projection, event: &ConfigEvent) -> Result<bool, Error> {
        if event.schema != 1 {
            return Err(Error::new(
                "UNKNOWN_SCHEMA",
                "未知事件版本，请重新获取完整快照",
            ));
        }
        if event.sequence <= projection.sequence {
            return Ok(false);
        }
        if event.sequence != projection.sequence + 1 {
            return Err(Error::new(
                "SEQUENCE_GAP",
                "配置通知缺口，请重新获取完整快照",
            ));
        }
        projection.sequence = event.sequence;
        projection.catalog = event.catalog.clone();
        Ok(true)
    }
}
