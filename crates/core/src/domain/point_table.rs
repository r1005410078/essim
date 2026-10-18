//! 点表数值转换与寄存器编码；不依赖通信句柄或窗口。
//!
//! # Examples
//! ```
//! use essim_core::domain::point_table::{from_engineering, encode, ValueType, Order};
//! let raw = from_engineering(ValueType::U32, "305419896", "1")?;
//! assert_eq!(encode(ValueType::U32, &raw, Order::Big, Order::Little)?, vec![0x5678, 0x1234]);
//! # Ok::<(), essim_core::domain::error::Error>(())
//! ```
use super::{
    error::Error,
    model::{Area, PointDefinition, Telemetry},
};
use num_bigint::BigInt;
use num_rational::BigRational;
use num_traits::{One, ToPrimitive, Zero};
use serde::{Deserialize, Serialize};

/// 数值解释类型，不改变Modbus单寄存器16位的协议宽度。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ValueType {
    /// 单个位地址的布尔值。
    Bool,
    /// 有符号16位整数。
    I16,
    /// 无符号16位整数。
    U16,
    /// 有符号32位整数。
    I32,
    /// 无符号32位整数。
    U32,
    /// 有符号64位整数。
    I64,
    /// 无符号64位整数。
    U64,
    /// IEEE 754单精度浮点数。
    F32,
    /// IEEE 754双精度浮点数。
    F64,
}
impl ValueType {
    /// 返回位地址或16位寄存器的占用数量。
    pub fn width(self) -> u32 {
        match self {
            Self::Bool | Self::I16 | Self::U16 => 1,
            Self::I32 | Self::U32 | Self::F32 => 2,
            Self::I64 | Self::U64 | Self::F64 => 4,
        }
    }
    fn bounds(self) -> Option<(BigInt, BigInt)> {
        let pair = match self {
            Self::I16 => (i16::MIN as i128, i16::MAX as i128),
            Self::U16 => (0, u16::MAX as i128),
            Self::I32 => (i32::MIN as i128, i32::MAX as i128),
            Self::U32 => (0, u32::MAX as i128),
            Self::I64 => (i64::MIN as i128, i64::MAX as i128),
            Self::U64 => (0, u64::MAX as i128),
            _ => return None,
        };
        Some((pair.0.into(), pair.1.into()))
    }
    fn signed(self) -> bool {
        matches!(self, Self::I16 | Self::I32 | Self::I64)
    }
}
/// 字节或16位字的顺序；默认高位在前。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Order {
    /// 高位在前，不交换。
    #[default]
    Big,
    /// 字节交换或所有16位字倒序。
    Little,
}
/// 原始业务数值；整数跨IPC使用文本，浮点同样使用有限数值文本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "camelCase")]
pub enum RawValue {
    /// 布尔值。
    Bool(bool),
    /// 有符号整数，文本序列化避免JS精度丢失。
    Signed(#[serde(with = "signed_text")] i64),
    /// 无符号整数，文本序列化避免JS精度丢失。
    Unsigned(#[serde(with = "super::serialization::decimal")] u64),
    /// 有限浮点数，文本序列化。
    Float(#[serde(with = "float_text")] f64),
}
impl std::fmt::Display for RawValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{v}"),
            Self::Signed(v) => write!(f, "{v}"),
            Self::Unsigned(v) => write!(f, "{v}"),
            Self::Float(v) => write!(f, "{v}"),
        }
    }
}
macro_rules! text_codec {
    ($name:ident, $ty:ty, $valid:expr) => {
        mod $name {
            use serde::{Deserialize, Deserializer, Serializer};
            pub fn serialize<S: Serializer>(value: &$ty, s: S) -> Result<S::Ok, S::Error> {
                if !($valid)(*value) {
                    return Err(serde::ser::Error::custom("非有限数值"));
                }
                s.serialize_str(&value.to_string())
            }
            pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<$ty, D::Error> {
                let text = String::deserialize(d)?;
                if text.len() > 512 {
                    return Err(serde::de::Error::custom("数值过长"));
                }
                let value: $ty = text.parse().map_err(serde::de::Error::custom)?;
                if !($valid)(value) {
                    return Err(serde::de::Error::custom("非有限数值"));
                }
                Ok(value)
            }
        }
    };
}
text_codec!(signed_text, i64, |_: i64| true);
text_codec!(float_text, f64, |v: f64| v.is_finite());

