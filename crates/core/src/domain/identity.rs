//! 稳定业务身份，不使用显示名称或行号。
use serde::{Deserialize, Serialize};
macro_rules! identity {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub struct $name(#[doc = "不可变的不透明标识；不使用显示名称、站号或树路径。"] pub String);
    };
}
identity!(DeviceId, "设备稳定身份。");
identity!(ModelId, "采集模型稳定身份。");
identity!(PointId, "模型内点的稳定身份。");
identity!(EndpointId, "通信端点或串口通道稳定身份。");
identity!(FolderId, "会话导航文件夹稳定身份。");
identity!(UnitId, "逻辑单元稳定身份，与线上站号分离。");
identity!(RunId, "一次显式启动的运行身份。");
identity!(ReplayId, "离线回放会话身份。");
