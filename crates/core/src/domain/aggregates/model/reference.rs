//! 采集模型的稳定身份引用。
use crate::domain::identity::ModelId;
use serde::{Deserialize, Serialize};
/// 通过稳定身份引用采集模型。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    /// 模型身份。
    pub id: ModelId,
}
