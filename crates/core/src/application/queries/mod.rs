//! 只读视图、应用预览和差异查询；不访问写入端口。
/// 完整应用预览。
pub mod apply_preview;
/// 当前实际能力。
pub mod capabilities;
/// 协议差异检查。
pub mod inspection;
/// 查询投影状态。
pub mod projection;
/// 工作区查询结果。
pub mod workspace_view;
pub(crate) use apply_preview::prepare_apply;
pub use apply_preview::ApplyPreview;
pub use capabilities::Capabilities;
pub(crate) use inspection::application_matches;
pub use inspection::{inspect_endpoint, protocol_model_matches};
pub use projection::Projection;
pub use workspace_view::WorkspaceView;

/// 独立从站运行查询。
pub mod slave;

/// TCP主站职责。
pub mod master;
