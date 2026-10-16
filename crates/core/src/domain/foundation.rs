//! 正式 Modbus 配置模型；四遥分类、通信角色和协议数据区相互独立。
pub use super::point_table::{Order, RawValue, ValueType};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

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
identity!(UnitId, "逻辑单元稳定身份，与线上站号分离。");
identity!(RunId, "一次显式启动的运行身份。");
identity!(ReplayId, "离线回放会话身份。");

/// 应用层可跨边界展示的业务错误。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Error {
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
            code: code.into(),
            message: message.into(),
            point_id: None,
            field: None,
            related_point: None,
        }
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

/// 通信角色，由端点统一确定，下属单元继承。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Role {
    /// 主站主动访问远端。
    Master,
    /// 从站提供本地映射。
    Slave,
}
/// 四遥业务分类，不决定 Modbus 数据区。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Telemetry {
    /// 遥测。
    Measurement,
    /// 遥信。
    Indication,
    /// 遥控。
    Control,
    /// 遥调。
    Adjustment,
}
/// Modbus 四种独立数据区；地址均为零起始。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Area {
    /// 保持寄存器。
    Holding,
    /// 输入寄存器。
    Input,
    /// 线圈。
    Coil,
    /// 离散输入。
    Discrete,
}
/// 点定义；公共映射只定义一次，角色字段并存。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PointDefinition {
    /// 稳定点身份。
    pub id: PointId,
    /// 点名。
    pub name: String,
    /// 四遥分类。
    pub category: Telemetry,
    /// 协议数据区。
    pub area: Area,
    /// 零起始地址，非4xxxx展示编号。
    pub address: u16,
    /// 原始值类型。
    pub value_type: ValueType,
    /// 原始值乘此精确十进制倍率得到工程值。
    pub scale: String,
    /// 工程单位。
    pub unit: String,
    /// 从站初始原始值，由工程输入反算并校验。
    pub initial: RawValue,
    /// 用户输入的初始工程值，保留十进制文本精度。
    pub initial_engineering: String,
    /// 寄存器内部字节顺序。
    pub byte_order: Order,
    /// 多寄存器16位字的顺序。
    pub word_order: Order,
    /// 用户备注。
    pub note: String,
    /// 从站是否允许外部写入。
    pub slave_writable: bool,
    /// 主站建议采集周期，毫秒。
    pub master_period_ms: u32,
}
/// 采集模型的不可变保存版本。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CollectionModel {
    /// 模型身份。
    pub id: ModelId,
    /// 名称。
    pub name: String,
    /// 单调递增版本，以十进制字符串跨IPC。
    #[serde(with = "decimal")]
    pub version: u64,
    /// 唯一点集合，主从角色共用。
    pub points: Vec<PointDefinition>,
}
impl CollectionModel {
    /// 校验基础点定义；完整地址占用和编码规则由M02实现。
    /// # Errors
    /// 名称或ID为空、重复点ID、类型与数据区不符、倍率无效或采集周期为零时失败。
    pub fn validate(&self) -> Result<(), Error> {
        let mut ids = BTreeSet::new();
        if self.id.0.is_empty() || self.name.trim().is_empty() || self.version == 0 {
            return Err(Error::new("INVALID_MODEL", "模型身份、名称和版本不能为空"));
        }
        for point in &self.points {
            if !ids.insert(&point.id) {
                return Err(
                    Error::new("DUPLICATE_POINT_ID", "点身份重复").at_point(&point.id, "id")
                );
            }
            point.validate()?;
        }
        super::point_table::validate_addresses(&self.points)?;
        Ok(())
    }
}
/// 引用具体不可变模型版本，不隐式跟随最新版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRef {
    /// 模型身份。
    pub id: ModelId,
    /// 保存版本。
    #[serde(with = "decimal")]
    pub version: u64,
}
/// 可选关联的设备实体；并非必须存在的真实硬件。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Device {
    /// 设备身份。
    pub id: DeviceId,
    /// 名称。
    pub name: String,
    /// 自定义设备类型，例如PCS。
    pub kind: String,
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
/// 端点聚合内的逻辑单元；无独立通信角色。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnitConfig {
    /// 单元稳定身份。
    pub id: UnitId,
    /// 显示名称。
    pub name: String,
    /// 线上站号；本阶段产品限制1–247，不是TCP标准上限。
    pub station: u8,
    /// 是否参与下一次应用。
    pub enabled: bool,
    /// 具体模型版本。
    pub model: ModelRef,
    /// 可选设备关联。
    pub device: Option<DeviceId>,
}
/// 通信配置聚合，统一校验下属单元站号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointConfig {
    /// 端点身份。
    pub id: EndpointId,
    /// 显示名称。
    pub name: String,
    /// 权威保存版本。
    #[serde(with = "decimal")]
    pub revision: u64,
    /// 所有单元继承的通信角色。
    pub role: Role,
    /// 通信参数。
    pub transport: Transport,
    /// 独立模型引用的单元集合。
    pub units: Vec<UnitConfig>,
    /// 已应用版本；保存不改变此字段。
    #[serde(with = "optional_decimal")]
    pub applied_revision: Option<u64>,
}
impl EndpointConfig {
    /// 校验聚合内身份、站号和通信参数；不打开通道。
    /// # Errors
    /// 重复身份／站号、非法参数或RTU从站超过一个单元时失败。
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.0.is_empty() || self.name.trim().is_empty() {
            return Err(Error::new("INVALID_ENDPOINT", "端点身份和名称不能为空"));
        }
        match &self.transport {
            Transport::Tcp { address, port }
                if address.parse::<std::net::Ipv4Addr>().is_err() || *port == 0 =>
            {
                return Err(Error::new(
                    "INVALID_TRANSPORT",
                    "TCP须使用IPv4地址和非零端口",
                ))
            }
            Transport::Rtu {
                path,
                baud,
                data_bits,
                stop_bits,
                ..
            } if path.trim().is_empty()
                || ![9600, 19200, 38400, 57600, 115200].contains(baud)
                || *data_bits != 8
                || ![1, 2].contains(stop_bits) =>
            {
                return Err(Error::new("INVALID_TRANSPORT", "串口路径或参数无效"))
            }
            _ => {}
        }
        if matches!(self.transport, Transport::Rtu { .. })
            && self.role == Role::Slave
            && self.units.len() > 1
        {
            return Err(Error::new(
                "RTU_SINGLE_STATION",
                "首版RTU从站通道只允许一个单元",
            ));
        }
        let mut stations = BTreeSet::new();
        let mut ids = BTreeSet::new();
        for unit in &self.units {
            if unit.id.0.is_empty()
                || unit.name.trim().is_empty()
                || !(1..=247).contains(&unit.station)
                || !stations.insert(unit.station)
                || !ids.insert(&unit.id)
            {
                return Err(Error::new("INVALID_UNIT", "单元身份／站号重复或超出1–247"));
            }
        }
        Ok(())
    }
}
/// 已应用单元的会话内值；运行时接入后所有权归执行器。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UnitValues {
    /// 实际应用的模型版本。
    pub model: Option<ModelRef>,
    /// 原始值，不随倍率变化重算。
    pub raw: BTreeMap<PointId, RawValue>,
    /// 主站上次结果是否过期。
    pub stale: bool,
}
/// 倍率变化时同一原始值对应的工程显示变化。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EngineeringChange {
    /// 稳定点身份。
    pub point: PointId,
    /// 原倍率下的工程显示。
    pub previous: String,
    /// 新倍率下的工程显示。
    pub next: String,
}
/// 应用预览，展示增删、恢复及倍率变化。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Migration {
    /// 原始值保留时的工程值变化。
    pub engineering_changed: Vec<EngineeringChange>,
    /// 迁移后的独立值。
    pub values: UnitValues,
    /// 倍率改变而保留原始值的点。
    pub scale_changed: Vec<PointId>,
    /// 新增或明确恢复初值的点。
    pub initialized: Vec<PointId>,
    /// 删除的点。
    pub removed: Vec<PointId>,
}
/// 纯迁移预览；失败不修改原模型或原始值。
/// # Errors
/// 类型变化且未显式允许恢复初值时返回不兼容错误。
pub fn migrate(
    old: Option<&CollectionModel>,
    current: &UnitValues,
    next: &CollectionModel,
    reset_incompatible: bool,
) -> Result<Migration, Error> {
    next.validate()?;
    let mut preview = Migration::default();
    preview.values.model = Some(ModelRef {
        id: next.id.clone(),
        version: next.version,
    });
    preview.values.stale = current.stale;
    let previous_points: BTreeMap<_, _> = old
        .filter(|model| model.id == next.id)
        .map(|model| {
            model
                .points
                .iter()
                .map(|point| (&point.id, point))
                .collect()
        })
        .unwrap_or_default();
    for point in &next.points {
        let previous = previous_points.get(&point.id).copied();
        let value = match previous {
            Some(previous)
                if super::point_table::convert_compatible(
                    previous.value_type,
                    point.value_type,
                    current.raw.get(&point.id).unwrap_or(&previous.initial),
                )
                .is_some() =>
            {
                if previous.scale != point.scale {
                    preview.scale_changed.push(point.id.clone());
                    let raw = current.raw.get(&point.id).unwrap_or(&previous.initial);
                    preview.engineering_changed.push(EngineeringChange {
                        point: point.id.clone(),
                        previous: super::point_table::engineering_value(raw, &previous.scale)?,
                        next: super::point_table::engineering_value(raw, &point.scale)?,
                    });
                }
                super::point_table::convert_compatible(
                    previous.value_type,
                    point.value_type,
                    current.raw.get(&point.id).unwrap_or(&previous.initial),
                )
                .ok_or_else(|| Error::new("INCOMPATIBLE_TYPE", "点值不能保留"))?
            }
            Some(_) if !reset_incompatible => {
                return Err(Error::new(
                    "INCOMPATIBLE_TYPE",
                    "点类型变化，请明确选择恢复初值",
                ))
            }
            _ => {
                preview.initialized.push(point.id.clone());
                point.initial.clone()
            }
        };
        preview.values.raw.insert(point.id.clone(), value);
    }
    preview.removed = current
        .raw
        .keys()
        .filter(|id| !preview.values.raw.contains_key(*id))
        .cloned()
        .collect();
    Ok(preview)
}
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
/// 可选版本的IPC字符串编码。
pub mod optional_decimal {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    /// 可选值编码为字符串或null。
    pub fn serialize<S: Serializer>(value: &Option<u64>, serializer: S) -> Result<S::Ok, S::Error> {
        value.map(|v| v.to_string()).serialize(serializer)
    }
    /// 解码可选版本。
    /// # Errors
    /// 非法整数时失败。
    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<u64>, D::Error> {
        Option::<String>::deserialize(deserializer)?
            .map(|v| v.parse().map_err(serde::de::Error::custom))
            .transpose()
    }
}

/// 显式复位静止实例的本地值，不调用任何协议端口。
/// 从站恢复已应用模型初值；主站仅清空缓存并标记过期，不能隐式向远端写入。
pub fn reset_values(role: Role, model: &CollectionModel) -> UnitValues {
    UnitValues {
        model: Some(ModelRef {
            id: model.id.clone(),
            version: model.version,
        }),
        raw: if role == Role::Slave {
            model
                .points
                .iter()
                .map(|point| (point.id.clone(), point.initial.clone()))
                .collect()
        } else {
            BTreeMap::new()
        },
        stale: role == Role::Master,
    }
}
