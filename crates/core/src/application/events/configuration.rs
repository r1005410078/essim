//! 成功配置提交的业务事实。
use crate::domain::identity::{DeviceId, EndpointId, FolderId, ModelId, UnitId};
use serde::{Deserialize, Serialize};
/// 已成功提交的配置业务事实；不是运行历史或完整事件溯源数据。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ConfigurationFact {
    /// 模型保存完成。
    ModelSaved {
        /// 模型身份。
        id: ModelId,
    },
    /// 模型副本创建完成。
    ModelCopied {
        /// 源模型身份。
        source: ModelId,
        /// 副本身份。
        id: ModelId,
    },
    /// 模型删除完成。
    ModelDeleted {
        /// 已删除模型身份。
        id: ModelId,
    },
    /// 模拟器和首个Unit创建完成。
    EndpointCreated {
        /// 模拟器身份。
        id: EndpointId,
    },
    /// 模拟器保存完成；并不表示已应用。
    EndpointSaved {
        /// 模拟器身份。
        id: EndpointId,
    },
    /// 模拟器副本创建完成。
    EndpointCopied {
        /// 源模拟器身份。
        source: EndpointId,
        /// 副本身份。
        id: EndpointId,
    },
    /// 释放确认后的模拟器删除完成。
    EndpointDeleted {
        /// 已删除模拟器身份。
        id: EndpointId,
    },
    /// 完整模拟器配置应用完成，保持停止。
    EndpointApplied {
        /// 模拟器身份。
        id: EndpointId,
    },
    /// 选定停止Unit的本地值复位完成。
    UnitValuesReset {
        /// 模拟器身份。
        id: EndpointId,
        /// 复位单元身份。
        unit: UnitId,
    },
    /// 导航文件夹保存完成。
    FolderSaved {
        /// 文件夹身份。
        id: FolderId,
    },
    /// 空文件夹删除完成。
    FolderDeleted {
        /// 已删除文件夹身份。
        id: FolderId,
    },
    /// 模拟器导航成员关系移动完成。
    EndpointMoved {
        /// 模拟器身份。
        id: EndpointId,
        /// 新父目录，根目录为空。
        parent: Option<FolderId>,
    },
    /// 可选设备资源创建完成。
    DeviceCreated {
        /// 设备身份。
        id: DeviceId,
    },
}
