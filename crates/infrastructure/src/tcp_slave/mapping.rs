//! 线程内C映射与共享模型之间的适配；每Unit拥有独立的紧凑四区值。
use super::failure;
use essim_core::domain::{
    error::Error,
    identity::PointId,
    point_table::{decode, RawValue},
    slave::{area_index, ModelLayout, ValuePlan},
    values::UnitValues,
};
use libmodbus_rs::{AddressRange, Exception, Mapping};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
pub(super) struct UnitMapping {
    pub layout: Arc<ModelLayout>,
    pub mapping: Mapping,
}
impl UnitMapping {
    pub fn new(layout: Arc<ModelLayout>, values: &UnitValues) -> Result<Self, Error> {
        let ranges: Vec<_> = layout
            .addresses
            .iter()
            .map(
                |area| match (area.first_key_value(), area.last_key_value()) {
                    (Some((&start, _)), Some((&end, _))) => {
                        AddressRange::new(start, u32::from(end) - u32::from(start) + 1).map(Some)
                    }
                    _ => Ok(None),
                },
            )
            .collect::<Result<_, _>>()
            .map_err(failure)?;
        let mapping = Mapping::new(ranges[0], ranges[1], ranges[2], ranges[3]).map_err(failure)?;
        let mut unit = Self { layout, mapping };
        for p in &unit.layout.model.points {
            let raw = values.raw.get(&p.id).unwrap_or(&p.initial);
            let plan = unit.layout.plan_values(&[(p.id.clone(), raw.clone())])?;
            Self::apply_to(&mut unit.mapping, &plan)?;
        }
        Ok(unit)
    }
    pub fn read_word(&self, area: usize, address: u16) -> Result<u16, Error> {
        match area {
            0 => self.mapping.coil(address).map(u16::from),
            1 => self.mapping.discrete(address).map(u16::from),
            2 => self.mapping.holding(address),
            _ => self.mapping.input(address),
        }
        .map_err(failure)
    }
    pub fn apply_to(mapping: &mut Mapping, plan: &ValuePlan) -> Result<(), Error> {
        for (area, start, words) in plan {
            for (offset, value) in words.iter().enumerate() {
                let address = *start + offset as u16;
                match area {
                    0 => mapping.set_coil(address, *value != 0),
                    1 => mapping.set_discrete(address, *value != 0),
                    2 => mapping.set_holding(address, *value),
                    _ => mapping.set_input(address, *value),
                }
                .map_err(failure)?;
            }
        }
        Ok(())
    }
    pub fn read_point(&self, id: &PointId) -> Result<RawValue, Error> {
        let i = self.layout.points.get(id).ok_or_else(|| {
            Error::new("POINT_NOT_FOUND", "点不在已应用模型中").at_point(id, "value")
        })?;
        let p = &self.layout.model.points[*i];
        let words = (0..p.value_type.width())
            .map(|offset| self.read_word(area_index(p.area), p.address + offset as u16))
            .collect::<Result<Vec<_>, _>>()?;
        decode(p.value_type, &words, p.byte_order, p.word_order)
    }
    pub fn raw_values(&self) -> Result<BTreeMap<PointId, RawValue>, Error> {
        self.layout
            .points
            .keys()
            .map(|id| Ok((id.clone(), self.read_point(id)?)))
            .collect()
    }
    pub fn values(&self) -> Result<UnitValues, Error> {
        Ok(UnitValues {
            model: Some(self.layout.model.clone()),
            raw: self.raw_values()?,
            stale: false,
        })
    }
    /// 只检查支持的8个功能码；参数非法在调用C前拒绝，避免库的同步flush路径。
    pub fn check_request(&self, req: &[u8]) -> Result<(), Exception> {
        self.check_pdu(&req[7..])
    }
    pub fn check_pdu(&self, req: &[u8]) -> Result<(), Exception> {
        if req.is_empty() {
            return Err(Exception::IllegalValue);
        }
        let function = req[0];
        if !matches!(function, 1..=6 | 15 | 16) {
            return Err(Exception::IllegalFunction);
        }
        if req.len() < 5 {
            return Err(Exception::IllegalValue);
        }
        let address = u16::from_be_bytes([req[1], req[2]]);
        let n = u16::from_be_bytes([req[3], req[4]]);
        let (area, count, write) = match function {
            1 => (0, n, false),
            2 => (1, n, false),
            3 => (2, n, false),
            4 => (3, n, false),
            5 => (0, 1, true),
            6 => (2, 1, true),
            15 => (0, n, true),
            16 => (2, n, true),
            _ => unreachable!(),
        };
        let valid = match function {
            1 | 2 => req.len() == 5 && (1..=2000).contains(&n),
            3 | 4 => req.len() == 5 && (1..=125).contains(&n),
            5 => req.len() == 5 && (n == 0 || n == 0xff00),
            6 => req.len() == 5,
            15 => {
                req.len() >= 6
                    && (1..=1968).contains(&n)
                    && usize::from(req[5]) == (usize::from(n) + 7) / 8
                    && req.len() == 6 + usize::from(req[5])
            }
            16 => {
                req.len() >= 6
                    && (1..=123).contains(&n)
                    && usize::from(req[5]) == usize::from(n) * 2
                    && req.len() == 6 + usize::from(req[5])
            }
            _ => false,
        };
        if !valid {
            return Err(Exception::IllegalValue);
        }
        let points = self
            .layout
            .check_span(area, address, count, write)
            .map_err(|_| Exception::IllegalAddress)?;
        if write {
            // 先解码拟提交后的完整点值，非法浮点或类型拒绝时C映射仍未修改。
            let incoming: Vec<u16> = match function {
                5 => vec![u16::from(n != 0)],
                6 => vec![n],
                15 => (0..count)
                    .map(|i| u16::from(req[6 + usize::from(i) / 8] & (1 << (i % 8)) != 0))
                    .collect(),
                16 => req[6..]
                    .chunks_exact(2)
                    .map(|v| u16::from_be_bytes([v[0], v[1]]))
                    .collect(),
                _ => vec![],
            };
            for index in points {
                let p = &self.layout.model.points[index];
                let mut words = Vec::new();
                for offset in 0..p.value_type.width() {
                    let a = p.address + offset as u16;
                    words.push(
                        if a >= address && u32::from(a) < u32::from(address) + u32::from(count) {
                            incoming[usize::from(a - address)]
                        } else {
                            self.read_word(area, a)
                                .map_err(|_| Exception::ServerFailure)?
                        },
                    );
                }
                decode(p.value_type, &words, p.byte_order, p.word_order)
                    .map_err(|_| Exception::IllegalValue)?;
            }
        }
        Ok(())
    }
    pub fn reset_plan(
        &self,
        ids: &[PointId],
    ) -> Result<(Vec<(PointId, RawValue)>, ValuePlan), Error> {
        let selected: BTreeSet<_> = if ids.is_empty() {
            self.layout.points.keys().cloned().collect()
        } else {
            ids.iter().cloned().collect()
        };
        let values = selected
            .into_iter()
            .map(|id| {
                let index = self.layout.points.get(&id).ok_or_else(|| {
                    Error::new("POINT_NOT_FOUND", "点不在已应用模型中").at_point(&id, "value")
                })?;
                Ok((id, self.layout.model.points[*index].initial.clone()))
            })
            .collect::<Result<Vec<_>, Error>>()?;
        let plan = self.layout.plan_values(&values)?;
        Ok((values, plan))
    }
}
