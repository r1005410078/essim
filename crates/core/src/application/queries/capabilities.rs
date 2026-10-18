//! 当前已装配能力。
use serde::{Deserialize, Serialize};
/// 实际生产能力信息；不根据底层库已编译推断可运行。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    /// 最小配置操作可用。
    pub configuration: bool,
    /// 生产协议执行器是否接入。
    pub execution: bool,
    /// 可靠录制是否接入。
    pub recording: bool,
    /// 可用历史回放是否接入。
    pub replay: bool,
    /// 当前存储是否仅本次会话。
    pub session_only: bool,
    /// 禁用运行操作的具体原因。
    pub reason: String,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            configuration: true,
            execution: false,
            recording: false,
            replay: false,
            session_only: true,
            reason: "尚未接入正式协议执行器和可靠录制；当前只能配置，不能运行或回放".into(),
        }
    }
}
