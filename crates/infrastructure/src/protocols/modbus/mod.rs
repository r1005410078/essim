mod native;
mod rtu;
use essim_core::{
    application::simulator::{Error, PacketLog, Result, SimulatorPort, Snapshot},
    domain::modbus::{
        Communication, Endpoint, RtuEndpoint, Soc, State, UnitId, INSTANCE_ID, POINT_ID,
    },
};
use native::Native;
use std::{
    collections::BTreeMap,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    sync::{
        mpsc::{self, Receiver, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

struct Broker {
    snapshot: Snapshot,
    subscribers: Vec<SyncSender<Snapshot>>,
}
impl Broker {
    fn change(&mut self, f: impl FnOnce(&mut Snapshot)) {
        f(&mut self.snapshot);
        self.snapshot.sequence += 1;
        self.subscribers
            .retain(|s| match s.try_send(self.snapshot.clone()) {
                Ok(()) | Err(TrySendError::Full(_)) => true,
                Err(TrySendError::Disconnected(_)) => false,
            });
    }
    fn log(&mut self, direction: &'static str, bytes: &[u8], message: impl Into<String>) {
        self.change(|s| {
            s.logs.push(PacketLog {
                direction,
                hex: bytes
                    .iter()
                    .map(|b| format!("{b:02X}"))
                    .collect::<Vec<_>>()
                    .join(" "),
                message: message.into(),
            });
            if s.logs.len() > 128 {
                s.logs.remove(0);
            }
        });
    }
}
struct Running {
    sender: SyncSender<Command>,
    join: JoinHandle<()>,
}
enum Command {
    Stop,
    Update(UnitId, Soc, SyncSender<Result<Snapshot>>),
    AddUnit(UnitId, SyncSender<Result<Snapshot>>),
}
pub struct ModbusSimulator {
    broker: Arc<Mutex<Broker>>,
    controller: Mutex<Option<Running>>,
}
impl Default for ModbusSimulator {
    fn default() -> Self {
        Self::new()
    }
}
impl ModbusSimulator {
    pub fn new() -> Self {
        Self {
            broker: Arc::new(Mutex::new(Broker {
                snapshot: Snapshot {
                    instance_id: INSTANCE_ID,
                    point_id: POINT_ID,
                    sequence: 0,
                    generation: 0,
                    state: State::Stopped,
                    endpoint: Endpoint::new("127.0.0.1", 1502).unwrap(),
                    communication: Communication::Tcp(Endpoint::new("127.0.0.1", 1502).unwrap()),
                    rtu: None,
                    rtu_soc: Soc::INITIAL,
                    soc: Soc::INITIAL,
                    units: BTreeMap::from([(UnitId::DEFAULT, Soc::INITIAL)]),
                    connections: 0,
                    error: None,
                    logs: vec![],
                },
                subscribers: vec![],
            })),
            controller: Mutex::new(None),
        }
    }
}
fn apply_communication(s: &mut Snapshot, communication: &Communication) {
    match communication {
        Communication::Tcp(endpoint) => s.endpoint = endpoint.clone(),
        Communication::Rtu(endpoint) => s.rtu = Some(endpoint.clone()),
    }
    s.communication = communication.clone();
}
impl ModbusSimulator {
    fn start_communication(&self, communication: Communication) -> Result<Snapshot> {
        rtu::validate(&communication)?;
        let mut guard = self.controller.lock().unwrap();
        if guard.is_some() {
            return Err(Error::new("ALREADY_RUNNING", "模拟器已经启动"));
        }
        self.broker.lock().unwrap().change(|s| {
            s.state = State::Starting;
            apply_communication(s, &communication);
            s.error = None;
            s.generation += 1;
        });
        let (sender, receiver) = mpsc::sync_channel(16);
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let broker = self.broker.clone();
        let join = thread::Builder::new()
            .name("modbus-demo-bms".into())
            .spawn(move || match communication {
                Communication::Tcp(endpoint) => worker(endpoint, broker, receiver, ready_tx),
                Communication::Rtu(endpoint) => rtu::worker(endpoint, broker, receiver, ready_tx),
            })
            .map_err(|e| {
                let err = Error::new("WORKER_FAILED", e.to_string());
                self.broker.lock().unwrap().change(|s| {
                    s.state = State::Failed;
                    s.error = Some(err.clone());
                });
                err
            })?;
        match ready_rx.recv_timeout(Duration::from_secs(1)) {
            Ok(Ok(())) => {
                *guard = Some(Running { sender, join });
                Ok(self.snapshot())
            }
            result => {
                let _ = sender.try_send(Command::Stop);
                let _ = join.join();
                Err(match result {
                    Ok(Err(e)) => e,
                    _ => Error::new("START_FAILED", "启动确认超时或工作线程退出"),
                })
            }
        }
    }
}
impl SimulatorPort for ModbusSimulator {
    fn snapshot(&self) -> Snapshot {
        self.broker.lock().unwrap().snapshot.clone()
    }
    fn start(&self, endpoint: Endpoint) -> Result<Snapshot> {
        self.start_communication(Communication::Tcp(endpoint))
    }
    fn start_rtu(&self, endpoint: RtuEndpoint) -> Result<Snapshot> {
        self.start_communication(Communication::Rtu(endpoint))
    }
    fn configure(&self, communication: Communication) -> Result<Snapshot> {
        let guard = self.controller.lock().unwrap();
        if guard.is_some() {
            return Err(Error::new("ALREADY_RUNNING", "请先停止再应用通信配置"));
        }
        rtu::validate(&communication)?;
        self.broker.lock().unwrap().change(|s| {
            apply_communication(s, &communication);
            s.error = None;
            s.state = State::Stopped;
        });
        Ok(self.snapshot())
    }
    fn stop(&self) -> Result<Snapshot> {
        let mut guard = self.controller.lock().unwrap();
        if let Some(running) = guard.take() {
            self.broker
                .lock()
                .unwrap()
                .change(|s| s.state = State::Stopping);
            let _ = running.sender.send(Command::Stop);
            if running.join.join().is_err() {
                return Err(Error::new("WORKER_FAILED", "协议线程异常退出"));
            }
        }
        self.broker.lock().unwrap().change(|s| {
            s.state = State::Stopped;
            s.connections = 0;
        });
        Ok(self.snapshot())
    }
    fn update(&self, value: Soc) -> Result<Snapshot> {
        self.update_unit(UnitId::DEFAULT, value)
    }
    fn add_unit(&self, unit_id: UnitId) -> Result<Snapshot> {
        let guard = self.controller.lock().unwrap();
        if matches!(self.snapshot().communication, Communication::Rtu(_)) {
            return Err(Error::new("RTU_SINGLE_UNIT", "RTU 首版仅支持一个从站号"));
        }
        if self.snapshot().units.contains_key(&unit_id) {
            return Err(Error::new("UNIT_EXISTS", "Unit ID 已存在"));
        }
        if let Some(running) = guard.as_ref() {
            let (tx, rx) = mpsc::sync_channel(1);
            running
                .sender
                .send(Command::AddUnit(unit_id, tx))
                .map_err(|_| Error::new("WORKER_FAILED", "协议线程已经退出"))?;
            rx.recv_timeout(Duration::from_secs(1))
                .map_err(|_| Error::new("ADD_UNIT_TIMEOUT", "添加未收到确认；请查询当前快照"))?
        } else {
            self.broker.lock().unwrap().change(|s| {
                s.units.insert(unit_id, Soc::INITIAL);
            });
            Ok(self.snapshot())
        }
    }
    fn update_unit(&self, unit_id: UnitId, value: Soc) -> Result<Snapshot> {
        let guard = self.controller.lock().unwrap();
        let snapshot = self.snapshot();
        let valid = match &snapshot.communication {
            Communication::Tcp(_) => snapshot.units.contains_key(&unit_id),
            Communication::Rtu(endpoint) => endpoint.slave == unit_id,
        };
        if !valid {
            return Err(Error::new("UNIT_NOT_FOUND", "Unit ID 不存在"));
        }
        if let Some(running) = guard.as_ref() {
            let (tx, rx) = mpsc::sync_channel(1);
            running
                .sender
                .send(Command::Update(unit_id, value, tx))
                .map_err(|_| Error::new("WORKER_FAILED", "协议线程已经退出"))?;
            rx.recv_timeout(Duration::from_secs(1))
                .map_err(|_| Error::new("UPDATE_TIMEOUT", "更新未收到确认；请查询当前快照"))?
        } else {
            self.broker
                .lock()
                .unwrap()
                .change(|s| match s.communication {
                    Communication::Rtu(_) => s.rtu_soc = value,
                    Communication::Tcp(_) => {
                        s.units.insert(unit_id, value);
                        if unit_id == UnitId::DEFAULT {
                            s.soc = value;
                        }
                    }
                });
            Ok(self.snapshot())
        }
    }
    fn subscribe(&self) -> Receiver<Snapshot> {
        let (tx, rx) = mpsc::sync_channel(64);
        let mut broker = self.broker.lock().unwrap();
        let current = broker.snapshot.clone();
        broker.subscribers.retain(|sender| {
            !matches!(
                sender.try_send(current.clone()),
                Err(TrySendError::Disconnected(_))
            )
        });
        tx.try_send(broker.snapshot.clone()).unwrap();
        broker.subscribers.push(tx);
        rx
    }
}
impl Drop for ModbusSimulator {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}
struct Client {
    stream: TcpStream,
    buffer: Vec<u8>,
    partial_since: Option<Instant>,
}
fn worker(
    endpoint: Endpoint,
    broker: Arc<Mutex<Broker>>,
    commands: Receiver<Command>,
    ready: SyncSender<Result<()>>,
) {
    let initial = broker.lock().unwrap().snapshot.units.clone();
    let initialized = (|| -> std::io::Result<_> {
        // 每个 Unit ID 拥有独立 C context 和映射，均由此 worker 独占。
        let native = initial
            .into_iter()
            .map(|(id, soc)| Native::new(soc.raw()).map(|native| (id, native)))
            .collect::<std::io::Result<BTreeMap<_, _>>>()?;
        let listener = TcpListener::bind((endpoint.address, endpoint.port))?;
        listener.set_nonblocking(true)?;
        Ok((native, listener))
    })();
    let (mut native, listener) = match initialized {
        Ok(v) => v,
        Err(e) => {
            let err = Error::new("START_FAILED", e.to_string());
            broker.lock().unwrap().change(|s| {
                s.state = State::Failed;
                s.error = Some(err.clone());
            });
            let _ = ready.send(Err(err));
            return;
        }
    };
    broker
        .lock()
        .unwrap()
        .change(|s| s.state = State::Listening);
    let _ = ready.send(Ok(()));
    let mut client: Option<Client> = None;
    'running: loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Stop => break 'running,
                Command::AddUnit(unit_id, ack) => {
                    if native.contains_key(&unit_id) {
                        let _ = ack.send(Err(Error::new("UNIT_EXISTS", "Unit ID 已存在")));
                        continue;
                    }
                    match Native::new(Soc::INITIAL.raw()) {
                        Ok(unit) => {
                            native.insert(unit_id, unit);
                            let mut b = broker.lock().unwrap();
                            b.change(|s| {
                                s.units.insert(unit_id, Soc::INITIAL);
                            });
                            let _ = ack.send(Ok(b.snapshot.clone()));
                        }
                        Err(error) => {
                            let _ = ack.send(Err(Error::new("ADD_UNIT_FAILED", error.to_string())));
                        }
                    }
                }
                Command::Update(unit_id, value, ack) => {
                    let Some(unit) = native.get_mut(&unit_id) else {
                        let _ = ack.send(Err(Error::new("UNIT_NOT_FOUND", "Unit ID 不存在")));
                        continue;
                    };
                    if let Err(error) = unit.update(value.raw()) {
                        let _ = ack.send(Err(Error::new("UPDATE_FAILED", error.to_string())));
                        continue;
                    }
                    let mut b = broker.lock().unwrap();
                    b.change(|s| {
                        s.units.insert(unit_id, value);
                        if unit_id == UnitId::DEFAULT {
                            s.soc = value;
                        }
                    });
                    let _ = ack.send(Ok(b.snapshot.clone()));
                }
            }
        }
        // Bound accept work so an incoming connection flood cannot starve stop messages.
        for _ in 0..8 {
            match listener.accept() {
                Ok((stream, _)) => {
                    if client.is_some() {
                        broker.lock().unwrap().log("event", &[], "已拒绝额外客户端");
                        continue;
                    }
                    if stream.set_nonblocking(true).is_err() {
                        continue;
                    }
                    client = Some(Client {
                        stream,
                        buffer: Vec::with_capacity(520),
                        partial_since: None,
                    });
                    broker.lock().unwrap().change(|s| s.connections = 1);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) => {
                    broker.lock().unwrap().log("error", &[], e.to_string());
                    break;
                }
            }
        }
        if let Some(c) = client.as_mut() {
            let mut close = false;
            let mut bytes = [0; 520];
            match c.stream.read(&mut bytes) {
                Ok(0) => close = true,
                Ok(n) => {
                    c.buffer.extend_from_slice(&bytes[..n]);
                    if c.buffer.len() > 1040 {
                        broker.lock().unwrap().log(
                            "error",
                            &[],
                            "请求积压超过 1040 字节，关闭连接",
                        );
                        close = true;
                    }
                    if c.partial_since.is_none() {
                        c.partial_since = Some(Instant::now());
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(_) => close = true,
            }
            // At most four complete frames before checking commands again.
            for _ in 0..4 {
                if close {
                    break;
                }
                let total = match frame_length(&c.buffer) {
                    Ok(Some(n)) => n,
                    Ok(None) => break,
                    Err(e) => {
                        broker.lock().unwrap().log("error", &c.buffer, e);
                        close = true;
                        break;
                    }
                };
                if c.buffer.len() < total {
                    break;
                }
                let frame: Vec<_> = c.buffer.drain(..total).collect();
                let unit_id = UnitId::new(u16::from(frame[6])).ok();
                let unit = unit_id.and_then(|id| native.get_mut(&id));
                let Some(unit) = unit else {
                    broker
                        .lock()
                        .unwrap()
                        .log("rx", &frame, "Unit ID 未配置，关闭连接");
                    close = true;
                    break;
                };
                broker.lock().unwrap().log("rx", &frame, "接收成功");
                match unit.reply(&frame) {
                    Ok(reply) => match send_bounded(&mut c.stream, &reply) {
                        Ok(()) => broker.lock().unwrap().log("tx", &reply, "发送成功"),
                        Err(e) => {
                            broker.lock().unwrap().log(
                                "tx-failed",
                                &reply,
                                format!("发送未完成：{e}"),
                            );
                            close = true;
                        }
                    },
                    Err(e) => {
                        broker
                            .lock()
                            .unwrap()
                            .log("error", &[], format!("libmodbus：{e}"));
                        close = true;
                    }
                }
                c.partial_since = if c.buffer.is_empty() {
                    None
                } else {
                    Some(Instant::now())
                };
            }
            if c.partial_since
                .is_some_and(|t| t.elapsed() > Duration::from_millis(200))
            {
                broker
                    .lock()
                    .unwrap()
                    .log("error", &c.buffer, "截断报文超时，关闭连接");
                close = true;
            }
            if close {
                client = None;
                broker.lock().unwrap().change(|s| s.connections = 0);
            }
        }
        thread::sleep(Duration::from_millis(5));
    }
    // Rust drops the external client/listener and the native context before stop joins.
    drop(client);
    drop(listener);
    drop(native);
    broker.lock().unwrap().change(|s| {
        s.connections = 0;
        s.state = State::Stopped;
    });
}
fn send_bounded(stream: &mut TcpStream, bytes: &[u8]) -> std::io::Result<()> {
    let deadline = Instant::now() + Duration::from_millis(100);
    let mut offset = 0;
    while offset < bytes.len() {
        match stream.write(&bytes[offset..]) {
            Ok(0) => return Err(std::io::ErrorKind::WriteZero.into()),
            Ok(n) => offset += n,
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline => {
                thread::sleep(Duration::from_millis(1))
            }
            Err(e) => return Err(e),
        }
    }
    Ok(())
}
// A transport guard validates MBAP and fixed request shapes before handing a full
// ADU to libmodbus. It does not decode register values or construct responses.
fn frame_length(bytes: &[u8]) -> std::result::Result<Option<usize>, &'static str> {
    if bytes.len() < 6 {
        return Ok(None);
    }
    let len = u16::from_be_bytes([bytes[4], bytes[5]]) as usize;
    if bytes[2] != 0 || bytes[3] != 0 || !(2..=254).contains(&len) {
        return Err("非法 MBAP 协议标识或长度");
    }
    let total = 6 + len;
    if bytes.len() < 8 {
        return Ok(None);
    }
    match bytes[7] {
        0..=6 if total != 12 => return Err("非法固定长度 PDU"),
        15 | 16 => {
            if total < 13 {
                return Err("非法 FC16 长度");
            }
            if bytes.len() < 13 {
                return Ok(None);
            }
            if total != 13 + bytes[12] as usize {
                return Err("FC16 字节数不匹配");
            }
        }
        0..=6 => {}
        22 if total != 14 => return Err("非法 FC22 长度"),
        22 => {}
        23 => {
            if total < 17 {
                return Err("非法 FC23 长度");
            }
            if bytes.len() < 17 {
                return Ok(None);
            }
            if total != 17 + bytes[16] as usize {
                return Err("FC23 字节数不匹配");
            }
        }
        _ if total != 8 => return Err("当前未支持的 PDU 形状"),
        _ => {}
    }
    Ok(Some(total))
}
