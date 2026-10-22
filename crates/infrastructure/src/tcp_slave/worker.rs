//! 固定线程内的监听、C映射、运行事实和非阻塞客户端所有权。
#[path = "serial.rs"]
mod serial;
use super::{failure, journal::Journal, mapping::UnitMapping, Message, Reply, Request, Snapshot};
use essim_core::{
    application::{
        commands::slave::{SlaveAction, SlaveCommand},
        events::slave::SlaveFact,
        queries::slave::{SlaveObservation, SlaveQuery, SlaveState, SlaveView},
    },
    domain::{
        endpoint::{Role, Transport},
        error::Error,
        identity::{EndpointId, UnitId},
        slave::ModelLayout,
    },
};
use libmodbus_rs::{Exception, Mapping, TcpResponder};
use mio::{
    net::{TcpListener, TcpStream},
    Events, Interest, Poll, Token,
};
use std::{
    collections::{BTreeMap, BTreeSet, HashMap, VecDeque},
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Receiver,
        Arc, Weak,
    },
    time::{Duration, Instant},
};
const BUFFER: usize = 16 * 1024;
struct Endpoint {
    config: essim_core::domain::endpoint::EndpointConfig,
    cache: Cache,
    _memory: crate::resources::Reservation,
    active: Option<crate::resources::Reservation>,
    units: BTreeMap<u8, (UnitId, UnitMapping)>,
    listener: Option<TcpListener>,
    serial: Option<serial::SerialState>,
    token: Token,
    journal: Option<Journal>,
    view: SlaveView,
    clients: BTreeSet<usize>,
}
struct Client {
    socket: TcpStream,
    endpoint: EndpointId,
    input: Vec<u8>,
    output: VecDeque<(Vec<u8>, usize)>,
    last_progress: Instant,
    frame_started: Option<Instant>,
}
type Models = std::rc::Rc<std::cell::RefCell<Vec<Weak<ModelLayout>>>>;
struct Owner {
    poll: Poll,
    cache: Cache,
    recording: crate::recording::RecordingPool,
    resources: crate::resources::ResourceManager,
    endpoints: BTreeMap<EndpointId, Endpoint>,
    listeners: HashMap<usize, EndpointId>,
    clients: HashMap<usize, Client>,
    next: usize,
    models: Models,
    global_clients: Arc<std::sync::atomic::AtomicUsize>,
    responder: TcpResponder,
    empty: Mapping,
    root: PathBuf,
}
pub(super) struct Cached {
    pub view: SlaveView,
    pub values: BTreeMap<UnitId, essim_core::domain::values::UnitValues>,
}
pub(super) type Cache = Arc<std::sync::Mutex<BTreeMap<EndpointId, Cached>>>;
fn publish_cache_parts(cache: &Cache, id: &EndpointId, view: &SlaveView) {
    if let Ok(mut c) = cache.lock() {
        if let Some(entry) = c.get_mut(id) {
            entry.view = view.clone();
        }
    }
}
fn publish_cache(ep: &Endpoint) {
    publish_cache_parts(&ep.cache, &ep.config.id, &ep.view);
}
fn refresh_cached_unit(
    ep: &mut Endpoint,
    id: &UnitId,
    points: Option<Vec<essim_core::domain::identity::PointId>>,
) -> Result<(), Error> {
    let map = &ep
        .units
        .values()
        .find(|(unit, _)| unit == id)
        .ok_or_else(|| failure("单元不存在"))?
        .1;
    let updates=if let Some(points)=points {points.into_iter().map(|id|Ok((id.clone(),map.read_point(&id)?))).collect::<Result<BTreeMap<_,_>,Error>>()?}else{map.raw_values()?};
    let mut cache=ep.cache.lock().map_err(|_|failure("查询缓存不可用"))?;
    if let Some(values)=cache.get_mut(&ep.config.id).and_then(|c|c.values.get_mut(id)){values.raw.extend(updates);}

    Ok(())
}
fn refresh_cached_request(ep: &mut Endpoint, station: u8, pdu: &[u8]) -> Result<(), Error> {
    if pdu.len() < 5 || !matches!(pdu[0], 5 | 6 | 15 | 16) {
        return Ok(());
    }
    let Some((id, map)) = ep.units.get(&station) else {
        return Ok(());
    };
    if map.check_pdu(pdu).is_err() {
        return Ok(());
    }
    let area = if matches!(pdu[0], 5 | 15) { 0 } else { 2 };
    let start = u16::from_be_bytes([pdu[1], pdu[2]]);
    let count = if matches!(pdu[0], 5 | 6) {
        1
    } else {
        u16::from_be_bytes([pdu[3], pdu[4]])
    };
    let points = map
        .layout
        .check_span(area, start, count, true)?
        .into_iter()
        .map(|i| map.layout.model.points[i].id.clone())
        .collect();
    let id=id.clone();
    refresh_cached_unit(ep, &id, Some(points))
}
pub(super) fn cached_query(cache: &Cache, query: SlaveQuery) -> Result<SlaveView, Error> {
    if query.points.len() > 1024 {
        return Err(Error::new("QUERY_TOO_LARGE", "单次最多1024点"));
    }
    let c = cache.lock().map_err(|_| failure("查询缓存不可用"))?;
    let Some(ep) = c.get(&query.endpoint) else {
        return Ok(idle(query.endpoint));
    };
    let mut view = ep.view.clone();
    if let Some(unit) = query.unit {
        let values = ep
            .values
            .get(&unit)
            .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "Unit未应用"))?;
        for id in query.points {
            let value = values
                .raw
                .get(&id)
                .ok_or_else(|| Error::new("POINT_NOT_FOUND", "点未应用"))?;
            view.values.insert(id, value.clone());
        }
    }
    Ok(view)
}
#[cfg(test)]
fn test_block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    let mut future = std::pin::pin!(future);
    let mut cx = std::task::Context::from_waker(std::task::Waker::noop());
    loop {
        match future.as_mut().poll(&mut cx) {
            std::task::Poll::Ready(v) => return v,
            std::task::Poll::Pending => std::thread::sleep(Duration::from_millis(1)),
        }
    }
}
fn idle(id: EndpointId) -> SlaveView {
    SlaveView {
        pending_recording: false,
        endpoint: id,
        state: SlaveState::Idle,
        revision: "0".into(),
        clients: 0,
        max_clients: 10,
        requests: "0".into(),
        exceptions: "0".into(),
        last_request_micros: "0".into(),
        error: None,
        record_path: None,
        last_reliable: "0".into(),
        uncertain: false,
        events: vec![],
        observations: vec![],
        values: BTreeMap::new(),
    }
}
fn revise(ep: &mut Endpoint) {
    ep.view.revision = (ep
        .view
        .revision
        .parse::<u64>()
        .unwrap_or(0)
        .saturating_add(1))
    .to_string();
    publish_cache(ep);
}
async fn record(ep: &mut Endpoint, command: Option<&str>, fact: SlaveFact) -> Result<(), Error> {
    let journal = ep
        .journal
        .as_mut()
        .ok_or_else(|| Error::new("RECORDING_FAILED", "运行记录未建立"))?;
    ep.view.pending_recording = true;
    publish_cache_parts(&ep.cache, &ep.config.id, &ep.view);
    let result = journal.record(command, &fact).await;
    ep.view.pending_recording = false;
    if let Err(e) = result {
        ep.view.state = SlaveState::Failed;
        ep.view.error = Some(e.message.clone());
        ep.view.uncertain = true;
        revise(ep);
        return Err(e);
    }
    ep.view.last_reliable = journal.sequence.to_string();
    let mut value = match &fact {
        SlaveFact::ValuesIntent { unit, .. } => {
            serde_json::json!({"kind":"valuesIntent","unit":unit})
        }
        _ => serde_json::to_value(&fact).map_err(failure)?,
    };
    if let Some(object) = value.as_object_mut() {
        object.remove("values");
    }
    ep.view.observations.push(SlaveObservation {
        sequence: journal.sequence.to_string(),
        unix_ms: journal.unix_ms.clone(),
        event: value.to_string(),
    });
    if ep.view.observations.len() > 100 {
        ep.view.observations.remove(0);
    }
    ep.view.events.push(value.to_string());
    if ep.view.events.len() > 100 {
        ep.view.events.remove(0);
    }
    revise(ep);
    Ok(())
}
pub(super) fn run(
    mut poll: Poll,
    rx: Receiver<Message>,
    root: PathBuf,
    shutdown: Arc<AtomicBool>,
    global_clients: Arc<std::sync::atomic::AtomicUsize>,
    ready: std::sync::mpsc::SyncSender<Result<(), Error>>,
    resources: crate::resources::ResourceManager,
    recording: crate::recording::RecordingPool,
    cache: Cache,
) {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll as TaskPoll, Waker},
    };
    struct Task {
        tx: std::sync::mpsc::SyncSender<Message>,
        future: Pin<Box<dyn Future<Output = ()>>>,
    }
    let mut tasks: BTreeMap<EndpointId, Task> = BTreeMap::new();
    let models: Models = Default::default();
    let waker = Waker::noop();
    let mut cx = Context::from_waker(waker);
    let mut events = Events::with_capacity(1);
    let _ = ready.send(Ok(()));
    loop {
        if !shutdown.load(Ordering::Acquire) {
            for _ in 0..64 {
                let message = match rx.try_recv() {
                    Ok(m) => m,
                    Err(std::sync::mpsc::TryRecvError::Empty) => break,
                    Err(_) => {
                        shutdown.store(true, Ordering::Release);
                        break;
                    }
                };
                let id = message.endpoint.clone();
                if !tasks.contains_key(&id) {
                    if tasks.len() >= resources.policy().batch_targets {
                        let _ = message.reply.send(Err(Error::new(
                            "ENDPOINT_OWNER_LIMIT",
                            "端点所有者达到工程上限",
                        )));
                        continue;
                    }
                    let owner = (|| {
                        Ok::<_, Error>(Owner {
                            poll: Poll::new().map_err(failure)?,
                            cache: cache.clone(),
                            recording: recording.clone(),
                            resources: resources.clone(),
                            endpoints: BTreeMap::new(),
                            listeners: HashMap::new(),
                            clients: HashMap::new(),
                            next: 1,
                            models: models.clone(),
                            global_clients: global_clients.clone(),
                            responder: TcpResponder::new().map_err(failure)?,
                            empty: Mapping::new(None, None, None, None).map_err(failure)?,
                            root: root.clone(),
                        })
                    })();
                    let owner = match owner {
                        Ok(o) => o,
                        Err(e) => {
                            let _ = message.reply.send(Err(e));
                            continue;
                        }
                    };
                    let (tx, queue) = std::sync::mpsc::sync_channel(256);
                    tasks.insert(
                        id.clone(),
                        Task {
                            tx,
                            future: Box::pin(run_endpoint(owner, queue, shutdown.clone())),
                        },
                    );
                }
                if let Err(e) = tasks.get(&id).unwrap().tx.try_send(message) {
                    let message = match e {
                        std::sync::mpsc::TrySendError::Full(m)
                        | std::sync::mpsc::TrySendError::Disconnected(m) => m,
                    };
                    let _ = message.reply.send(Err(Error::new(
                        "RUNTIME_BUSY",
                        "端点队列已满或所有者已结束",
                    )));
                }
            }
        }
        let mut completed = Vec::new();
        for (id, task) in &mut tasks {
            if let TaskPoll::Ready(()) = task.future.as_mut().poll(&mut cx) {
                completed.push(id.clone());
            }
        }
        for id in completed {
            tasks.remove(&id);
        }
        if shutdown.load(Ordering::Acquire) && tasks.is_empty() {
            break;
        }
        let _ = poll.poll(&mut events, Some(Duration::from_millis(1)));
    }
}
async fn yield_owner() {
    let mut yielded = false;
    std::future::poll_fn(move |_| {
        if yielded {
            std::task::Poll::Ready(())
        } else {
            yielded = true;
            std::task::Poll::Pending
        }
    })
    .await
}
async fn run_endpoint(mut owner: Owner, rx: Receiver<Message>, shutdown: Arc<AtomicBool>) {
    let mut events = Events::with_capacity(256);
    while !shutdown.load(Ordering::Acquire) {
        for _ in 0..8 {
            match rx.try_recv() {
                Ok(message) => {
                    let result = owner.handle(message.request).await;
                    drop(message.permit);
                    let _ = message.reply.send(result);
                }
                Err(_) => break,
            }
        }
        if owner.poll.poll(&mut events, Some(Duration::ZERO)).is_err() {
            break;
        }
        let tokens: Vec<_> = events.iter().map(|event| event.token().0).collect();
        for token in tokens {
            if owner.listeners.contains_key(&token) {
                owner.accept(token).await;
            } else {
                owner.client(token).await;
            }
        }
        let pending: Vec<_> = owner
            .clients
            .iter()
            .filter(|(_, c)| !c.input.is_empty() || !c.output.is_empty())
            .map(|(t, _)| *t)
            .collect();
        for token in pending {
            owner.client(token).await;
        }
        let ids: Vec<_> = owner
            .endpoints
            .iter()
            .filter(|(_, ep)| ep.serial.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in ids {
            owner.serial(&id).await;
        }
        let failed: Vec<_> = owner
            .endpoints
            .iter()
            .filter(|(_, ep)| {
                ep.view.state == SlaveState::Failed
                    && (ep.listener.is_some() || ep.serial.is_some())
            })
            .map(|(id, _)| id.clone())
            .collect();
        for id in failed {
            owner.close_resources(&id);
        }
        yield_owner().await;
    }
    for id in owner.endpoints.keys().cloned().collect::<Vec<_>>() {
        let _ = owner.stop(&id, "application-exit").await;
    }
}
impl Owner {
    fn token(&mut self) -> Token {
        let token = Token(self.next);
        self.next = self.next.checked_add(1).expect("运行token耗尽");
        token
    }
    async fn handle(&mut self, request: Request) -> Result<Reply, Error> {
        match request {
            Request::Start(snapshot, max, command) => {
                self.start(snapshot, max, &command).await.map(Reply::View)
            }
            Request::Stop(id, command) => self.stop(&id, &command).await.map(Reply::View),
            Request::Values(command) => self.values(command).await.map(Reply::View),
            Request::Forget(id) => {
                if self
                    .endpoints
                    .get(&id)
                    .is_some_and(|ep| ep.listener.is_some() || ep.serial.is_some())
                {
                    return Err(Error::new("STILL_RUNNING", "端点资源未释放"));
                }
                self.endpoints.remove(&id);
                Ok(Reply::Snapshot(None))
            }
            Request::Snapshot(id) => {
                let Some(ep) = self.endpoints.get(&id) else {
                    return Ok(Reply::Snapshot(None));
                };
                if ep.listener.is_some() || ep.serial.is_some() {
                    return Err(Error::new("STILL_RUNNING", "请先确认端点停止"));
                }
                Ok(Reply::Snapshot(Some(
                    ep.units
                        .values()
                        .map(|(id, map)| Ok((id.clone(), map.values()?)))
                        .collect::<Result<_, Error>>()?,
                )))
            }
        }
    }
    async fn start(
        &mut self,
        snapshot: Snapshot,
        max: usize,
        command: &str,
    ) -> Result<SlaveView, Error> {
        let id = snapshot.endpoint.id.clone();
        if let Some(ep) = self.endpoints.get(&id) {
            if ep.listener.is_some() || ep.serial.is_some() {
                return Err(Error::new("ALREADY_RUNNING", "端点仍持有监听资源"));
            }
        }
        if !(1..=100).contains(&max) {
            return Err(Error::new(
                "INVALID_CONNECTION_LIMIT",
                "每端点连接上限须为1～100",
            ));
        }
        if snapshot.endpoint.role != Role::Slave {
            return Err(Error::new(
                "CAPABILITY_UNAVAILABLE",
                "仅从站可以启动本地应答服务",
            ));
        }
        snapshot.endpoint.validate()?;
        if !snapshot.endpoint.units.iter().any(|u| u.enabled) {
            return Err(Error::new("NO_ENABLED_UNIT", "至少启用一个已应用Unit"));
        }
        // 先预留再编译布局和分配C映射，旧停止映射在替换前仍计费。
        let bytes = crate::resources::serialized_size(&snapshot)?.saturating_mul(8);
        let memory = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                memory_bytes: bytes,
                ..Default::default()
            })?;
        let active = self
            .resources
            .reserve(essim_core::domain::resources::ResourceUsage {
                endpoints: 1,
                connections: if matches!(snapshot.endpoint.transport, Transport::Tcp { .. }) {
                    max as u64
                } else {
                    0
                },
                memory_bytes: (max as u64) * BUFFER as u64 * 2,
                ..Default::default()
            })?;
        let mut units = BTreeMap::new();
        self.models
            .borrow_mut()
            .retain(|model| model.strong_count() > 0);
        for unit in &snapshot.endpoint.units {
            let values = snapshot
                .values
                .get(&unit.id)
                .ok_or_else(|| Error::new("NOT_APPLIED", "缺少Unit已应用值"))?;
            let model = values
                .model
                .as_ref()
                .ok_or_else(|| Error::new("NOT_APPLIED", "缺少已应用模型"))?;
            if unit.enabled && model.points.is_empty() {
                return Err(Error::new("EMPTY_UNIT_MODEL","启用单元的有效点表为空，请补齐配置后应用").at_unit(&snapshot.endpoint.id,Some(&unit.id),"model"));
            }
            if model.id != unit.model.id {
                return Err(Error::new("MODEL_MISMATCH", "Unit引用与已应用模型不一致"));
            }
            // 同一固定协议线程中的端点共享不可变布局；C映射仍各Unit独立。
            let existing = {
                let models = self.models.borrow();
                models.iter().filter_map(Weak::upgrade).find(|layout| {
                    Arc::ptr_eq(&layout.model, model) || layout.model.as_ref() == model.as_ref()
                })
            };
            let layout = if let Some(layout) = existing {
                layout
            } else {
                let layout = Arc::new(ModelLayout::shared(model.clone())?);
                self.models.borrow_mut().push(Arc::downgrade(&layout));
                layout
            };
            let mut map = UnitMapping::new(layout, values)?;
            // 同一已应用快照的停启从所有者保留值，不把旧启动输入重新当成初值。
            if let Some(old) = self
                .endpoints
                .get(&id)
                .filter(|ep| ep.config == snapshot.endpoint)
                .and_then(|ep| ep.units.values().find(|(old_id, _)| old_id == &unit.id))
            {
                if *old.1.layout.model != *map.layout.model {
                    return Err(Error::new(
                        "CONFIG_CHANGED",
                        "已应用模型与运行模型不一致，请重新应用",
                    ));
                }
                let previous = old.1.values()?;
                if !previous.raw.is_empty() {
                    let plan = map
                        .layout
                        .plan_values(&previous.raw.into_iter().collect::<Vec<_>>())?;
                    UnitMapping::apply_to(&mut map.mapping, &plan)?;
                }
            }
            units.insert(unit.station, (unit.id.clone(), map));
        }
        let token = self.token();
        let mut view = idle(id.clone());
        view.state = SlaveState::Starting;
        view.max_clients = max;
        memory.commit();
        let mut ep = Endpoint {
            config: snapshot.endpoint,
            cache: self.cache.clone(),
            _memory: memory,
            active: Some(active),
            units,
            listener: None,
            serial: None,
            token,
            journal: None,
            view,
            clients: BTreeSet::new(),
        };
        self.cache
            .lock()
            .map_err(|_| failure("查询缓存不可用"))?
            .insert(
                id.clone(),
                Cached {
                    view: ep.view.clone(),
                    values: ep
                        .units
                        .values()
                        .map(|(id, u)| Ok((id.clone(), u.values()?)))
                        .collect::<Result<_, Error>>()?,
                },
            );
        // 记录实际本次启动值，包含停启之间保留的修改。
        drop(snapshot.values);
        let result: Result<(), Error> = (async {
            ep.view.pending_recording = true;
            publish_cache(&ep);
            let journal =
                Journal::create(&self.root, &ep.config, &ep.units, &self.recording).await?;
            ep.view.pending_recording = false;
            ep.view.record_path = Some(journal.path.clone());
            ep.journal = Some(journal);
            match &ep.config.transport {
                Transport::Tcp { address, port } => {
                    let address = format!("{address}:{port}").parse().map_err(failure)?;
                    let mut listener = TcpListener::bind(address).map_err(failure)?;
                    self.poll
                        .registry()
                        .register(&mut listener, token, Interest::READABLE)
                        .map_err(failure)?;
                    ep.listener = Some(listener);
                }
                Transport::Rtu { .. } => {
                    ep.serial = Some(serial::SerialState::open(&ep.config)?);
                }
            }
            record(&mut ep, Some(command), SlaveFact::Listening).await?;
            ep.view.state = SlaveState::Running;
            revise(&mut ep);
            Ok(())
        })
        .await;
        if let Err(error) = result {
            ep.listener = None;
            ep.serial = None;
            ep.active.take();
            ep.view.pending_recording = false;
            ep.view.state = SlaveState::Failed;
            ep.view.error = Some(error.message.clone());
            revise(&mut ep);
            self.endpoints.insert(id, ep);
            return Err(error);
        }
        if let Some(active) = &ep.active {
            active.commit();
        }
        if ep.listener.is_some() {
            self.listeners.insert(token.0, id.clone());
        }
        let view = ep.view.clone();
        self.endpoints.insert(id, ep);
        Ok(view)
    }
    fn close_resources(&mut self, id: &EndpointId) {
        if let Some(ep) = self.endpoints.get_mut(id) {
            self.listeners.remove(&ep.token.0);
            ep.listener.take();
            if let Some(serial) = ep.serial.take() {
                if serial.pending_output() {
                    ep.view.uncertain = true;
                }
                drop(serial);
            }
            for token in std::mem::take(&mut ep.clients) {
                if let Some(client) = self.clients.remove(&token) {
                    self.global_clients.fetch_sub(1, Ordering::AcqRel);
                    if !client.output.is_empty() {
                        ep.view.uncertain = true;
                    }
                    drop(client);
                }
            }
            ep.active.take();
            ep.view.clients = 0;
            revise(ep);
        }
    }
    async fn stop(&mut self, id: &EndpointId, command: &str) -> Result<SlaveView, Error> {
        let Some(ep) = self.endpoints.get_mut(id) else {
            let mut view = idle(id.clone());
            view.state = SlaveState::Stopped;
            return Ok(view);
        };
        if ep.listener.is_none() && ep.serial.is_none() {
            return Ok(ep.view.clone());
        }
        ep.view.state = SlaveState::Stopping;
        let stopping = record(ep, Some(command), SlaveFact::Stopping).await;
        self.close_resources(id);
        let ep = self.endpoints.get_mut(id).unwrap();
        stopping?;
        record(ep, Some(command), SlaveFact::Stopped).await?;
        ep.view.state = SlaveState::Stopped;
        revise(ep);
        Ok(ep.view.clone())
    }
    async fn values(&mut self, command: SlaveCommand) -> Result<SlaveView, Error> {
        let ep = self
            .endpoints
            .get_mut(&command.endpoint)
            .ok_or_else(|| Error::new("NOT_STARTED", "请先启动一次TCP从站以建立运行值所有者"))?;
        if ep.view.state == SlaveState::Failed {
            return Err(Error::new("RUN_FAILED", "请先排除运行或录制故障并重新启动"));
        }
        let (id, values, plan) = match command.action {
            SlaveAction::SetValues { unit, values } => {
                let map = &ep
                    .units
                    .values()
                    .find(|(id, _)| id == &unit)
                    .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "Unit未应用"))?
                    .1;
                let plan = map.layout.plan_values(&values)?;
                (unit, values, plan)
            }
            SlaveAction::ResetValues { unit, points } => {
                let map = &ep
                    .units
                    .values()
                    .find(|(id, _)| id == &unit)
                    .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "Unit未应用"))?
                    .1;
                let (values, plan) = map.reset_plan(&points)?;
                (unit, values, plan)
            }
            _ => return Err(Error::new("INVALID_COMMAND", "该命令不是运行值操作")),
        };
        let changed:Vec<_>=values.iter().map(|(id,_)|id.clone()).collect();
        record(
            ep,
            Some(&command.command_id),
            SlaveFact::ValuesIntent {
                unit: id.clone(),
                values,
            },
        )
        .await?;
        let map = &mut ep
            .units
            .values_mut()
            .find(|(unit, _)| unit == &id)
            .unwrap()
            .1;
        UnitMapping::apply_to(&mut map.mapping, &plan)?;
        record(
            ep,
            Some(&command.command_id),
            SlaveFact::ValuesCommitted { unit: id.clone() },
        )
        .await?;
        refresh_cached_unit(ep,&id,Some(changed))?;
        Ok(ep.view.clone())
    }
    async fn accept(&mut self, token: usize) {
        let Some(id) = self.listeners.get(&token).cloned() else {
            return;
        };
        for _ in 0..32 {
            let accepted = {
                let Some(listener) = self
                    .endpoints
                    .get_mut(&id)
                    .and_then(|ep| ep.listener.as_mut())
                else {
                    return;
                };
                listener.accept()
            };
            let (mut socket, peer) = match accepted {
                Ok(pair) => pair,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            };
            let ep = self.endpoints.get(&id).unwrap();
            if ep.clients.len() >= ep.view.max_clients
                || self
                    .global_clients
                    .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                        (count < 10000).then_some(count + 1)
                    })
                    .is_err()
            {
                drop(socket);
                continue;
            }
            let client_token = self.token();
            if self
                .poll
                .registry()
                .register(&mut socket, client_token, Interest::READABLE)
                .is_err()
            {
                self.global_clients.fetch_sub(1, Ordering::AcqRel);
                continue;
            }
            let ep = self.endpoints.get_mut(&id).unwrap();
            if record(
                ep,
                None,
                SlaveFact::Connected {
                    client: client_token.0 as u64,
                    peer: peer.to_string(),
                },
            )
            .await
            .is_err()
            {
                self.global_clients.fetch_sub(1, Ordering::AcqRel);
                break;
            }
            ep.clients.insert(client_token.0);
            ep.view.clients = ep.clients.len();
            self.clients.insert(
                client_token.0,
                Client {
                    socket,
                    endpoint: id.clone(),
                    input: Vec::new(),
                    output: VecDeque::new(),
                    last_progress: Instant::now(),
                    frame_started: None,
                },
            );
        }
    }
    async fn client(&mut self, token: usize) {
        let Some(mut client) = self.clients.remove(&token) else {
            return;
        };
        let Some(ep) = self.endpoints.get_mut(&client.endpoint) else {
            return;
        };
        let result = (async {
            if ep.view.state != SlaveState::Running {
                return Err(Error::new("ENDPOINT_STOPPED", "端点已停止"));
            }
            let mut buffer = [0; 4096];
            loop {
                match client.socket.read(&mut buffer) {
                    Ok(0) => return Err(Error::new("CLIENT_CLOSED", "客户端断开")),
                    Ok(n) => {
                        if client.input.is_empty() {
                            client.frame_started = Some(Instant::now());
                        }
                        client.input.extend_from_slice(&buffer[..n]);
                        if client.input.len() > BUFFER {
                            return Err(Error::new("BUFFER_LIMIT", "客户端接收缓冲超限"));
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(failure(e)),
                }
            }
            for _ in 0..16 {
                if client.input.len() < 6 {
                    break;
                }
                let length = usize::from(u16::from_be_bytes([client.input[4], client.input[5]]));
                if client.input[2..4] != [0, 0] || !(2..=254).contains(&length) {
                    return Err(Error::new("INVALID_MBAP", "非法MBAP长度或协议标识"));
                }
                let length = length + 6;
                if client.input.len() < length {
                    break;
                }
                let request: Vec<_> = client.input.drain(..length).collect();
                client.frame_started = if client.input.is_empty() {
                    None
                } else {
                    Some(Instant::now())
                };
                let started = Instant::now();
                record(
                    ep,
                    None,
                    SlaveFact::Request {
                        client: token as u64,
                        bytes: request.clone(),
                    },
                )
                .await?;
                let station = request[6];
                let available = ep
                    .config
                    .units
                    .iter()
                    .any(|unit| unit.station == station && unit.enabled);
                let response = if available {
                    let (_, unit) = ep.units.get_mut(&station).unwrap();
                    let exception = unit.check_request(&request).err();
                    self.responder
                        .respond(&request, &mut unit.mapping, exception)
                } else {
                    self.responder.respond(
                        &request,
                        &mut self.empty,
                        Some(Exception::GatewayTargetUnavailable),
                    )
                }
                .map_err(failure)?;
                ep.view.requests = (ep.view.requests.parse::<u64>().unwrap_or(0) + 1).to_string();
                if response.get(7).is_some_and(|f| f & 0x80 != 0) {
                    ep.view.exceptions =
                        (ep.view.exceptions.parse::<u64>().unwrap_or(0) + 1).to_string();
                }
                record(
                    ep,
                    None,
                    SlaveFact::ResponsePrepared {
                        client: token as u64,
                        bytes: response.clone(),
                    },
                )
                .await?;
                refresh_cached_request(ep, station, &request[7..])?;
                ep.view.last_request_micros = started.elapsed().as_micros().to_string();
                if client
                    .output
                    .iter()
                    .map(|(bytes, offset)| bytes.len() - offset)
                    .sum::<usize>()
                    + response.len()
                    > BUFFER
                {
                    return Err(Error::new("BUFFER_LIMIT", "客户端发送队列超限"));
                }
                if client.output.is_empty() {
                    client.last_progress = Instant::now();
                }
                client.output.push_back((response, 0));
            }
            while let Some((bytes, offset)) = client.output.front_mut() {
                match client.socket.write(&bytes[*offset..]) {
                    Ok(0) => return Err(Error::new("CLIENT_CLOSED", "客户端无法接收")),
                    Ok(n) => {
                        *offset += n;
                        client.last_progress = Instant::now();
                        if *offset == bytes.len() {
                            client.output.pop_front();
                            record(
                                ep,
                                None,
                                SlaveFact::ResponseSent {
                                    client: token as u64,
                                },
                            )
                            .await?;
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(failure(e)),
                }
            }
            if client
                .frame_started
                .is_some_and(|t| t.elapsed() > Duration::from_secs(5))
                || (!client.output.is_empty()
                    && client.last_progress.elapsed() > Duration::from_secs(5))
            {
                return Err(Error::new("CLIENT_TIMEOUT", "客户端半帧或发送停滞超时"));
            }
            let interest = if client.output.is_empty() {
                Interest::READABLE
            } else {
                Interest::READABLE.add(Interest::WRITABLE)
            };
            self.poll
                .registry()
                .reregister(&mut client.socket, Token(token), interest)
                .map_err(failure)?;
            Ok(())
        })
        .await;
        match result {
            Ok(()) => {
                self.clients.insert(token, client);
            }
            Err(error) => {
                self.global_clients.fetch_sub(1, Ordering::AcqRel);
                if !client.output.is_empty() {
                    ep.view.uncertain = true;
                }
                ep.clients.remove(&token);
                ep.view.clients = ep.clients.len();
                let _ = record(
                    ep,
                    None,
                    SlaveFact::Disconnected {
                        client: token as u64,
                        reason: error.message,
                    },
                )
                .await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_failure_after_write_preserves_uncertainty_and_releases_resources() {
        test_block_on(async {
            let root = std::env::temp_dir().join(format!(
                "essim-record-fault-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            ));
            let model=serde_json::from_value(serde_json::json!({"id":"m","name":"M","points":[{"id":"p","name":"P","category":"measurement","area":"holding","address":0,"valueType":"u16","scale":"1","unit":"","initial":{"kind":"unsigned","value":"7"},"initialEngineering":"7","byteOrder":"big","wordOrder":"big","note":"","slaveWritable":true,"masterPeriodMs":1000}]})).unwrap();
            let endpoint=serde_json::from_value(serde_json::json!({"id":"e","name":"E","role":"slave","transport":{"kind":"tcp","address":"127.0.0.1","port":std::net::TcpListener::bind("127.0.0.1:0").unwrap().local_addr().unwrap().port()},"units":[{"id":"u","name":"U","station":1,"enabled":true,"model":{"id":"m"},"device":null}]})).unwrap();
            let snapshot = Snapshot {
                endpoint,
                values: BTreeMap::from([(
                    UnitId("u".into()),
                    essim_core::domain::values::reset_values(Role::Slave, &model),
                )]),
            };
            let poll = Poll::new().unwrap();
            let mut owner = Owner {
                resources: Default::default(),
                cache: Default::default(),
                recording: crate::recording::RecordingPool::new(Default::default()).unwrap(),
                poll,
                endpoints: BTreeMap::new(),
                listeners: HashMap::new(),
                clients: HashMap::new(),
                next: 1,
                models: Default::default(),
                global_clients: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
                responder: TcpResponder::new().unwrap(),
                empty: Mapping::new(None, None, None, None).unwrap(),
                root: root.clone(),
            };
            let id = EndpointId("e".into());
            owner.start(snapshot, 10, "start").await.unwrap();
            let ep = owner.endpoints.get(&id).unwrap();
            let address = ep.listener.as_ref().unwrap().local_addr().unwrap();
            let listener_token = ep.token.0;
            let mut peer = std::net::TcpStream::connect(address).unwrap();
            let mut accepted = Events::with_capacity(16);
            owner
                .poll
                .poll(&mut accepted, Some(Duration::from_secs(1)))
                .unwrap();
            owner.accept(listener_token).await;
            let token = *owner.clients.keys().next().unwrap();
            let journal = owner
                .endpoints
                .get_mut(&id)
                .unwrap()
                .journal
                .as_mut()
                .unwrap();
            let reliable = journal.sequence + 1;
            journal.fail_after = Some(reliable);
            peer.write_all(&[0, 1, 0, 0, 0, 6, 1, 6, 0, 0, 0, 42])
                .unwrap();
            let mut events = Events::with_capacity(16);
            owner
                .poll
                .poll(&mut events, Some(Duration::from_secs(1)))
                .unwrap();
            owner.client(token).await;
            let ep = owner.endpoints.get(&id).unwrap();
            assert_eq!(ep.view.state, SlaveState::Failed);
            assert!(ep.view.uncertain);
            assert_eq!(ep.view.last_reliable, reliable.to_string());
            assert_eq!(ep.units[&1].1.read_word(2, 0).unwrap(), 42);
            owner.close_resources(&id);
            assert!(std::net::TcpListener::bind(address).is_ok());
            assert!(owner.clients.is_empty());
            drop(owner);
            let _ = std::fs::remove_dir_all(root);
        });
    }
}
