//! RTU串口收发；映射和串口始终属于同一固定所有者线程。
use super::*;
use libmodbus_rs::{rtu_crc, RtuConfig, RtuResponder, SerialPort};

pub(super) struct SerialState {
    port: SerialPort,
    responder: RtuResponder,
    input: Vec<u8>,
    output: Vec<u8>,
    written: usize,
    last_byte: Instant,
    send_started: Instant,
}
impl SerialState {
    pub(super) fn open(
        config: &essim_core::domain::endpoint::EndpointConfig,
    ) -> Result<Self, Error> {
        let Transport::Rtu {
            path,
            baud,
            parity,
            data_bits,
            stop_bits,
        } = &config.transport
        else {
            unreachable!()
        };
        let station = config
            .units
            .iter()
            .find(|u| u.enabled)
            .ok_or_else(|| failure("至少启用一个Unit"))?
            .station;
        let cfg = RtuConfig::new(
            path,
            *baud,
            match parity {
                essim_core::domain::endpoint::Parity::Even => 'E',
                essim_core::domain::endpoint::Parity::Odd => 'O',
                _ => 'N',
            },
            *data_bits,
            *stop_bits,
            u16::from(station),
            50,
            2,
        )
        .map_err(failure)?;
        let responder = RtuResponder::new(&cfg).map_err(failure)?;
        let port = SerialPort::open(path, &cfg).map_err(failure)?;
        Ok(Self {
            port,
            responder,
            input: vec![],
            output: vec![],
            written: 0,
            last_byte: Instant::now(),
            send_started: Instant::now(),
        })
    }
    pub(super) fn pending_output(&self) -> bool {
        !self.output.is_empty()
    }
}
fn expected(bytes: &[u8]) -> Option<usize> {
    match *bytes.get(1)? {
        1..=6 | 8 => Some(8),
        15 | 16 => bytes.get(6).map(|n| 9 + usize::from(*n)),
        7 | 11 | 12 | 17 => Some(4),
        _ => None,
    }
}
async fn frame(
    ep: &mut Endpoint,
    direction: &str,
    bytes: &[u8],
    complete: bool,
    discarded: bool,
    stage: &str,
) -> Result<(), Error> {
    record(
        ep,
        None,
        SlaveFact::RtuFrame {
            direction: direction.into(),
            hex: bytes
                .iter()
                .map(|b| format!("{b:02X}"))
                .collect::<Vec<_>>()
                .join(" "),
            complete,
            crc_valid: complete && rtu_crc(bytes) == 0,
            discarded,
            stage: stage.into(),
        },
    )
    .await
}
impl Owner {
    pub(super) async fn serial(&mut self, id: &EndpointId) {
        let Some(ep) = self.endpoints.get_mut(id) else {
            return;
        };
        let Some(mut serial) = ep.serial.take() else {
            return;
        };
        let result = (async {
            if ep.view.state != SlaveState::Running {
                return Err(failure("串口端点已停止"));
            }
            let mut buffer = [0; 512];
            for _ in 0..8 {
                match serial.port.read(&mut buffer) {
                    Ok(0) => return Err(failure("串口已断开")),
                    Ok(n) => {
                        serial.input.extend_from_slice(&buffer[..n]);
                        serial.last_byte = Instant::now();
                    }
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(e) => return Err(failure(e)),
                }
                if serial.input.len() > 1024 {
                    return Err(failure("串口接收缓冲超限"));
                }
            }
            // 单串口响应按序完整写出，超时不重发；write返回只代表本机驱动接收。
            if !serial.output.is_empty() {
                match serial.port.write(&serial.output[serial.written..]) {
                    Ok(0) => return Err(failure("串口写出中断")),
                    Ok(n) => serial.written += n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                    Err(e) => return Err(failure(e)),
                }
                if serial.written == serial.output.len() {
                    frame(ep, "TX", &serial.output, true, false, "driverWritten").await?;
                    serial.output.clear();
                    serial.written = 0;
                } else if serial.send_started.elapsed() > Duration::from_secs(1) {
                    return Err(failure("串口发送超时，结果不确定"));
                }
                return Ok(());
            }
            if serial.input.is_empty() {
                return Ok(());
            }
            let quiet = serial.last_byte.elapsed() >= Duration::from_millis(20);
            let length = expected(&serial.input);
            let complete = length.is_some_and(|n| n <= 256 && serial.input.len() >= n);
            if !complete && !quiet {
                return Ok(());
            }
            let count = if complete {
                length.unwrap()
            } else {
                serial.input.len()
            };
            let request: Vec<_> = serial.input.drain(..count).collect();
            let valid_length = if length.is_some() {
                complete
            } else {
                (4..=256).contains(&request.len())
            };
            let valid = valid_length && rtu_crc(&request) == 0;
            let station = ep.config.units.iter().find(|u| u.enabled).unwrap().station;
            let addressed = valid && (request[0] == station || request[0] == 0);
            let broadcast_write = valid && request[0] == 0 && matches!(request[1], 5 | 6 | 15 | 16);
            frame(
                ep,
                "RX",
                &request,
                valid_length,
                !addressed || (request[0] == 0 && !broadcast_write),
                "received",
            )
            .await?;
            if !addressed || request[0] == 0 && !broadcast_write {
                return Ok(());
            }
            let started = Instant::now();
            let (_, unit) = ep.units.get_mut(&station).unwrap();
            let pdu = &request[1..request.len() - 2];
            let exception = match request[1] {
                8 if request.len() == 8 && request[2..4] == [0, 0] => None,
                17 if request.len() == 4 => None,
                _ => unit.check_pdu(pdu).err(),
            };
            let response = serial
                .responder
                .respond(&request, &mut unit.mapping, exception)
                .map_err(failure)?;
            ep.view.requests = (ep.view.requests.parse::<u64>().unwrap_or(0) + 1).to_string();
            if exception.is_some() {
                ep.view.exceptions =
                    (ep.view.exceptions.parse::<u64>().unwrap_or(0) + 1).to_string();
            }
            ep.view.last_request_micros = started.elapsed().as_micros().to_string();
            revise(ep);
            if !response.is_empty() {
                frame(ep, "TX", &response, true, false, "prepared").await?;
                serial.output = response;
                serial.send_started = Instant::now();
            }
            if broadcast_write {
                let unit = ep.units.get(&station).unwrap().0.clone();
                record(ep, None, SlaveFact::ValuesCommitted { unit }).await?;
            }
            refresh_cached_request(ep, station, pdu)?;
            Ok(())
        })
        .await;
        if let Err(error) = result {
            if serial.pending_output() {
                ep.view.uncertain = true;
            }
            drop(serial);
            ep.view.state = SlaveState::Failed;
            ep.view.error = Some(error.message);
            revise(ep);
        } else {
            ep.serial = Some(serial);
        }
        publish_cache(ep);
    }
}
