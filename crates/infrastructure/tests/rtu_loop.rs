#[path = "../../libmodbus/libmodbus-rs/tests/support/mod.rs"]
mod support;
use essim_core::{
    application::simulator::SimulatorService,
    domain::modbus::{Communication, Endpoint, RtuEndpoint, State},
};
use essim_infrastructure::ModbusSimulator;
use std::{
    io::Write,
    time::{Duration, Instant},
};
use support::{adu, Pty};
fn endpoint(path: &str) -> RtuEndpoint {
    RtuEndpoint::new(path, 19200, 'N', 8, 2, 7, 50, 2).unwrap()
}
#[test]
fn serial_read_update_readonly_broadcast_mode_isolation_and_restart() {
    let mut pty = Pty::new();
    let app = SimulatorService(ModbusSimulator::new());
    app.add_unit(2).unwrap();
    app.update_unit(2, "soc", 720.0).unwrap();
    let ep = endpoint(&pty.path);
    app.configure(Communication::Rtu(ep.clone())).unwrap();
    app.update_unit(7, "soc", 810.0).unwrap();
    app.start_rtu(ep.clone()).unwrap();
    assert_eq!(app.add_unit(8).unwrap_err().code, "RTU_SINGLE_UNIT");
    assert_eq!(
        app.configure(Communication::Tcp(
            Endpoint::new("127.0.0.1", 1502).unwrap()
        ))
        .unwrap_err()
        .code,
        "ALREADY_RUNNING"
    );
    pty.send(&[7, 3, 0, 100, 0, 1]);
    assert_eq!(pty.read(7), adu(&[7, 3, 2, 3, 42]));
    app.update_unit(7, "soc", 700.0).unwrap();
    pty.send(&[7, 6, 0, 100, 0, 12]);
    assert_eq!(pty.read(5), adu(&[7, 0x86, 2]));
    pty.send(&[0, 6, 0, 100, 0, 12]);
    assert!(!pty.ready(Duration::from_millis(60)));
    pty.send(&[7, 3, 0, 100, 0, 1]);
    assert_eq!(pty.read(7), adu(&[7, 3, 2, 2, 188]));
    let now = Instant::now();
    app.stop().unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
    let s = app
        .configure(Communication::Tcp(
            Endpoint::new("127.0.0.1", 1502).unwrap(),
        ))
        .unwrap();
    assert_eq!(s.units.len(), 2);
    assert_eq!(
        s.units.values().map(|v| v.raw()).collect::<Vec<_>>(),
        [650, 720]
    );
    app.start_rtu(ep).unwrap();
    pty.send(&[7, 3, 0, 100, 0, 1]);
    assert_eq!(pty.read(7), adu(&[7, 3, 2, 2, 188]));
    pty.wire.write_all(&[7, 3, 0]).unwrap();
    let now = Instant::now();
    app.stop().unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
}
#[test]
fn missing_device_fails_and_configuration_is_atomic() {
    let app = SimulatorService(ModbusSimulator::new());
    let ep = endpoint("/no/such/serial/device");
    assert!(app.start_rtu(ep.clone()).is_err());
    assert_eq!(app.snapshot().state, State::Failed);
    app.stop().unwrap();
    let before = app.snapshot();
    let mut invalid = ep;
    invalid.baud = 12345;
    assert!(app.configure(Communication::Rtu(invalid)).is_err());
    assert_eq!(app.snapshot().communication, before.communication);
}

#[test]
fn serial_disconnect_does_not_block_stop() {
    // 拔出用关闭 PTY master 模拟软件层断开，不宣称是 USB 驱动硬件拔出。
    let pty = Pty::new();
    let app = SimulatorService(ModbusSimulator::new());
    app.start_rtu(endpoint(&pty.path)).unwrap();
    drop(pty);
    let deadline = Instant::now() + Duration::from_secs(2);
    while app.snapshot().state != State::Failed && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(app.snapshot().state, State::Failed);
    let now = Instant::now();
    app.stop().unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
}
#[test]
fn slow_maximum_frame_and_continuous_bad_crc_have_bounded_stop() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let pty = Pty::new();
    let app = SimulatorService(ModbusSimulator::new());
    app.start_rtu(endpoint(&pty.path)).unwrap();
    let mut wire = pty.wire.try_clone().unwrap();
    let finish = Arc::new(AtomicBool::new(false));
    let flag = finish.clone();
    let writer = std::thread::spawn(move || {
        // FC10 宣称 246 个数据字节，1ms 持续送达，使字节间超时不断被续期。
        let mut frame = vec![7, 16, 0, 100, 0, 123, 246];
        frame.extend([0; 246]);
        frame.extend([0; 2]);
        for byte in frame {
            if flag.load(Ordering::Acquire) {
                break;
            }
            if wire.write_all(&[byte]).is_err() {
                break;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    std::thread::sleep(Duration::from_millis(10));
    let now = Instant::now();
    app.stop().unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
    finish.store(true, Ordering::Release);
    writer.join().unwrap();
    // 持续无效帧依然必须检查命令，日志有界。
    app.start_rtu(endpoint(&pty.path)).unwrap();
    let mut wire = pty.wire.try_clone().unwrap();
    let writer = std::thread::spawn(move || {
        for _ in 0..100 {
            let _ = wire.write_all(&[7, 3, 0, 100, 0, 1, 0, 0]);
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    std::thread::sleep(Duration::from_millis(15));
    let now = Instant::now();
    app.stop().unwrap();
    assert!(now.elapsed() < Duration::from_secs(1));
    writer.join().unwrap();
    assert!(app.snapshot().logs.len() <= 128);
}
