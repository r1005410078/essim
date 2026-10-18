//! 精确数值的跨边界序列化。
/// IPC中的u64使用十进制字符串，避免JavaScript精度损失。
pub mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    /// 序列化为字符串。
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    /// 从十进制字符串读取。
    /// # Errors
    /// 非整数、负数或溢出时失败。
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        String::deserialize(deserializer)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}