/// 精确解析十进制数；限制512字符和绝对指数400，防止无界大整数分配。
/// # Errors
/// 非法十进制、长度或指数超限时失败。
pub fn decimal(text: &str) -> Result<BigRational, Error> {
    let text = text.trim();
    let invalid = || {
        Error::new(
            "INVALID_NUMBER",
            "须为有限十进制数，最多512字符，指数绝对值不超过400",
        )
    };
    if text.is_empty() || text.len() > 512 {
        return Err(invalid());
    }
    let (mantissa, exponent) = match text.split_once(['e', 'E']) {
        Some((m, e)) => (m, e.parse::<i32>().map_err(|_| invalid())?),
        None => (text, 0),
    };
    if !(-400..=400).contains(&exponent) {
        return Err(invalid());
    }
    let (negative, unsigned) = if let Some(v) = mantissa.strip_prefix('-') {
        (true, v)
    } else {
        (false, mantissa.strip_prefix('+').unwrap_or(mantissa))
    };
    let (whole, fractional) = unsigned.split_once('.').unwrap_or((unsigned, ""));
    let digits = format!("{whole}{fractional}");
    if digits.is_empty() || !digits.bytes().all(|v| v.is_ascii_digit()) {
        return Err(invalid());
    }
    let mut numerator = BigInt::parse_bytes(digits.as_bytes(), 10).ok_or_else(invalid)?;
    if negative {
        numerator = -numerator;
    }
    let shift = exponent - fractional.len() as i32;
    if shift >= 0 {
        Ok(BigRational::from_integer(
            numerator * BigInt::from(10).pow(shift as u32),
        ))
    } else {
        Ok(BigRational::new(
            numerator,
            BigInt::from(10).pow((-shift) as u32),
        ))
    }
}
fn scale_value(scale: &str) -> Result<BigRational, Error> {
    let value = decimal(if scale.trim().is_empty() { "1" } else { scale })?;
    if value.is_zero() {
        Err(Error::new("INVALID_SCALE", "倍率不能为0"))
    } else {
        Ok(value)
    }
}
/// 校验倍率的十进制形式和Bool固定倍率约束。
/// # Errors
/// 倍率非法、为0或Bool倍率非1时失败。
pub fn validate_scale(kind: ValueType, scale: &str) -> Result<(), Error> {
    let value = scale_value(scale).map_err(|e| Error::new("INVALID_SCALE", e.message))?;
    if kind == ValueType::Bool && value != BigRational::one() {
        return Err(Error::new("INVALID_SCALE", "Bool倍率必须为1"));
    }
    Ok(())
}
fn from_integer(kind: ValueType, value: BigInt) -> Result<RawValue, Error> {
    let (min, max) = kind
        .bounds()
        .ok_or_else(|| Error::new("INVALID_TYPE", "非整数类型"))?;
    if value < min || value > max {
        return Err(Error::new(
            "VALUE_OVERFLOW",
            format!("数值超出{kind:?}范围{min}～{max}"),
        ));
    }
    if kind.signed() {
        value.to_i64().map(RawValue::Signed)
    } else {
        value.to_u64().map(RawValue::Unsigned)
    }
    .ok_or_else(|| Error::new("VALUE_OVERFLOW", "整数转换越界"))
}
/// 将工程值反算为原始值；整数必须精确可表示，不静默舍入。
/// # Errors
/// 非法倍率、非法数值、不整除、越界、Bool非开关值时失败。
pub fn from_engineering(
    kind: ValueType,
    engineering: &str,
    scale: &str,
) -> Result<RawValue, Error> {
    let multiplier = scale_value(scale)?;
    if kind == ValueType::Bool {
        if multiplier != BigRational::one() {
            return Err(Error::new("INVALID_SCALE", "Bool倍率必须为1"));
        }
        return match engineering.trim() {
            "true" | "1" => Ok(RawValue::Bool(true)),
            "false" | "0" => Ok(RawValue::Bool(false)),
            _ => Err(Error::new("INVALID_BOOL", "Bool须为开或关")),
        };
    }
    let value = decimal(engineering)? / &multiplier;
    if let Some((min, max)) = kind.bounds() {
        if value < BigRational::from_integer(min.clone())
            || value > BigRational::from_integer(max.clone())
        {
            return Err(Error::new(
                "VALUE_OVERFLOW",
                format!("数值超出{kind:?}范围{min}～{max}"),
            ));
        }
        if !value.is_integer() {
            let left = value.floor().to_integer();
            let right = &left + 1;
            let a = finite_decimal(&(BigRational::from_integer(left) * &multiplier));
            let b = finite_decimal(&(BigRational::from_integer(right) * &multiplier));
            return Err(Error::new(
                "UNREPRESENTABLE",
                format!("整数不能表示该工程值，请选择相邻可表示值{a}或{b}"),
            ));
        }
        return from_integer(kind, value.to_integer());
    }
    let mut float = value
        .to_f64()
        .ok_or_else(|| Error::new("VALUE_OVERFLOW", "浮点值溢出"))?;
    if kind == ValueType::F32 {
        float = f64::from(float as f32);
    }
    if !float.is_finite() {
        return Err(Error::new("VALUE_OVERFLOW", "目标浮点类型溢出"));
    }
    Ok(RawValue::Float(float))
}
fn finite_decimal(value: &BigRational) -> String {
    let mut numerator = value.numer().clone();
    let negative = numerator < BigInt::zero();
    if negative {
        numerator = -numerator;
    }
    let whole = &numerator / value.denom();
    let mut remainder = numerator % value.denom();
    let mut fraction = String::new();
    // 十进制倍率及二进制浮点均有有限十进制展开；限制展示长度，不影响校验。
    for _ in 0..1200 {
        if remainder.is_zero() {
            break;
        }
        remainder *= 10;
        fraction.push_str(&(&remainder / value.denom()).to_string());
        remainder %= value.denom();
    }
    let sign = if negative { "-" } else { "" };
    if fraction.is_empty() {
        format!("{sign}{whole}")
    } else {
        format!("{sign}{whole}.{fraction}")
    }
}
/// 根据实际原始值显示工程值，浮点显示目标编码后的数值。
/// # Errors
/// 倍率或原始浮点值无效时失败。
pub fn engineering_value(raw: &RawValue, scale: &str) -> Result<String, Error> {
    let value = match raw {
        RawValue::Bool(v) => return Ok(v.to_string()),
        RawValue::Signed(v) => BigRational::from_integer((*v).into()),
        RawValue::Unsigned(v) => BigRational::from_integer((*v).into()),
        RawValue::Float(v) => BigRational::from_float(*v)
            .ok_or_else(|| Error::new("INVALID_NUMBER", "非有限浮点值"))?,
    };
    Ok(finite_decimal(&(value * scale_value(scale)?)))
}
/// 校验原始值与目标类型匹配，返回规范目标值。
/// # Errors
/// 类型不符或值越界时失败。
pub fn validate_raw(kind: ValueType, raw: &RawValue) -> Result<RawValue, Error> {
    match (kind, raw) {
        (ValueType::Bool, RawValue::Bool(_)) => Ok(raw.clone()),
        (ValueType::F32, RawValue::Float(v)) if v.is_finite() && f64::from(*v as f32) == *v => {
            Ok(raw.clone())
        }
        (ValueType::F64, RawValue::Float(v)) if v.is_finite() => Ok(raw.clone()),
        (k, RawValue::Signed(v)) if k.signed() => from_integer(k, (*v).into()),
        (k, RawValue::Unsigned(v)) if k.bounds().is_some() && !k.signed() => {
            from_integer(k, (*v).into())
        }
        _ => Err(Error::new(
            "INVALID_RAW_VALUE",
            "原始值类型或精度与点定义不符",
        )),
    }
}
/// 按选定字节及字序编码为寄存器；Bool使用0或1的位映射值。
/// # Errors
/// 原始值与类型不匹配或越界时失败。
pub fn encode(
    kind: ValueType,
    raw: &RawValue,
    bytes: Order,
    words: Order,
) -> Result<Vec<u16>, Error> {
    validate_raw(kind, raw)?;
    let bits = match raw {
        RawValue::Bool(v) => u64::from(*v),
        RawValue::Signed(v) => *v as u64,
        RawValue::Unsigned(v) => *v,
        RawValue::Float(v) if kind == ValueType::F32 => (*v as f32).to_bits() as u64,
        RawValue::Float(v) => v.to_bits(),
    };
    let mut registers: Vec<u16> = (0..kind.width())
        .rev()
        .map(|i| (bits >> (i * 16)) as u16)
        .collect();
    if kind != ValueType::Bool {
        if bytes == Order::Little {
            registers.iter_mut().for_each(|v| *v = v.swap_bytes());
        }
        if words == Order::Little {
            registers.reverse();
        }
    }
    Ok(registers)
}
/// 判断运行值能否无损迁移，浮点只允许单精度扩为双精度。
pub fn convert_compatible(old: ValueType, next: ValueType, raw: &RawValue) -> Option<RawValue> {
    validate_raw(old, raw).ok()?;
    if old == next || (old == ValueType::F32 && next == ValueType::F64) {
        return validate_raw(next, raw).ok();
    }
    if old.bounds().is_some() && next.bounds().is_some() {
        let number = match raw {
            RawValue::Signed(v) => BigInt::from(*v),
            RawValue::Unsigned(v) => BigInt::from(*v),
            _ => return None,
        };
        return from_integer(next, number).ok();
    }
    None
}

