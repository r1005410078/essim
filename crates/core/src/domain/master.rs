//! 主站请求规划与运行参数，不持有连接或窗口。
use super::{
    error::Error,
    identity::PointId,
    model::{Area, CollectionModel},
    slave::area_index,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
/// 当前端点主站参数，时间单位均为毫秒，不是配置历史版本。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MasterSettings {
    /// TCP连接等待上限。
    pub connect_timeout_ms: u64,
    /// libmodbus响应等待上限。
    pub request_timeout_ms: u64,
    /// 采集中断线后的重连间隔。
    pub reconnect_ms: u64,
    /// 相对于点采集周期的过期倍数。
    pub stale_multiplier: u64,
}
impl Default for MasterSettings {
    fn default() -> Self {
        Self {
            connect_timeout_ms: 3000,
            request_timeout_ms: 1000,
            reconnect_ms: 3000,
            stale_multiplier: 3,
        }
    }
}
impl MasterSettings {
    /// 校验等待10～30000ms、重连100～60000ms、过期倍数1～100。
    /// # Errors
    /// 任一参数越界时拒绝。
    pub fn validate(&self) -> Result<(), Error> {
        if !(10..=30000).contains(&self.connect_timeout_ms)
            || !(10..=30000).contains(&self.request_timeout_ms)
            || !(100..=60000).contains(&self.reconnect_ms)
            || !(1..=100).contains(&self.stale_multiplier)
        {
            return Err(Error::new(
                "INVALID_MASTER_SETTINGS",
                "连接／请求超时须10～30000ms，重连100～60000ms，过期倍数1～100",
            ));
        }
        Ok(())
    }
}
/// 一次完整点组成的协议读取／写入块。
#[derive(Debug, Clone)]
pub struct RequestBlock {
    /// 数据区。
    pub area: Area,
    /// 起始零基地址。
    pub start: u16,
    /// 位或寄存器数量。
    pub count: u16,
    /// 相同周期（手动操作为0）。
    pub period_ms: u64,
    /// 模型内点索引；不使用表格行作为身份。
    pub points: Vec<usize>,
}
/// 合并连续完整点；空选择表示读取全部，写入必须明确选点。
/// # Errors
/// 模型非法、未知／重复点、空写入或写只读数据区时拒绝。
pub fn plan(
    model: &CollectionModel,
    selected: &[PointId],
    write: bool,
) -> Result<Vec<RequestBlock>, Error> {
    model.validate()?;
    let ids: BTreeSet<_> = selected.iter().collect();
    if ids.len() != selected.len()
        || ids
            .iter()
            .any(|id| !model.points.iter().any(|p| &p.id == *id))
    {
        return Err(Error::new("INVALID_SELECTION", "选点包含未知或重复身份"));
    }
    if write && selected.is_empty() {
        return Err(Error::new("EMPTY_BATCH", "没有待写点"));
    }
    let manual = !selected.is_empty() || write;
    let mut points: Vec<_> = model
        .points
        .iter()
        .enumerate()
        .filter(|(_, p)| selected.is_empty() || ids.contains(&p.id))
        .collect();
    points.sort_by_key(|(_, p)| {
        (
            area_index(p.area),
            if manual { 0 } else { p.master_period_ms },
            p.address,
        )
    });
    let mut blocks: Vec<RequestBlock> = vec![];
    for (index, p) in points {
        if write && matches!(p.area, Area::Input | Area::Discrete) {
            return Err(
                Error::new("WRITE_DENIED", "主站只能写coil／holding").at_point(&p.id, "area")
            );
        }
        let max = match (p.area, write) {
            (Area::Coil | Area::Discrete, false) => 2000,
            (Area::Coil, true) => 1968,
            (_, false) => 125,
            _ => 123,
        };
        let width = p.value_type.width() as u16;
        let period = if manual { 0 } else { p.master_period_ms };
        if let Some(last) = blocks.last_mut() {
            if last.area == p.area
                && last.period_ms == u64::from(period)
                && u32::from(last.start) + u32::from(last.count) == u32::from(p.address)
                && last.count + width <= max
            {
                last.count += width;
                last.points.push(index);
                continue;
            }
        }
        blocks.push(RequestBlock {
            area: p.area,
            start: p.address,
            count: width,
            period_ms: u64::from(period),
            points: vec![index],
        });
    }
    Ok(blocks)
}
