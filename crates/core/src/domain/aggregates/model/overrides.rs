//! Unit 所有的单层稀疏覆盖；合成不修改基础模型或运行值。
use super::{Area, CollectionModel, PointDefinition, Telemetry};
use crate::domain::{
    error::Error,
    identity::PointId,
    point_table::{Order, ValueType, from_engineering},
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

macro_rules! fields {
    ($($name:ident: $ty:ty => $variant:ident),* $(,)?) => {
        /// 可以选择回写或恢复的点字段；初值以工程文本为唯一输入。
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        #[serde(rename_all="camelCase")]
        pub enum OverrideField { $(#[doc=concat!("点定义字段 `",stringify!($name),"`。")] $variant),* }
        /// 缺省字段继承基础值；显式空字符串及 false 均保留。
        #[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
        #[serde(rename_all="camelCase", deny_unknown_fields)]
        pub struct PointOverride { $(#[doc=concat!("覆盖 `",stringify!($name),"`；None 表示继承。")] #[serde(default,skip_serializing_if="Option::is_none")] pub $name: Option<$ty>),* }
        impl PointOverride {
            /// 返回已显式覆盖的字段。
            pub fn fields(&self)->Vec<OverrideField>{let mut result=Vec::new();$(if self.$name.is_some(){result.push(OverrideField::$variant);})*result}
            /// 取走选定字段作为补丁，其他覆盖保持不变。
            pub fn take(&mut self, fields:&BTreeSet<OverrideField>)->Self {let mut selected=Self::default();$(if fields.contains(&OverrideField::$variant){selected.$name=self.$name.take();})*selected}
            /// 将字段叠加到点定义并按工程初值重算原始编码。
            /// # Errors
            /// 类型或倍率无法表达初值时失败；不会修改输入点。
            pub fn apply(&self, base:&PointDefinition)->Result<PointDefinition,Error>{
                let mut p=base.clone();$(if let Some(value)=&self.$name{p.$name=value.clone();})*
                p.initial=from_engineering(p.value_type,&p.initial_engineering,&p.scale).map_err(|e|e.at_point(&p.id,"initialEngineering"))?;
                p.validate()?;Ok(p)
            }
        }
    }
}
fields! {
 name:String=>Name, note:String=>Note, unit:String=>Unit,
 category:Telemetry=>Category, area:Area=>Area, address:u16=>Address,
 value_type:ValueType=>ValueType, scale:String=>Scale,
 byte_order:Order=>ByteOrder, word_order:Order=>WordOrder,
 initial_engineering:String=>InitialEngineering,
 slave_writable:bool=>SlaveWritable, master_period_ms:u32=>MasterPeriodMs,
}
/// 单个 Unit 的配置差异，不含实时值或父模型链。
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UnitOverrides {
    /// 基础点的稀疏字段覆盖。
    #[serde(default)]
    pub point_overrides: BTreeMap<PointId, PointOverride>,
    /// Unit 局部新增点，身份不与基础点重合。
    #[serde(default)]
    pub added_points: Vec<PointDefinition>,
    /// 当前 Unit 隐藏的基础点。
    #[serde(default)]
    pub removed_point_ids: BTreeSet<PointId>,
}
impl UnitOverrides {
    /// 是否完全继承模型。
    pub fn is_empty(&self) -> bool {
        self.point_overrides.is_empty()
            && self.added_points.is_empty()
            && self.removed_point_ids.is_empty()
    }
    /// 合成当前有效模型并执行完整地址、数值及角色字段校验。
    /// # Errors
    /// 失效点引用、重复身份、互斥覆盖或合成点表非法时失败。
    pub fn resolve(&self, base: &CollectionModel) -> Result<CollectionModel, Error> {
        let ids: BTreeSet<_> = base.points.iter().map(|p| &p.id).collect();
        for id in self.point_overrides.keys().chain(&self.removed_point_ids) {
            if !ids.contains(id) {
                return Err(Error::new(
                    "OVERRIDE_POINT_MISSING",
                    "覆盖引用的基础点已不存在，请先恢复覆盖或另存模型",
                )
                .at_point(id, "overrides"));
            }
        }
        for id in self.point_overrides.keys() {
            if self.removed_point_ids.contains(id) {
                return Err(
                    Error::new("OVERRIDE_CONFLICT", "隐藏点不能同时保留字段覆盖")
                        .at_point(id, "overrides"),
                );
            }
        }
        let mut model = CollectionModel {
            id: base.id.clone(),
            name: base.name.clone(),
            points: Vec::with_capacity(base.points.len() + self.added_points.len()),
        };
        for point in &base.points {
            if self.removed_point_ids.contains(&point.id) {
                continue;
            }
            model
                .points
                .push(match self.point_overrides.get(&point.id) {
                    Some(patch) => patch.apply(point)?,
                    None => point.clone(),
                });
        }
        for p in &self.added_points {
            if ids.contains(&p.id) {
                return Err(Error::new("DUPLICATE_POINT_ID", "局部点身份与基础点重复")
                    .at_point(&p.id, "id"));
            }
            model.points.push(p.clone());
        }
        model.validate()?;
        Ok(model)
    }
}
