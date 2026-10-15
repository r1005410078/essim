//! RTU 从站 worker；直接使用 C 串口上下文，不经过 TCP 帧门禁或 socketpair。
use super::*;
use essim_core::domain::modbus::SOC_ADDRESS;
use libmodbus_rs::{Exception, HoldingRegisters, RtuConfig, RtuContext};
fn config(e: &RtuEndpoint) -> std::io::Result<RtuConfig> {
    RtuConfig::new(
        &e.device,
        e.baud,
        e.parity,
        e.data_bits,
        e.stop_bits,
        e.slave.raw().into(),
        e.receive_timeout_ms,
        e.byte_timeout_ms,
    )
}
pub(super) fn validate(c: &Communication) -> Result<()> {
    match c {
        Communication::Tcp(e) => Endpoint::new(&e.address.to_string(), e.port)
            .map(|_| ())
            .map_err(|e| Error::new("INVALID_ENDPOINT", e)),
        Communication::Rtu(e) => config(e)
            .map(|_| ())
            .map_err(|e| Error::new("INVALID_RTU_CONFIG", e.to_string())),
    }
}
pub(super) fn worker(
    endpoint: RtuEndpoint,
    broker: Arc<Mutex<Broker>>,
    commands: Receiver<Command>,
    ready: SyncSender<Result<()>>,
) {
    let initial = broker.lock().unwrap().snapshot.rtu_soc;
    let initialized = (|| -> std::io::Result<_> {
        let mut mapping = HoldingRegisters::new(SOC_ADDRESS, 1)?;
        mapping.set(SOC_ADDRESS, initial.raw())?;
        let ctx = RtuContext::open(&config(&endpoint)?)?;
        Ok((ctx, mapping))
    })();
    let (mut ctx, mut mapping) = match initialized {
        Ok(v) => v,
        Err(e) => {
            let error = Error::new("START_FAILED", e.to_string());
            broker.lock().unwrap().change(|s| {
                s.state = State::Failed;
                s.error = Some(error.clone());
            });
            let _ = ready.send(Err(error));
            return;
        }
    };
    broker.lock().unwrap().change(|s| {
        s.state = State::Listening;
        s.connections = 0;
    });
    let _ = ready.send(Ok(()));
    'running: loop {
        while let Ok(command) = commands.try_recv() {
            match command {
                Command::Stop => break 'running,
                Command::AddUnit(_, ack) => {
                    let _ = ack.send(Err(Error::new(
                        "RTU_SINGLE_UNIT",
                        "RTU 首版仅支持一个从站号",
                    )));
                }
                Command::Update(id, value, ack) => {
                    if id != endpoint.slave {
                        let _ = ack.send(Err(Error::new("UNIT_NOT_FOUND", "RTU 站号不匹配")));
                        continue;
                    }
                    if let Err(e) = mapping.set(SOC_ADDRESS, value.raw()) {
                        let _ = ack.send(Err(Error::new("UPDATE_FAILED", e.to_string())));
                        continue;
                    }
                    let mut b = broker.lock().unwrap();
                    b.change(|s| s.rtu_soc = value);
                    let _ = ack.send(Ok(b.snapshot.clone()));
                }
            }
        }
        let req = match ctx.receive() {
            Ok(Some(req)) => req,
            Ok(None) => continue,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => continue,
            Err(e)
                if e.kind() == std::io::ErrorKind::InvalidData
                    || e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::Interrupted =>
            {
                broker
                    .lock()
                    .unwrap()
                    .log("error", &[], format!("RTU 接收丢弃：{e}"));
                continue;
            }
            Err(e) => {
                fail(&broker, e);
                break;
            }
        };
        broker
            .lock()
            .unwrap()
            .log("rx", req.as_bytes(), "RTU 接收成功（含 CRC）");
        // SOC 只读；广播不调用任何可能改变映射或发送异常的应答路径。
        if req.is_broadcast() {
            broker
                .lock()
                .unwrap()
                .log("event", &[], "广播忽略：SOC 只读，不应答");
            continue;
        }
        let exception = match req.as_bytes()[1] {
            3 => None,
            6 | 16 => Some(Exception::IllegalAddress),
            _ => Some(Exception::IllegalFunction),
        };
        let response = if let Some(e) = exception {
            ctx.reply_exception(&req, e)
        } else {
            ctx.reply(&req, &mut mapping)
        };
        match response {
            Ok(n) => broker.lock().unwrap().log(
                "tx-result",
                &[],
                format!("RTU 已写入驱动 {n} 字节；未捕获原始 TX"),
            ),
            Err(e) => {
                fail(&broker, e);
                break;
            }
        }
    }
    // I/O 已退出；由 worker 先关闭 context 持有的串口，再释放映射。
    drop(ctx);
    drop(mapping);
    broker.lock().unwrap().change(|s| {
        s.connections = 0;
        if s.state != State::Failed {
            s.state = State::Stopped;
        }
    });
}
fn fail(broker: &Arc<Mutex<Broker>>, e: std::io::Error) {
    let error = Error::new("SERIAL_IO_FAILED", e.to_string());
    let mut b = broker.lock().unwrap();
    b.log("error", &[], &error.message);
    b.change(|s| {
        s.state = State::Failed;
        s.error = Some(error);
    });
}
