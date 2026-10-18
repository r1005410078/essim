//! 可跨边界展示的业务错误及稳定身份定位。
use super::identity::*;
use serde::{Deserialize, Serialize};
/// 应用层可跨边界展示的业务错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    /// 所属端点身份。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint_id: Option<String>,
    /// 所属单元身份。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit_id: Option<String>,
    /// 冲突单元身份。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_unit_id: Option<String>,
    /// 机器可识别的错误码。
    pub code: String,
    /// 中文错误原因。
    pub message: String,
    /// 可定位的稳定点身份，非点表错误时为空。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub point_id: Option<String>,
    /// 出错字段标识。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub field: Option<String>,
    /// 地址冲突时关联的另一点。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub related_point: Option<String>,
}
impl Error {
    /// 构造业务错误，不包含底层通信句柄。
    pub fn new(code: &str, message: impl Into<String>) -> Self {
        Self {
            endpoint_id: None,
            unit_id: None,
            related_unit_id: None,
            code: code.into(),
            message: message.into(),
            point_id: None,
            field: None,
            related_point: None,
        }
    }
    /// 关联端点、单元及属性，供工作台定位。
    pub fn at_unit(mut self, endpoint: &EndpointId, unit: Option<&UnitId>, field: &str) -> Self {
        self.endpoint_id = Some(endpoint.0.clone());
        self.unit_id = unit.map(|id| id.0.clone());
        self.field = Some(field.into());
        self
    }
    /// 将错误关联到点表稳定身份及字段，供前端定位。
    pub fn at_point(mut self, point: &PointId, field: &str) -> Self {
        self.point_id = Some(point.0.clone());
        self.field = Some(field.into());
        self
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Error {}
