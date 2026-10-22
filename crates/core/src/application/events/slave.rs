//! TCP从站实际运行事实；不与配置事件混用。
use crate::domain::{
    identity::{PointId, UnitId},
    point_table::RawValue,
};
use serde::{Deserialize, Serialize};
/// 可可靠记录并供观测的实际运行事实。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SlaveFact {
    /// 实际RTU字节及其完整性；发送准备不等于驱动写出或远端确认。
    RtuFrame {
        /// RX或TX。
        direction: String,
        /// 完整十六进制字节。
        hex: String,
        /// 软件是否取得完整帧。
        complete: bool,
        /// 观测CRC结果。
        #[serde(rename = "crcValid")]
        crc_valid: bool,
        /// 非本站、广播查询或非法帧被忽略。
        discarded: bool,
        /// received、prepared或driverWritten。
        stage: String,
    },
    /// 监听已建立。
    Listening,
    /// 停止请求已接收。
    Stopping,
    /// 所有连接及监听已释放。
    Stopped,
    /// 新连接已接受。
    Connected {
        /// 连接本次运行内身份。
        client: u64,
        /// 对端地址。
        peer: String,
    },
    /// 连接已关闭。
    Disconnected {
        /// 连接身份。
        client: u64,
        /// 关闭原因。
        reason: String,
    },
    /// 已收到的完整请求；不是远端执行确认。
    Request {
        /// 连接身份。
        client: u64,
        /// 原始TCP ADU。
        bytes: Vec<u8>,
    },
    /// 响应已准备并排队，尚不代表实际写出。
    ResponsePrepared {
        /// 连接身份。
        client: u64,
        /// 原始TCP ADU。
        bytes: Vec<u8>,
    },
    /// 单条响应已完整写出本机socket；不是远端应用确认。
    ResponseSent {
        /// 连接身份。
        client: u64,
    },
    /// 本地批量修改在提交前已可靠记录。
    ValuesIntent {
        /// Unit身份。
        unit: UnitId,
        /// 修改值。
        values: Vec<(PointId, RawValue)>,
    },
    /// 本地批量修改已提交。
    ValuesCommitted {
        /// Unit身份。
        unit: UnitId,
    },
}
