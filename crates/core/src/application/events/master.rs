//! 主站事实；不以快照通知代替协议事件。
use serde::{Deserialize, Serialize};
/// 主站真实生命周期、通信及操作摘要。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MasterFact {
    /// 实际RTU帧或短帧；隔离中的字节不能当作本次成功响应。
    RtuFrame {
        /// 发送／接收方向。
        direction: String,
        /// 实际十六进制字节。
        hex: String,
        /// 是否取得完整帧。
        complete: bool,
        /// CRC是否正确。
        #[serde(rename = "crcValid")]
        crc_valid: bool,
        /// 是否在迟到隔离窗口中丢弃。
        discarded: bool,
    },
    /// 手动运行意图；先可靠记录，再执行网络操作。
    Intent {
        /// 完整命令和稳定因果身份。
        command: crate::application::commands::master::MasterCommand,
    },
    /// 真实状态变化。
    State {
        /// 状态描述。
        state: String,
    },
    /// 实际完整字节，TX为主站请求、RX为远端响应。
    Frame {
        /// 实际方向。
        direction: String,
        /// 十六进制字节。
        hex: String,
    },
    /// 请求或连接失败。
    Failure {
        /// 具体错误。
        message: String,
    },
    /// 手动操作结果。
    Operation {
        /// 操作身份。
        command: String,
        /// 处理结果摘要。
        result: String,
    },
}
