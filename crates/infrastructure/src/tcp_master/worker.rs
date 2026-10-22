//! 单目标同步所有者：周期按块轮转，手动批次优先且串行。
use super::*;
use bridge::Connection;
use essim_core::{
    application::commands::master::MasterAction,
    domain::{
        endpoint::{Role, Transport},
        master::{plan, RequestBlock},
        model::{Area, CollectionModel},
        point_table::{decode, encode, RawValue},
    },
};
use std::sync::mpsc::Receiver;
fn request_error(error: std::io::Error) -> Error {
    if let Some(code) = libmodbus_rs::exception_code(&error) {
        let label = match code {
            1 => "非法功能（远端不支持）",
            2 => "非法数据地址",
            3 => "非法数据值",
            4 => "设备故障",
            5 => "已接受但未完成",
            6 => "设备忙",
            8 => "存储奇偶校验错误",
            10 => "网关路径不可用",
            11 => "网关目标无响应",
            _ => "设备异常",
        };
        Error::new("MODBUS_EXCEPTION", format!("Modbus异常 {code}：{label}"))
    } else {
        failure(error)
    }
}
pub(super) fn validate(s: &MasterSnapshot) -> Result<(), Error> {
    s.endpoint.validate()?;
    if s.endpoint.role != Role::Master {
        return Err(Error::new("MASTER_TYPE", "仅支持已应用主站"));
    }
    for u in s.endpoint.units.iter().filter(|u| u.enabled) {
        let model = s
            .values
            .get(&u.id)
            .and_then(|v| v.model.as_ref())
            .ok_or_else(|| Error::new("NOT_APPLIED", "单元模型尚未应用"))?;
        if model.points.is_empty(){return Err(Error::new("EMPTY_UNIT_MODEL","启用单元的有效点表为空，请补齐配置后应用"));}
        plan(model, &[], false)?;
    }
    Ok(())
}
struct Task {
    unit: UnitId,
    block: RequestBlock,
    due: Instant,
}
struct Engine {
    shared: Shared,
    snapshot: Option<MasterSnapshot>,
    connection: Option<Connection>,
    tasks: Vec<Task>,
    cursor: usize,
    next_connect: Instant,
    cancel: Arc<AtomicBool>,
    disconnect: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    root: std::path::PathBuf,
    recording: crate::recording::RecordingPool,
}
impl Engine {
    fn state(&self, state: &str) {
        if let Ok(mut p) = self.shared.lock() {
            p.view.state = state.into();
        }
        publish(
            &self.shared,
            MasterFact::State {
                state: state.into(),
            },
        );
    }
    fn error(&self, e: impl std::fmt::Display) {
        let message = e.to_string();
        if let Ok(mut p) = self.shared.lock() {
            p.view.error = Some(message.clone());
        }
        publish(&self.shared, MasterFact::Failure { message });
    }
    fn aborted(&self) -> bool {
        self.shared.lock().map(|p| p.record_failed).unwrap_or(true)
            || self.disconnect.load(Ordering::Acquire)
            || self.shutdown.load(Ordering::Acquire)
    }
    fn install(&mut self, s: MasterSnapshot) -> Result<(), Error> {
        validate(&s)?;
        let mut p = self.shared.lock().map_err(failure)?;
        let mut points = BTreeMap::new();
        let mut tasks = vec![];
        for u in s.endpoint.units.iter().filter(|u| u.enabled) {
            let model = s.values[&u.id].model.as_ref().unwrap();
            let old_model = self
                .snapshot
                .as_ref()
                .filter(|old| {
                    old.endpoint.transport == s.endpoint.transport
                        && old
                            .endpoint
                            .units
                            .iter()
                            .any(|x| x.id == u.id && x.station == u.station)
                })
                .and_then(|old| old.values.get(&u.id))
                .and_then(|v| v.model.as_ref());
            let mut values = BTreeMap::new();
            for point in &model.points {
                let old = old_model
                    .and_then(|m| m.points.iter().find(|x| x.id == point.id))
                    .filter(|x| {
                        x.area == point.area
                            && x.address == point.address
                            && x.value_type == point.value_type
                            && x.byte_order == point.byte_order
                            && x.word_order == point.word_order
                            && x.scale == point.scale
                    });
                let cached = old
                    .and_then(|_| p.points.get(&u.id)?.get(&point.id))
                    .cloned();
                let mut value = cached.unwrap_or_default();
                value.quality = if value.value.is_some() {
                    "stale"
                } else {
                    "unread"
                }
                .into();
                value.error = None;
                value.write_result = None;
                value.write_operation = None;
                values.insert(point.id.clone(), value);
            }
            points.insert(u.id.clone(), values);
            for block in plan(model, &[], false)? {
                tasks.push(Task {
                    unit: u.id.clone(),
                    block,
                    due: Instant::now(),
                });
            }
        }
        p.view.observations.clear();
        p.sequence = 0;
        p.view.revision = "0".into();
        p.view.polling = false;
        p.view.paused_units.clear();
        p.snapshot = Some(s.clone());
        p.points = points;
        p.view.record_path = None;
        p.journal = None;
        p.record_failed = false;
        drop(p);
        self.tasks = tasks;
        self.cursor = 0;
        self.snapshot = Some(s);
        Ok(())
    }
    fn ensure_journal(&self) -> Result<(), Error> {
        let s = self
            .snapshot
            .as_ref()
            .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用主站"))?;
        if self.shared.lock().map_err(failure)?.journal.is_none() {
            let cache = self.shared.lock().map_err(failure)?.points.clone();
            match super::journal::Journal::create(&self.root, s, &cache, &self.recording) {
                Ok(journal) => {
                    let mut p = self.shared.lock().map_err(failure)?;
                    p.view.record_path = Some(journal.path.clone());
                    p.journal = Some(journal);
                }
                Err(error) => {
                    self.state("failed");
                    return Err(error);
                }
            }
        }
        Ok(())
    }
    fn connect(&mut self) -> Result<(), Error> {
        self.connection.take();
        self.ensure_journal()?;
        let s = self
            .snapshot
            .as_ref()
            .ok_or_else(|| Error::new("NOT_APPLIED", "请先应用主站"))?;
        self.state("connecting");
        match Connection::open(&s.endpoint, self.shared.clone()) {
            Ok(c) => {
                self.connection = Some(c);
                if let Ok(mut p) = self.shared.lock() {
                    p.view.error = None;
                }
                self.state("connected");
                Ok(())
            }
            Err(e) => {
                self.state("failed");
                Err(request_error(e))
            }
        }
    }
    fn unit_model(&self, id: &UnitId) -> Result<(u8, Arc<CollectionModel>), Error> {
        let s = self
            .snapshot
            .as_ref()
            .ok_or_else(|| Error::new("NOT_APPLIED", "主站尚未连接"))?;
        let unit = s
            .endpoint
            .units
            .iter()
            .find(|u| u.id == *id && u.enabled)
            .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "启用单元不存在"))?;
        Ok((unit.station, s.values[id].model.clone().unwrap()))
    }
    fn lost(&mut self, e: &std::io::Error) {
        if libmodbus_rs::exception_code(e).is_none() {
            let is_rtu = self
                .snapshot
                .as_ref()
                .is_some_and(|s| matches!(s.endpoint.transport, Transport::Rtu { .. }));
            let recoverable = matches!(
                e.kind(),
                std::io::ErrorKind::TimedOut
                    | std::io::ErrorKind::WouldBlock
                    | std::io::ErrorKind::InvalidData
            ) || e.raw_os_error().is_some_and(|code| code >= 112345678);
            if is_rtu && recoverable {
                if let Some(c) = self.connection.as_mut() {
                    if c.recover_rtu(&self.disconnect, &self.shutdown).is_ok() {
                        return;
                    }
                }
            }

            self.connection.take();
            self.next_connect = Instant::now()
                + Duration::from_millis(
                    self.snapshot.as_ref().unwrap().endpoint.master.reconnect_ms,
                );
            let polling = self.shared.lock().map(|p| p.view.polling).unwrap_or(false);
            self.state(if polling {
                "reconnecting"
            } else {
                "disconnected"
            });
        }
    }
    fn read(
        &mut self,
        unit: &UnitId,
        model: &CollectionModel,
        b: &RequestBlock,
    ) -> Result<BTreeMap<PointId, RawValue>, Error> {
        let (station, _) = self.unit_model(unit)?;
        let c = self
            .connection
            .as_mut()
            .ok_or_else(|| Error::new("NOT_CONNECTED", "请先连接主站"))?;
        c.client().set_unit(station).map_err(failure)?;
        self.shared.lock().map_err(failure)?.view.requests += 1;
        let result = match b.area {
            Area::Coil => c
                .client()
                .read_coil(b.start, b.count)
                .map(|v| v.into_iter().map(u16::from).collect()),
            Area::Discrete => c
                .client()
                .read_discrete(b.start, b.count)
                .map(|v| v.into_iter().map(u16::from).collect()),
            Area::Holding => c.client().read_holding(b.start, b.count),
            Area::Input => c.client().read_input(b.start, b.count),
        };
        let result = match result {
            Ok(words) => (|| {
                let mut values = BTreeMap::new();
                for index in &b.points {
                    let p = &model.points[*index];
                    let offset = usize::from(p.address - b.start);
                    let raw = decode(
                        p.value_type,
                        &words[offset..offset + p.value_type.width() as usize],
                        p.byte_order,
                        p.word_order,
                    )?;
                    values.insert(p.id.clone(), raw);
                }
                Ok(values)
            })(),
            Err(e) => {
                self.lost(&e);
                Err(request_error(e))
            }
        };
        let mut projection = self.shared.lock().map_err(failure)?;
        if let Some(points) = projection.points.get_mut(unit) {
            for index in &b.points {
                let p = &model.points[*index];
                if let Some(value) = points.get_mut(&p.id) {
                    match &result {
                        Ok(values) => {
                            value.value = values.get(&p.id).cloned();
                            value.unix_ms = Some(now());
                            value.quality = "valid".into();
                            value.error = None;
                        }
                        Err(e) => {
                            value.quality = "failed".into();
                            value.error = Some(e.message.clone());
                        }
                    }
                }
            }
        }
        result
    }
    fn write_result(&self, unit: &UnitId, ids: impl IntoIterator<Item = PointId>, result: &str) {
        if let Ok(mut p) = self.shared.lock() {
            let operation = p.view.operation.clone();
            if let Some(points) = p.points.get_mut(unit) {
                for id in ids {
                    if let Some(point) = points.get_mut(&id) {
                        point.write_result = Some(result.into());
                        point.write_operation = operation.clone();
                    }
                }
            }
        }
    }
    fn manual(&mut self, command: &MasterCommand) -> Result<(), Error> {
        match &command.action {
            MasterAction::Connect => self.connect(),
            MasterAction::StartPolling => {
                if self.connection.is_none() {
                    return Err(Error::new("NOT_CONNECTED", "请先连接，再开始采集"));
                }
                self.shared.lock().map_err(failure)?.view.polling = true;
                for t in &mut self.tasks {
                    t.due = Instant::now();
                }
                Ok(())
            }
            MasterAction::SetUnitPolling { unit, enabled } => {
                self.unit_model(unit)?;
                let mut p = self.shared.lock().map_err(failure)?;
                p.view.paused_units.retain(|id| id != unit);
                if !enabled {
                    p.view.paused_units.push(unit.clone());
                }
                Ok(())
            }
            MasterAction::Diagnose {
                unit,
                function,
                sub,
                data,
            } => {
                essim_core::domain::master::validate_diagnostic(*function, *sub, *data)?;
                let (station, _) = self.unit_model(unit)?;
                if !matches!(
                    self.snapshot.as_ref().unwrap().endpoint.transport,
                    Transport::Rtu { .. }
                ) {
                    return Err(Error::new("DIAGNOSTIC_TRANSPORT", "串行诊断仅用于RTU主站"));
                }
                {
                    let mut p = self.shared.lock().map_err(failure)?;
                    p.view.diagnostic = None;
                    p.view.total = 1;
                    p.view.requests += 1;
                }
                let timeout = Duration::from_millis(
                    self.snapshot
                        .as_ref()
                        .unwrap()
                        .endpoint
                        .master
                        .request_timeout_ms,
                );
                let c = self
                    .connection
                    .as_mut()
                    .ok_or_else(|| Error::new("NOT_CONNECTED", "请先打开串口"))?;
                match c
                    .client()
                    .diagnose(station, *function, *sub, *data, timeout)
                {
                    Ok(bytes) => {
                        let mut p = self.shared.lock().map_err(failure)?;
                        p.view.diagnostic = Some(
                            bytes
                                .iter()
                                .map(|b| format!("{b:02X}"))
                                .collect::<Vec<_>>()
                                .join(" "),
                        );
                        p.view.completed = 1;
                        Ok(())
                    }
                    Err(e) => {
                        self.lost(&e);
                        Err(request_error(e))
                    }
                }
            }
            MasterAction::Read { unit, points } => {
                let (_, model) = self.unit_model(unit)?;
                let blocks = plan(&model, points, false)?;
                self.shared.lock().map_err(failure)?.view.total = blocks.len();
                for block in blocks {
                    if self.aborted() {
                        return Err(Error::new("CANCELLED", "读取剩余请求已取消"));
                    }
                    self.read(unit, &model, &block)?;
                    self.shared.lock().map_err(failure)?.view.completed += 1;
                }
                Ok(())
            }
            MasterAction::Write { unit, values } => {
                let (station, model) = self.unit_model(unit)?;
                let ids: Vec<_> = values.iter().map(|(id, _)| id.clone()).collect();
                self.write_result(unit, ids.clone(), "校验未通过，未发送");
                let blocks = plan(&model, &ids, true)?;
                // 整批编码校验先于任何网络写入；slaveWritable仅供从站使用。
                let mut raw = BTreeMap::new();
                for (id, value) in values {
                    let p = model.points.iter().find(|p| p.id == *id).unwrap();
                    encode(p.value_type, value, p.byte_order, p.word_order)
                        .map_err(|e| e.at_point(id, "value"))?;
                    raw.insert(id.clone(), value.clone());
                }
                if self.connection.is_none() {
                    return Err(Error::new("NOT_CONNECTED", "请先连接主站"));
                }
                self.shared.lock().map_err(failure)?.view.total = blocks.len();
                self.write_result(unit, ids.clone(), "未执行");
                for b in blocks {
                    if self.aborted() {
                        return Err(Error::new(
                            "CANCELLED",
                            "剩余写入已取消；已发出项请按执行结果核对",
                        ));
                    }
                    let block_ids: Vec<_> = b
                        .points
                        .iter()
                        .map(|i| model.points[*i].id.clone())
                        .collect();
                    let mut words = vec![];
                    for index in &b.points {
                        let p = &model.points[*index];
                        words.extend(encode(
                            p.value_type,
                            &raw[&p.id],
                            p.byte_order,
                            p.word_order,
                        )?);
                    }
                    let c = self
                        .connection
                        .as_mut()
                        .ok_or_else(|| Error::new("NOT_CONNECTED", "连接已断开"))?;
                    c.client().set_unit(station).map_err(failure)?;
                    self.shared.lock().map_err(failure)?.view.requests += 1;
                    let written = match b.area {
                        Area::Coil if b.count == 1 => c.client().write_coil(b.start, words[0] != 0),
                        Area::Coil => c.client().write_coils(
                            b.start,
                            &words.iter().map(|w| *w != 0).collect::<Vec<_>>(),
                        ),
                        _ if b.count == 1 => c.client().write_holding(b.start, words[0]),
                        _ => c.client().write_holdings(b.start, &words),
                    };
                    if let Err(e) = written {
                        let known = libmodbus_rs::exception_code(&e).is_some();
                        self.write_result(
                            unit,
                            block_ids,
                            if known {
                                "协议拒绝"
                            } else {
                                "结果不确定，禁止自动重发"
                            },
                        );
                        self.lost(&e);
                        return Err(request_error(e));
                    }
                    self.write_result(unit, block_ids.clone(), "协议已确认，待回验");
                    if self.aborted() {
                        self.write_result(unit, block_ids, "协议已确认，回验已取消");
                        return Err(Error::new("CANCELLED", "回验及剩余写入已取消"));
                    }
                    match self.read(unit, &model, &b) {
                        Ok(actual) => {
                            let consistent =
                                block_ids.iter().all(|id| actual.get(id) == raw.get(id));
                            for id in &block_ids {
                                self.write_result(
                                    unit,
                                    [id.clone()],
                                    if actual.get(id) == raw.get(id) {
                                        "协议已确认，回验一致"
                                    } else {
                                        "协议已确认，回验不一致"
                                    },
                                );
                            }
                            if !consistent {
                                return Err(Error::new(
                                    "READBACK_MISMATCH",
                                    "回验不一致，停止剩余写入",
                                ));
                            }
                        }
                        Err(e) => {
                            self.write_result(unit, block_ids, "协议已确认，回验失败");
                            return Err(e);
                        }
                    }
                    self.shared.lock().map_err(failure)?.view.completed += 1;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }
    fn quality(&self) {
        let Some(s) = &self.snapshot else {
            return;
        };
        let connected = self.connection.is_some();
        let stamp = now().parse::<u128>().unwrap_or(0);
        if let Ok(mut p) = self.shared.lock() {
            for (u, values) in &mut p.points {
                if let Some(model) = s.values.get(u).and_then(|v| v.model.as_ref()) {
                    for point in &model.points {
                        if let Some(value) = values.get_mut(&point.id) {
                            if value.quality == "failed" {
                                continue;
                            }
                            if !connected {
                                if value.value.is_some() {
                                    value.quality = "disconnected".into();
                                }
                                continue;
                            }
                            if value.quality == "disconnected" {
                                value.quality = "stale".into();
                            }
                            if value.quality == "valid"
                                && value
                                    .unix_ms
                                    .as_ref()
                                    .and_then(|t| t.parse::<u128>().ok())
                                    .is_some_and(|t| {
                                        stamp.saturating_sub(t)
                                            > u128::from(point.master_period_ms)
                                                * u128::from(s.endpoint.master.stale_multiplier)
                                    })
                            {
                                value.quality = "stale".into();
                            }
                        }
                    }
                }
            }
        }
    }
}
pub(super) fn run(
    rx: Receiver<Job>,
    shared: Shared,
    cancel: Arc<AtomicBool>,
    disconnect: Arc<AtomicBool>,
    shutdown: Arc<AtomicBool>,
    root: std::path::PathBuf,
    recording: crate::recording::RecordingPool,
) {
    let mut e = Engine {
        shared,
        snapshot: None,
        connection: None,
        tasks: vec![],
        cursor: 0,
        next_connect: Instant::now(),
        cancel,
        disconnect,
        shutdown,
        root,
        recording,
    };
    while !e.shutdown.load(Ordering::Acquire) {
        let record_failed = e.shared.lock().map(|p| p.record_failed).unwrap_or(true);
        if record_failed {
            e.connection.take();
            if let Ok(mut p) = e.shared.lock() {
                p.view.polling = false;
                p.view.state = "failed".into();
                p.active_charge.take();
            }
        }
        let disconnecting = e.disconnect.swap(false, Ordering::AcqRel);
        let cancelling = e.cancel.swap(false, Ordering::AcqRel);
        if disconnecting {
            e.connection.take();
            if let Ok(mut p) = e.shared.lock() {
                p.view.polling = false;
                p.record_failed = false;
                p.active_charge.take();
            }
            e.state("disconnected");
        }
        if cancelling {
            if let Ok(mut p) = e.shared.lock() {
                p.view.polling = false;
            }
        }
        super::release_completed(&e.shared);
        match rx.try_recv() {
            Ok(job) => {
                if let Ok(mut p) = e.shared.lock() {
                    p.view.error = None;
                }
                if let MasterAction::Write { unit, values } = &job.command.action {
                    e.write_result(unit, values.iter().map(|(id, _)| id.clone()), "未执行");
                }
                let result = if disconnecting
                    || (cancelling && matches!(job.command.action, MasterAction::StartPolling))
                {
                    Err(Error::new("CANCELLED", "未执行操作已取消"))
                } else {
                    (|| {
                        if let Some(s) = job.snapshot {
                            e.install(s)?;
                        }
                        if matches!(job.command.action, MasterAction::Connect) {
                            e.ensure_journal()?;
                        }
                        publish(
                            &e.shared,
                            MasterFact::Intent {
                                command: job.command.clone(),
                            },
                        );
                        if e.aborted() {
                            return Err(Error::new(
                                "CANCELLED",
                                "操作已取消或可靠记录失败，未发送剩余请求",
                            ));
                        }
                        e.manual(&job.command)
                    })()
                };
                if let Err(error) = &result {
                    e.error(&error.message);
                }
                publish(
                    &e.shared,
                    MasterFact::Operation {
                        command: job.command.command_id,
                        result: result
                            .as_ref()
                            .map(|_| "已完成".to_string())
                            .unwrap_or_else(|e| e.message.clone()),
                    },
                );
                if let Ok(mut p) = e.shared.lock() {
                    p.view.busy = false;
                    if disconnecting {
                        p.disconnect_pending = false;
                    }
                    if let Some(charge) = job.memory_charge {
                        charge.commit();
                        p.memory_charge = Some(charge);
                    }
                    if let Some(charge) = job.active_charge {
                        if e.connection.is_some() {
                            charge.commit();
                            p.active_charge = Some(charge);
                        }
                    }
                }
                drop(job.permit);
                super::release_completed(&e.shared);
                continue;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(_) => {}
        }
        if disconnecting {
            if let Ok(mut p) = e.shared.lock() {
                p.disconnect_pending = false;
            }
            super::release_completed(&e.shared);
        }
        e.quality();
        let polling = e.shared.lock().map(|p| p.view.polling).unwrap_or(false);
        if polling && e.connection.is_none() && Instant::now() >= e.next_connect {
            if let Err(error) = e.connect() {
                e.error(error.message);
                e.state("reconnecting");
            }
            e.next_connect = Instant::now()
                + Duration::from_millis(
                    e.snapshot
                        .as_ref()
                        .map_or(3000, |s| s.endpoint.master.reconnect_ms),
                );
        }
        if polling && e.connection.is_some() && !e.tasks.is_empty() {
            let len = e.tasks.len();
            let mut selected = None;
            for offset in 0..len {
                let i = (e.cursor + offset) % len;
                if e.tasks[i].due <= Instant::now()
                    && !e
                        .shared
                        .lock()
                        .map(|p| p.view.paused_units.contains(&e.tasks[i].unit))
                        .unwrap_or(true)
                {
                    selected = Some(i);
                    break;
                }
            }
            if let Some(i) = selected {
                let unit = e.tasks[i].unit.clone();
                let b = e.tasks[i].block.clone();
                if let Ok((_, model)) = e.unit_model(&unit) {
                    if let Err(error) = e.read(&unit, &model, &b) {
                        e.error(error.message);
                    }
                }
                e.tasks[i].due = Instant::now() + Duration::from_millis(b.period_ms);
                e.cursor = (i + 1) % len;
                continue;
            }
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    e.connection.take();
    e.state("disconnected");
    if let Ok(mut p) = e.shared.lock() {
        p.view.polling = false;
        p.view.busy = false;
        p.disconnect_pending = false;
        p.active_charge.take();
    };
    super::release_completed(&e.shared);
}