impl PointDefinition {
    /// 校验单点编码、工程输入及业务用途，不执行任何通信。
    /// # Errors
    /// 控制点使用只读区、非法类型／倍率／初始值或非规范编码时失败。
    pub fn validate(&self) -> Result<(), Error> {
        let fail = |field: &str, message: &str| {
            Error::new(
                "INVALID_POINT",
                format!("点{}，字段{field}：{message}", self.name),
            )
            .at_point(&self.id, field)
        };
        if self.id.0.trim().is_empty() {
            return Err(fail("id", "点身份不能为空"));
        }
        if self.name.trim().is_empty() {
            return Err(fail("name", "点名不能为空"));
        }
        if self.master_period_ms == 0 {
            return Err(fail("masterPeriodMs", "采集周期须为正整数毫秒"));
        }
        validate_scale(self.value_type, &self.scale).map_err(|e| fail("scale", &e.message))?;
        if matches!(self.category, Telemetry::Control | Telemetry::Adjustment)
            && matches!(self.area, Area::Input | Area::Discrete)
        {
            return Err(fail("area", "遥控／遥调须使用holding或coil"));
        }
        if self.value_type == ValueType::Bool
            && (self.byte_order != Order::Big || self.word_order != Order::Big)
        {
            return Err(fail("byteOrder", "Bool不适用编码交换"));
        }
        if self.value_type.width() == 1 && self.word_order != Order::Big {
            return Err(fail("wordOrder", "单地址类型不适用字序交换"));
        }
        let expected = from_engineering(self.value_type, &self.initial_engineering, &self.scale)
            .map_err(|e| fail("initialEngineering", &e.message))?;
        if expected != self.initial {
            return Err(fail("initial", "原始初值与工程输入不一致"));
        }
        let bit_area = matches!(self.area, Area::Coil | Area::Discrete);
        if bit_area != (self.value_type == ValueType::Bool) {
            return Err(fail(
                "valueType",
                "coil／discrete只使用Bool，holding／input使用数值类型",
            ));
        }
        if self.slave_writable && matches!(self.area, Area::Input | Area::Discrete) {
            return Err(fail("slaveWritable", "input／discrete不允许外部写入"));
        }
        validate_raw(self.value_type, &self.initial).map_err(|e| fail("initial", &e.message))?;
        Ok(())
    }
}
/// 跨分类校验同数据区地址区间；不同区相同数字地址不冲突。
/// # Errors
/// 多寄存器越界或区间重叠时返回含双方点身份的错误。
pub fn validate_addresses(points: &[PointDefinition]) -> Result<(), Error> {
    for area in [Area::Holding, Area::Input, Area::Coil, Area::Discrete] {
        let mut sorted: Vec<_> = points.iter().filter(|p| p.area == area).collect();
        sorted.sort_by_key(|p| p.address);
        let mut previous: Option<&PointDefinition> = None;
        for point in sorted {
            let end = u32::from(point.address) + point.value_type.width() - 1;
            if end > u16::MAX as u32 {
                return Err(Error::new(
                    "ADDRESS_OVERFLOW",
                    format!("点{}，字段address：末地址{end}超过65535", point.id.0),
                )
                .at_point(&point.id, "address"));
            }
            if let Some(old) = previous {
                let old_end = u32::from(old.address) + old.value_type.width() - 1;
                if u32::from(point.address) <= old_end {
                    let mut error = Error::new(
                        "ADDRESS_OVERLAP",
                        format!(
                            "点{}与{}，字段address：{area:?}地址重叠",
                            old.name, point.name
                        ),
                    )
                    .at_point(&point.id, "address");
                    error.related_point = Some(old.id.0.clone());
                    return Err(error);
                }
            }
            previous = Some(point);
        }
    }
    Ok(())
}
