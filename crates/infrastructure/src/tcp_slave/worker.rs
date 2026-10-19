//! 固定线程内的监听、C映射、运行事实和非阻塞客户端所有权。
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
    units: BTreeMap<u8, (UnitId, UnitMapping)>,
    listener: Option<TcpListener>,
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
struct Owner {
    poll: Poll,
    endpoints: BTreeMap<EndpointId, Endpoint>,
    listeners: HashMap<usize, EndpointId>,
    clients: HashMap<usize, Client>,
    next: usize,
    models: Vec<Weak<ModelLayout>>,
    global_clients: Arc<std::sync::atomic::AtomicUsize>,
    responder: TcpResponder,
    empty: Mapping,
    root: PathBuf,
}
fn idle(id: EndpointId) -> SlaveView {
    SlaveView {
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
}
fn record(ep: &mut Endpoint, command: Option<&str>, fact: SlaveFact) -> Result<(), Error> {
    let journal = ep
        .journal
        .as_mut()
        .ok_or_else(|| Error::new("RECORDING_FAILED", "运行记录未建立"))?;
    if let Err(e) = journal.record(command, &fact) {
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
    if ep.view.observations.len() > 100 { ep.view.observations.remove(0); }
    ep.view.events.push(value.to_string());
    if ep.view.events.len() > 100 {
        ep.view.events.remove(0);
    }
    revise(ep);
    Ok(())
}
pub(super) fn run(
    poll: Poll,
    rx: Receiver<Message>,
    root: PathBuf,
    shutdown: Arc<AtomicBool>,
    global_clients: Arc<std::sync::atomic::AtomicUsize>,
    ready: std::sync::mpsc::SyncSender<Result<(), Error>>,
) {
    let responder = match TcpResponder::new() {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(failure(error)));
            return;
        }
    };
    let empty = match Mapping::new(None, None, None, None) {
        Ok(value) => value,
        Err(error) => {
            let _ = ready.send(Err(failure(error)));
            return;
        }
    };
    let mut owner = Owner {
        poll,
        endpoints: BTreeMap::new(),
        listeners: HashMap::new(),
        clients: HashMap::new(),
        next: 1,
        models: vec![],
        global_clients,
        responder,
        empty,
        root,
    };
    let _ = ready.send(Ok(()));
    let mut events = Events::with_capacity(256);
    'running: while !shutdown.load(Ordering::Acquire) {
        for _ in 0..64 {
            match rx.try_recv() {
                Ok(message) => {
                    let result = owner.handle(message.request);
                    let _ = message.reply.send(result);
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => break 'running,
                Err(_) => break,
            }
        }
        if let Err(e) = owner
            .poll
            .poll(&mut events, Some(Duration::from_millis(20)))
        {
            if e.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        let tokens: Vec<_> = events
            .iter()
            .map(|event| event.token().0)
            .filter(|token| *token != 0)
            .collect();
        for token in tokens {
            if owner.listeners.contains_key(&token) {
                owner.accept(token);
            } else {
                owner.client(token);
            }
        }
        // 一次事件最多处理16帧；剩余已缓冲帧在后续轮次继续，不能依赖新的边沿通知。
        let pending: Vec<_> = owner
            .clients
            .iter()
            .filter(|(_, c)| !c.input.is_empty() || !c.output.is_empty())
            .map(|(token, _)| *token)
            .collect();
        for token in pending {
            owner.client(token);
        }
        let failed: Vec<_> = owner
            .endpoints
            .iter()
            .filter(|(_, ep)| ep.view.state == SlaveState::Failed && ep.listener.is_some())
            .map(|(id, _)| id.clone())
            .collect();
        for id in failed {
            owner.close_resources(&id);
        }
    }
    let ids: Vec<_> = owner.endpoints.keys().cloned().collect();
    for id in ids {
        let _ = owner.stop(&id, "application-exit");
    }
}
impl Owner {
    fn token(&mut self) -> Token {
        let token = Token(self.next);
        self.next = self.next.checked_add(1).expect("运行token耗尽");
        token
    }
    fn handle(&mut self, request: Request) -> Result<Reply, Error> {
        match request {
            Request::Start(snapshot, max, command) => {
                self.start(snapshot, max, &command).map(Reply::View)
            }
            Request::Stop(id, command) => self.stop(&id, &command).map(Reply::View),
            Request::Query(query) => self.query(query).map(Reply::View),
            Request::Values(command) => self.values(command).map(Reply::View),
            Request::Forget(id) => {
                if self
                    .endpoints
                    .get(&id)
                    .is_some_and(|ep| ep.listener.is_some())
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
                if ep.listener.is_some() {
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
    fn start(&mut self, snapshot: Snapshot, max: usize, command: &str) -> Result<SlaveView, Error> {
        let id = snapshot.endpoint.id.clone();
        if let Some(ep) = self.endpoints.get(&id) {
            if ep.listener.is_some() {
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
            return Err(Error::new("CAPABILITY_UNAVAILABLE", "本阶段仅接入TCP从站"));
        }
        let Transport::Tcp { address, port } = &snapshot.endpoint.transport else {
            return Err(Error::new("CAPABILITY_UNAVAILABLE", "RTU运行尚未接入"));
        };
        let address = format!("{address}:{port}").parse().map_err(failure)?;
        snapshot.endpoint.validate()?;
        if !snapshot.endpoint.units.iter().any(|u| u.enabled) {
            return Err(Error::new("NO_ENABLED_UNIT", "至少启用一个已应用Unit"));
        }
        let mut units = BTreeMap::new();
        self.models.retain(|model| model.strong_count() > 0);
        for unit in &snapshot.endpoint.units {
            let values = snapshot
                .values
                .get(&unit.id)
                .ok_or_else(|| Error::new("NOT_APPLIED", "缺少Unit已应用值"))?;
            let model = values
                .model
                .as_ref()
                .ok_or_else(|| Error::new("NOT_APPLIED", "缺少已应用模型"))?;
            if model.id != unit.model.id {
                return Err(Error::new("MODEL_MISMATCH", "Unit引用与已应用模型不一致"));
            }
            let layout = if let Some(layout) = self
                .models
                .iter()
                .filter_map(Weak::upgrade)
                .find(|layout| *layout.model == *model)
            {
                layout
            } else {
                let layout = Arc::new(ModelLayout::new(model.clone())?);
                self.models.push(Arc::downgrade(&layout));
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
        let mut ep = Endpoint {
            config: snapshot.endpoint,
            units,
            listener: None,
            token,
            journal: None,
            view,
            clients: BTreeSet::new(),
        };
        // 记录实际本次启动值，包含停启之间保留的修改。
        drop(snapshot.values);
        let result: Result<(), Error> = (|| {
            let journal = Journal::create(&self.root, &ep.config, &ep.units)?;
            ep.view.record_path = Some(journal.path.clone());
            ep.journal = Some(journal);
            let mut listener = TcpListener::bind(address).map_err(failure)?;
            self.poll
                .registry()
                .register(&mut listener, token, Interest::READABLE)
                .map_err(failure)?;
            ep.listener = Some(listener);
            record(&mut ep, Some(command), SlaveFact::Listening)?;
            ep.view.state = SlaveState::Running;
            revise(&mut ep);
            Ok(())
        })();
        if let Err(error) = result {
            ep.listener = None;
            ep.view.state = SlaveState::Failed;
            ep.view.error = Some(error.message.clone());
            revise(&mut ep);
            self.endpoints.insert(id, ep);
            return Err(error);
        }
        self.listeners.insert(token.0, id.clone());
        let view = ep.view.clone();
        self.endpoints.insert(id, ep);
        Ok(view)
    }
    fn close_resources(&mut self, id: &EndpointId) {
        if let Some(ep) = self.endpoints.get_mut(id) {
            self.listeners.remove(&ep.token.0);
            ep.listener.take();
            for token in std::mem::take(&mut ep.clients) {
                if let Some(client) = self.clients.remove(&token) {
                    self.global_clients.fetch_sub(1, Ordering::AcqRel);
                    if !client.output.is_empty() {
                        ep.view.uncertain = true;
                    }
                    drop(client);
                }
            }
            ep.view.clients = 0;
            revise(ep);
        }
    }
    fn stop(&mut self, id: &EndpointId, command: &str) -> Result<SlaveView, Error> {
        let Some(ep) = self.endpoints.get_mut(id) else {
            let mut view = idle(id.clone());
            view.state = SlaveState::Stopped;
            return Ok(view);
        };
        if ep.listener.is_none() {
            return Ok(ep.view.clone());
        }
        ep.view.state = SlaveState::Stopping;
        let stopping = record(ep, Some(command), SlaveFact::Stopping);
        self.close_resources(id);
        let ep = self.endpoints.get_mut(id).unwrap();
        stopping?;
        record(ep, Some(command), SlaveFact::Stopped)?;
        ep.view.state = SlaveState::Stopped;
        revise(ep);
        Ok(ep.view.clone())
    }
    fn query(&self, query: SlaveQuery) -> Result<SlaveView, Error> {
        if query.points.len() > 1024 {
            return Err(Error::new("QUERY_TOO_LARGE", "单次最多查询1024个点"));
        }
        let Some(ep) = self.endpoints.get(&query.endpoint) else {
            return Ok(idle(query.endpoint));
        };
        let mut view = ep.view.clone();
        if let Some(id) = query.unit {
            let (_, unit) = ep
                .units
                .values()
                .find(|(unit, _)| unit == &id)
                .ok_or_else(|| Error::new("UNIT_NOT_FOUND", "Unit未应用"))?;
            for point in query.points {
                view.values.insert(point.clone(), unit.read_point(&point)?);
            }
        }
        Ok(view)
    }
    fn values(&mut self, command: SlaveCommand) -> Result<SlaveView, Error> {
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
        record(
            ep,
            Some(&command.command_id),
            SlaveFact::ValuesIntent {
                unit: id.clone(),
                values,
            },
        )?;
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
            SlaveFact::ValuesCommitted { unit: id },
        )?;
        Ok(ep.view.clone())
    }
    fn accept(&mut self, token: usize) {
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
    fn client(&mut self, token: usize) {
        let Some(mut client) = self.clients.remove(&token) else {
            return;
        };
        let Some(ep) = self.endpoints.get_mut(&client.endpoint) else {
            return;
        };
        let result = (|| -> Result<(), Error> {
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
                )?;
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
                )?;
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
                            )?;
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
        })();
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
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recording_failure_after_write_preserves_uncertainty_and_releases_resources() {
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
            poll,
            endpoints: BTreeMap::new(),
            listeners: HashMap::new(),
            clients: HashMap::new(),
            next: 1,
            models: vec![],
            global_clients: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            responder: TcpResponder::new().unwrap(),
            empty: Mapping::new(None, None, None, None).unwrap(),
            root: root.clone(),
        };
        let id = EndpointId("e".into());
        owner.start(snapshot, 10, "start").unwrap();
        let ep = owner.endpoints.get(&id).unwrap();
        let address = ep.listener.as_ref().unwrap().local_addr().unwrap();
        let listener_token = ep.token.0;
        let mut peer = std::net::TcpStream::connect(address).unwrap();
        let mut accepted = Events::with_capacity(16);
        owner
            .poll
            .poll(&mut accepted, Some(Duration::from_secs(1)))
            .unwrap();
        owner.accept(listener_token);
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
        owner.client(token);
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
    }
}
