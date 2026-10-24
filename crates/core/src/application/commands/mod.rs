//! 强类型写入意图与用例处理器；不引入通用消息总线。
use super::events::ConfigurationFact;
use crate::domain::navigation::Folder;
use crate::domain::{
    device::Device,
    endpoint::EndpointConfig,
    identity::{EndpointId, FolderId, ModelId, RunId, UnitId},
    model::{CollectionModel, ModelRef},
    serialization::decimal,
};
use serde::{Deserialize, Serialize};
/// 命令携带因果标识。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    /// 调用者生成的命令身份，用于关联结果和事件。
    pub command_id: String,
    /// 强类型业务操作。
    pub action: Action,
}
/// 最小配置命令及尚未接入的真实运行入口。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Action {
    /// 在指定目录原子创建一个业务资源。
    CreateInFolder {
        /// 根目录为空，按资源类型确定目录范围。
        parent: Option<FolderId>,
        /// 限定模型或模拟器创建载荷。
        resource: NewNavigationResource,
    },
    /// 原子调整选中对象的导航父级，不改变运行配置。
    MoveNavigation {
        /// 去重前的稳定对象身份，领域层合并父子混选。
        nodes: Vec<crate::domain::navigation::NavigationNode>,
        /// 目标导航分类。
        scope: crate::domain::navigation::FolderScope,
        /// 根目录为空。
        parent: Option<FolderId>,
    },
    /// 新模型与模拟器一次创建。
    CreateEndpointWithModel {
        /// 首个单元引用该模型的端点。
        endpoint: EndpointConfig,
        /// 新模型。
        model: CollectionModel,
    },
    /// 将选定已保存差异回写模型。
    PromoteOverrides {
        /// 需重新核实的内容预览。
        plan: overrides::PromotionPlan,
    },
    /// 展平有效配置为独立模型并重绑。
    ExtractUnitModel {
        /// 源端点。
        endpoint: EndpointId,
        /// 源单元。
        unit: UnitId,
        /// 新模型身份。
        id: ModelId,
        /// 新模型名称。
        name: String,
    },
    /// 复制当前已保存模型，生成独立模型与点身份。
    CopyModel {
        /// 源模型身份。
        source: ModelRef,
        /// 新模型稳定身份，必须尚未使用。
        id: ModelId,
        /// 副本名称。
        name: String,
    },
    /// 无引用时删除模型当前配置。
    DeleteModel {
        /// 模型稳定身份。
        id: ModelId,
    },
    /// 新增或保存模型。
    SaveModel {
        /// 完整模型草稿，保存时覆盖同一模型。
        model: CollectionModel,
    },
    /// 原子创建包含首个Unit的端点。
    CreateEndpoint {
        /// 完整聚合。
        endpoint: EndpointConfig,
    },
    /// 复制当前保存配置，不带应用及会话值。
    CopyEndpoint {
        /// 源端点。
        source: EndpointId,
        /// 全新目标身份。
        id: EndpointId,
    },
    /// 确认释放后删除整个端点。
    DeleteEndpoint {
        /// 目标身份。
        id: EndpointId,
    },
    /// 恢复选定停止单元的已应用初值。
    ResetUnit {
        /// 端点身份。
        id: EndpointId,
        /// 单元身份。
        unit: UnitId,
    },
    /// 新建或修改导航文件夹。
    SaveFolder {
        /// 完整导航节点。
        folder: Folder,
    },
    /// 只删除空文件夹。
    DeleteFolder {
        /// 节点身份。
        id: FolderId,
    },
    /// 移动模拟器，不触发停机。
    MoveEndpoint {
        /// 端点身份。
        id: EndpointId,
        /// 根目录为空。
        parent: Option<FolderId>,
    },
    /// 保存既存端点；修改不会自动应用。
    SaveEndpoint {
        /// 聚合草稿。
        endpoint: EndpointConfig,
    },
    /// 新增可选设备资源。
    CreateDevice {
        /// 待创建实体。
        device: Device,
    },
    /// 原子应用一个端点的全部单元。
    Apply {
        /// 目标端点。
        id: EndpointId,
        /// 显式允许不兼容点恢复初值。
        #[serde(rename = "resetIncompatible")]
        reset_incompatible: bool,
    },
    /// 真实协议启动，M01生产装配明确拒绝。
    Start {
        /// 目标端点。
        id: EndpointId,
    },
    /// 真实读取，绝不是查询缓存。
    ReadRemote {
        /// 目标端点。
        id: EndpointId,
    },
    /// 对外写入；未接入执行器和可靠录制时拒绝。
    WriteRemote {
        /// 目标端点。
        id: EndpointId,
    },
    /// 加载真实历史回放；M01仅提供契约测试。
    Replay {
        /// 原运行身份。
        run: RunId,
    },
}
/// 命令确认，和投影位置关联。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    /// 修改的稳定对象。
    pub object_id: String,
    /// 查询可等待的配置通知位置。
    #[serde(with = "decimal")]
    pub sequence: u64,
}

/// 处理器完成校验后的事务内结果；仅提交成功后才能转成公开事件。
pub(crate) struct Handled {
    pub object_id: String,
    pub fact: ConfigurationFact,
}
pub(crate) mod device;
pub(crate) mod endpoint;
pub(crate) mod lifecycle;
pub(crate) mod model;
pub(crate) mod navigation;
pub use navigation::NewNavigationResource;
pub(crate) mod operations;

/// 独立从站运行命令。
pub mod slave;

/// TCP主站职责。
pub mod master;

/// 点位分组创建用例。
pub mod partition;

/// 批量生命周期任务契约。
pub mod batch;

/// 批量固定范围与内容复核。
pub mod batch_targets;

/// 模型到 Unit 覆盖事务与回写预览。
pub mod overrides;
