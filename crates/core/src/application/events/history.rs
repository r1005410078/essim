//! 运行历史事实及有序记录契约；尚未接入生产可靠存储。
use crate::{
    application::runtime::ExecutionResult,
    domain::{
        identity::{PointId, RunId, UnitId},
        point_table::RawValue,
        serialization::decimal,
    },
};
use serde::{Deserialize, Serialize};
/// 历史中的运行事实、外部效果意图及通信记录。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Fact {
    /// 执行器已确认打开。
    Opened,
    /// 执行外部效果前的可靠意图。
    Intent {
        /// 稳定操作身份。
        operation: String,
    },
    /// 对应操作的结果事实，不重放原命令。
    Result {
        /// 操作身份。
        operation: String,
        /// 技术结果。
        result: ExecutionResult,
    },
    /// 已发生的值变化。
    Value {
        /// 稳定单元身份。
        unit: UnitId,
        /// 稳定点身份。
        point: PointId,
        /// 原始值。
        raw: RawValue,
    },
    /// 实际协议收发记录，不能由UI推测生成。
    Communication {
        /// 发送或接收。
        direction: String,
        /// 原始字节。
        bytes: Vec<u8>,
    },
    /// 临时连接变化，沿用RunId。
    Connection {
        /// 是否连接。
        connected: bool,
    },
    /// 暂停单元采集，沿用RunId。
    UnitPaused {
        /// 单元身份。
        unit: UnitId,
        /// 暂停标记。
        paused: bool,
    },
    /// 资源释放已经确认的可靠结束标记。
    Finalized,
}
/// 一次运行内严格有序的记录。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEvent {
    /// 事件数据版本。
    pub schema: u32,
    /// 运行身份，定位固定配置与模型。
    pub run: RunId,
    /// 运行内记录位置。
    #[serde(with = "decimal")]
    pub sequence: u64,
    /// 相对运行开始的单调时间，毫秒。
    #[serde(with = "decimal")]
    pub relative_ms: u64,
    /// 展示用Unix时间，毫秒。
    #[serde(with = "decimal")]
    pub unix_ms: u64,
    /// 操作因果身份；自主接收事实可为空。
    pub command_id: Option<String>,
    /// 已记录内容。
    pub fact: Fact,
}
