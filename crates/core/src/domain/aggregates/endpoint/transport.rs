//! 端点通信角色和传输参数值对象。
use serde::{Deserialize, Serialize};
/// 通信角色，由端点统一确定，下属单元继承。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// 主站主动访问远端。
    Master,
    /// 从站提供本地映射。
    Slave,
}
/// 串口校验位。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Parity {
    /// 无校验。
    None,
    /// 偶校验。
    Even,
    /// 奇校验。
    Odd,
}
/// TCP参数或RTU参数；不持有实际连接。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Transport {
    /// TCP监听地址或主站目标；首版仅IPv4。
    Tcp {
        /// IPv4地址。
        address: String,
        /// 非零端口。
        port: u16,
    },
    /// RTU串口通道；实际资源独占由运行执行器保证。
    Rtu {
        /// 串口路径。
        path: String,
        /// 波特率，bit/s。
        baud: u32,
        /// 校验位。
        parity: Parity,
        /// 首版仅8位数据位。
        #[serde(rename = "dataBits")]
        data_bits: u8,
        /// 1或2停止位。
        #[serde(rename = "stopBits")]
        stop_bits: u8,
    },
}
