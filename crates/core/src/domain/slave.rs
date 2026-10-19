//! 从站共享模型索引与无副作用的运行值修改规划，不持有C映射或连接。
use super::{
    error::Error,
    identity::PointId,
    model::{Area, CollectionModel},
    point_table::{encode, RawValue},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
/// 一次点值修改的区域、起始零基地址及编码结果。
pub type ValuePlan = Vec<(usize, u16, Vec<u16>)>;
/// 可跨Unit共享的不可变点定义及地址索引。
pub struct ModelLayout {
    /// 实际已应用模型，后续保存不得修改它。
    pub model: Arc<CollectionModel>,
    /// 每个数据区的地址到所属点下标；空洞不自动变成可访问点。
    pub addresses: [BTreeMap<u16, usize>; 4],
    /// 稳定点身份到点下标。
    pub points: BTreeMap<PointId, usize>,
}
/// 返回线圈、离散输入、保持寄存器、输入寄存器的内部区域下标。
pub fn area_index(area: Area) -> usize {
    match area {
        Area::Coil => 0,
        Area::Discrete => 1,
        Area::Holding => 2,
        Area::Input => 3,
    }
}
impl ModelLayout {
    /// 编译并校验模型，不分配各Unit运行值。
    /// # Errors
    /// 模型非法、地址重叠或越界时拒绝。
    pub fn new(model: CollectionModel) -> Result<Self, Error> {
        model.validate()?;
        let mut addresses: [BTreeMap<u16, usize>; 4] = Default::default();
        let mut points = BTreeMap::new();
        for (index, p) in model.points.iter().enumerate() {
            points.insert(p.id.clone(), index);
            for offset in 0..p.value_type.width() {
                addresses[area_index(p.area)].insert((u32::from(p.address) + offset) as u16, index);
            }
        }
        Ok(Self {
            model: Arc::new(model),
            addresses,
            points,
        })
    }
    /// 检查请求完整地址范围及外部写权限，返回触及的点下标。
    /// # Errors
    /// 数据区、数量、地址空洞或外部写权限非法时拒绝整条请求。
    pub fn check_span(
        &self,
        area: usize,
        start: u16,
        count: u16,
        write: bool,
    ) -> Result<Vec<usize>, Error> {
        if area >= 4 || count == 0 || u32::from(start) + u32::from(count) > 65536 {
            return Err(Error::new("ILLEGAL_ADDRESS", "地址范围非法"));
        }
        let mut points = BTreeSet::new();
        for offset in 0..count {
            let index = *self.addresses[area]
                .get(&(start + offset))
                .ok_or_else(|| Error::new("ILLEGAL_ADDRESS", "请求包含未定义地址"))?;
            if write && (!self.model.points[index].slave_writable || area == 1 || area == 3) {
                return Err(Error::new("WRITE_DENIED", "点不允许外部写入"));
            }
            points.insert(index);
        }
        Ok(points.into_iter().collect())
    }
    /// 先验证整批本地点值，再返回编码规划；不修改任何运行值。
    /// # Errors
    /// 空批次、未知／重复点或类型越界时拒绝整批，并携带错误点身份。
    pub fn plan_values(&self, values: &[(PointId, RawValue)]) -> Result<ValuePlan, Error> {
        if values.is_empty() {
            return Err(Error::new("EMPTY_BATCH", "请选择需要修改的点"));
        }
        let mut seen = BTreeSet::new();
        values
            .iter()
            .map(|(id, raw)| {
                if !seen.insert(id) {
                    return Err(Error::new("DUPLICATE_POINT_ID", "批量修改包含重复点")
                        .at_point(id, "value"));
                }
                let index = self.points.get(id).ok_or_else(|| {
                    Error::new("POINT_NOT_FOUND", "点不在已应用模型中").at_point(id, "value")
                })?;
                let p = &self.model.points[*index];
                let words = encode(p.value_type, raw, p.byte_order, p.word_order)
                    .map_err(|e| e.at_point(id, "value"))?;
                Ok((area_index(p.area), p.address, words))
            })
            .collect()
    }
}
