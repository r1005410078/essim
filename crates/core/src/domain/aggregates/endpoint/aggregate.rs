//! 端点聚合根及聚合内不变量。
use super::{Role, Transport, UnitConfig};
use crate::domain::{error::Error, identity::*};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
/// 通信配置聚合，统一校验下属单元站号。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointConfig {
    /// 端点身份。
    pub id: EndpointId,
    /// 显示名称。
    pub name: String,
    /// 展示备注，不参与协议映射。
    #[serde(default)]
    pub note: String,
    /// 所有单元继承的通信角色。
    pub role: Role,
    /// 通信参数。
    pub transport: Transport,
    /// 独立模型引用的单元集合。
    pub units: Vec<UnitConfig>,
    /// 当前主站运行参数。
    #[serde(default)]
    pub master: crate::domain::master::MasterSettings,
}
impl EndpointConfig {
    /// 校验既存端点的角色和传输种类不变；名称与通信参数可编辑。
    /// # Errors
    /// 角色或TCP／RTU种类改变时拒绝，不修改任一配置。
    pub fn ensure_same_type(&self, next: &Self) -> Result<(), Error> {
        if self.role != next.role
            || std::mem::discriminant(&self.transport) != std::mem::discriminant(&next.transport)
        {
            return Err(
                Error::new("ENDPOINT_TYPE_IMMUTABLE", "创建后角色及传输类型不能修改").at_unit(
                    &self.id,
                    None,
                    "transport",
                ),
            );
        }
        Ok(())
    }

    /// 返回1–247中最小可用站号；耗尽时为空，不修改配置。
    pub fn next_station(&self) -> Option<u8> {
        (1..=247).find(|station| !self.units.iter().any(|unit| unit.station == *station))
    }
    /// 校验聚合内身份、站号和通信参数；不打开通道。
    /// # Errors
    /// 重复身份／站号、非法参数或RTU从站超过一个单元时失败。
    pub fn validate(&self) -> Result<(), Error> {
        if self.id.0.is_empty() || self.name.trim().is_empty() {
            return Err(Error::new("INVALID_ENDPOINT", "端点身份和名称不能为空")
                .at_unit(&self.id, None, "name"));
        }
        self.master.validate()?;
        match &self.transport {
            Transport::Tcp { address, port }
                if address.parse::<std::net::Ipv4Addr>().is_err() || *port == 0 =>
            {
                return Err(
                    Error::new("INVALID_TRANSPORT", "TCP须使用IPv4地址和非零端口").at_unit(
                        &self.id,
                        None,
                        "transport",
                    ),
                )
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
                return Err(
                    Error::new("INVALID_TRANSPORT", "串口路径或参数无效").at_unit(
                        &self.id,
                        None,
                        "transport",
                    ),
                )
            }
            _ => {}
        }
        if matches!(self.transport, Transport::Rtu { .. })
            && self.role == Role::Slave
            && self.units.len() > 1
        {
            return Err(
                Error::new("RTU_SINGLE_STATION", "首版RTU从站通道只允许一个单元")
                    .at_unit(&self.id, None, "units"),
            );
        }
        let mut stations = BTreeMap::new();
        let mut ids = BTreeSet::new();
        for unit in &self.units {
            let invalid = if unit.id.0.trim().is_empty() || !ids.insert(&unit.id) {
                Some(("DUPLICATE_ID", "单元身份为空或重复", "id"))
            } else if unit.name.trim().is_empty() {
                Some(("INVALID_UNIT", "单元名称不能为空", "name"))
            } else if !(1..=247).contains(&unit.station) {
                Some(("INVALID_UNIT", "站号须在1–247", "station"))
            } else {
                None
            };
            if let Some((code, message, field)) = invalid {
                return Err(Error::new(code, message).at_unit(&self.id, Some(&unit.id), field));
            }
            if let Some(other) = stations.insert(unit.station, &unit.id) {
                let mut error = Error::new("INVALID_UNIT", "单元站号重复").at_unit(
                    &self.id,
                    Some(&unit.id),
                    "station",
                );
                error.related_unit_id = Some(other.0.clone());
                return Err(error);
            }
        }
        Ok(())
    }
}
