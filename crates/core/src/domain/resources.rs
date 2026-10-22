//! 运行资源预算与无副作用的额度计算；估计值不是OS内存测量。
use crate::domain::error::Error;
use serde::{Deserialize, Serialize};
/// 分开计费的资源维度，所有字节均为工程估计。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceUsage {
    /// 预计分配内存字节，包括暂存峰值。
    pub memory_bytes: u64,
    /// 运行端点数量。
    pub endpoints: u64,
    /// 实际／预留连接数量。
    pub connections: u64,
    /// 控制与录制队列字节。
    pub queue_bytes: u64,
}
impl ResourceUsage {
    /// 校验并计算新增占用，不允许整数绕回。
    /// # Errors
    /// 任一维度溢出或超过预算。
    pub fn admit(self, delta: Self, limit: Self) -> Result<Self, Error> {
        let sum = Self {
            memory_bytes: self
                .memory_bytes
                .checked_add(delta.memory_bytes)
                .ok_or_else(overflow)?,
            endpoints: self
                .endpoints
                .checked_add(delta.endpoints)
                .ok_or_else(overflow)?,
            connections: self
                .connections
                .checked_add(delta.connections)
                .ok_or_else(overflow)?,
            queue_bytes: self
                .queue_bytes
                .checked_add(delta.queue_bytes)
                .ok_or_else(overflow)?,
        };
        for (name, used, requested, max) in [
            (
                "内存字节",
                self.memory_bytes,
                delta.memory_bytes,
                limit.memory_bytes,
            ),
            ("运行端点", self.endpoints, delta.endpoints, limit.endpoints),
            (
                "连接",
                self.connections,
                delta.connections,
                limit.connections,
            ),
            (
                "队列字节",
                self.queue_bytes,
                delta.queue_bytes,
                limit.queue_bytes,
            ),
        ] {
            if used.checked_add(requested).is_none_or(|v| v > max) {
                return Err(Error::new(
                    "RESOURCE_LIMIT",
                    format!("{name}超限：当前{used}，申请{requested}，上限{max}"),
                ));
            }
        }
        Ok(sum)
    }
}
fn overflow() -> Error {
    Error::new("RESOURCE_OVERFLOW", "资源估计超出整数范围")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_and_each_limit_refuse_without_changing_usage() {
        let used = ResourceUsage {
            memory_bytes: u64::MAX,
            ..Default::default()
        };
        assert_eq!(
            used.admit(
                ResourceUsage {
                    memory_bytes: 1,
                    ..Default::default()
                },
                used
            )
            .unwrap_err()
            .code,
            "RESOURCE_OVERFLOW"
        );
        for delta in [
            ResourceUsage {
                memory_bytes: 1,
                ..Default::default()
            },
            ResourceUsage {
                endpoints: 1,
                ..Default::default()
            },
            ResourceUsage {
                connections: 1,
                ..Default::default()
            },
            ResourceUsage {
                queue_bytes: 1,
                ..Default::default()
            },
        ] {
            assert_eq!(
                ResourceUsage::default()
                    .admit(delta, ResourceUsage::default())
                    .unwrap_err()
                    .code,
                "RESOURCE_LIMIT"
            );
        }
    }
}
