use essim_core::{application::simulator::SimulatorService, domain::modbus::State};
use essim_infrastructure::ModbusSimulator;
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::{Duration, Instant},
};
fn port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
fn connect(port: u16) -> TcpStream {
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    s
}
fn read_frame(s: &mut TcpStream) -> Vec<u8> {
    let mut h = [0; 6];
    s.read_exact(&mut h).unwrap();
    let mut v = h.to_vec();
    v.resize(6 + u16::from_be_bytes([h[4], h[5]]) as usize, 0);
    s.read_exact(&mut v[6..]).unwrap();
    v
}
fn request(s: &mut TcpStream, hex: &[u8]) -> Vec<u8> {
    s.write_all(hex).unwrap();
    read_frame(s)
}
const READ: [u8; 12] = [0, 1, 0, 0, 0, 6, 1, 3, 0, 100, 0, 1];
#[test]
fn real_read_update_policy_and_lifecycle() {
    let app = SimulatorService(ModbusSimulator::new());
    let p = port();
    assert!(app.update("wrong", 700.0).is_err());
    app.start("127.0.0.1", p).unwrap();
    assert_eq!(
        app.start("127.0.0.1", p).unwrap_err().code,
        "ALREADY_RUNNING"
    );
    let mut s = connect(p);
    assert_eq!(request(&mut s, &READ), [0, 1, 0, 0, 0, 5, 1, 3, 2, 2, 138]);
    app.update("soc", 700.0).unwrap();
    assert_eq!(&request(&mut s, &READ)[9..], &[2, 188]);
    assert!(app.update("soc", 1001.0).is_err());
    for (fc, addr, qty, exception) in [
        (3, 101, 1, 2),
        (3, 100, 0, 3),
        (3, 100, 126, 3),
        (6, 100, 700, 2),
        (4, 100, 1, 1),
    ] {
        let mut r = READ;
        r[7] = fc;
        r[9] = addr;
        r[10..12].copy_from_slice(&(qty as u16).to_be_bytes());
        assert_eq!(&request(&mut s, &r)[7..], &[fc | 128, exception]);
    }
    assert_eq!(
        &request(&mut s, &[0, 2, 0, 0, 0, 9, 1, 16, 0, 100, 0, 1, 2, 2, 188])[7..],
        &[144, 2]
    );
    let before = Instant::now();
    app.stop().unwrap();
    assert!(before.elapsed() < Duration::from_secs(1));
    assert_eq!(app.stop().unwrap().state, State::Stopped);
    app.start("127.0.0.1", p).unwrap();
    let mut s = connect(p);
    assert_eq!(&request(&mut s, &READ)[9..], &[2, 188]);
    app.stop().unwrap();
    TcpListener::bind(("127.0.0.1", p)).unwrap();
}
#[test]
fn stream_boundaries_bad_headers_extra_client_and_stop() {
    let app = SimulatorService(ModbusSimulator::new());
    let p = port();
    app.start("127.0.0.1", p).unwrap();
    let mut s = connect(p);
    s.write_all(&READ[..5]).unwrap();
    std::thread::sleep(Duration::from_millis(25));
    s.write_all(&READ[5..]).unwrap();
    assert_eq!(read_frame(&mut s).len(), 11);
    s.write_all(&[READ, READ].concat()).unwrap();
    assert_eq!(read_frame(&mut s).len(), 11);
    assert_eq!(read_frame(&mut s).len(), 11);
    let mut extra = connect(p);
    extra.write_all(&READ).unwrap();
    let mut b = [0; 1];
    assert!(matches!(extra.read(&mut b), Ok(0) | Err(_)));
    drop(s);
    std::thread::sleep(Duration::from_millis(30));
    for mut bad in [READ.to_vec(), READ.to_vec(), READ.to_vec()] {
        // Non-zero protocol ID must close only this client.
        bad[3] = 1;
        let mut s = connect(p);
        s.write_all(&bad).unwrap();
        assert!(matches!(s.read(&mut b), Ok(0) | Err(_)));
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut s = connect(p);
    s.write_all(&READ[..8]).unwrap();
    let before = Instant::now();
    app.stop().unwrap();
    assert!(before.elapsed() < Duration::from_secs(1));
    app.start("127.0.0.1", p).unwrap();
    app.stop().unwrap();
}
#[test]
fn bind_failure_recovers_unit_id_and_malformed_length_close() {
    let app = SimulatorService(ModbusSimulator::new());
    let occupied = TcpListener::bind("127.0.0.1:0").unwrap();
    let p = occupied.local_addr().unwrap().port();
    assert_eq!(app.start("127.0.0.1", p).unwrap_err().code, "START_FAILED");
    assert_eq!(app.snapshot().state, State::Failed);
    drop(occupied);
    app.start("127.0.0.1", p).unwrap();
    let mut unit = READ;
    unit[6] = 2;
    let mut badlen = READ;
    badlen[5] = 255;
    let mut short = READ;
    short[5] = 5;
    let mut oversized = READ;
    oversized[5] = 7;
    for frame in [unit, badlen, short, oversized] {
        let mut s = connect(p);
        s.write_all(&frame).unwrap();
        let mut b = [0; 1];
        assert!(matches!(s.read(&mut b), Ok(0) | Err(_)));
        std::thread::sleep(Duration::from_millis(20));
    }
    let mut s = connect(p);
    assert_eq!(&request(&mut s, &READ)[9..], &[2, 138]);
    drop(s);
    app.stop().unwrap();
}
#[test]
fn subscriptions_are_bounded_and_disconnected_receivers_reclaimed() {
    use essim_core::application::simulator::SimulatorPort;
    let engine = ModbusSimulator::new();
    for _ in 0..100 {
        let rx = engine.subscribe();
        assert_eq!(rx.recv().unwrap().sequence, engine.snapshot().sequence);
        drop(rx);
        engine
            .update(essim_core::domain::modbus::Soc::INITIAL)
            .unwrap();
    }
    let rx = engine.subscribe();
    for i in 0..200 {
        engine
            .update(essim_core::domain::modbus::Soc::new(i as f64).unwrap())
            .unwrap();
    }
    assert!(rx.try_iter().count() <= 64);
    assert_eq!(engine.snapshot().soc.raw(), 199);
}
#[test]
fn nonreading_client_cannot_block_stop() {
    let app = SimulatorService(ModbusSimulator::new());
    let p = port();
    app.start("127.0.0.1", p).unwrap();
    let mut s = connect(p);
    s.write_all(&READ.repeat(80)).unwrap();
    std::thread::sleep(Duration::from_millis(20));
    let start = Instant::now();
    app.stop().unwrap();
    assert!(start.elapsed() < Duration::from_secs(1));
    TcpListener::bind(("127.0.0.1", p)).unwrap();
}

#[test]
fn units_can_be_added_live_and_keep_independent_values_after_restart() {
    let app = SimulatorService(ModbusSimulator::new());
    for id in [0, 248, 256] {
        assert_eq!(app.add_unit(id).unwrap_err().code, "INVALID_UNIT_ID");
    }
    assert_eq!(app.add_unit(1).unwrap_err().code, "UNIT_EXISTS");
    app.add_unit(2).unwrap();
    app.update_unit(2, "soc", 720.0).unwrap();
    let p = port();
    app.start("127.0.0.1", p).unwrap();
    let mut client = connect(p);
    let mut read = READ;
    read[6] = 2;
    assert_eq!(&request(&mut client, &read)[9..], &[2, 208]);
    assert_eq!(&request(&mut client, &READ)[9..], &[2, 138]);
    app.add_unit(3).unwrap();
    assert_eq!(app.add_unit(3).unwrap_err().code, "UNIT_EXISTS");
    assert_eq!(
        app.update_unit(4, "soc", 700.0).unwrap_err().code,
        "UNIT_NOT_FOUND"
    );
    assert_eq!(
        app.update_unit(3, "soc", 1001.0).unwrap_err().code,
        "INVALID_VALUE"
    );
    let snapshot = app.update_unit(3, "soc", 810.0).unwrap();
    assert_eq!(snapshot.units.len(), 3);
    read[6] = 3;
    assert_eq!(&request(&mut client, &read)[9..], &[3, 42]);
    assert_eq!(&request(&mut client, &READ)[9..], &[2, 138]);
    app.stop().unwrap();
    app.start("127.0.0.1", p).unwrap();
    let mut client = connect(p);
    assert_eq!(&request(&mut client, &read)[9..], &[3, 42]);
    read[6] = 4;
    client.write_all(&read).unwrap();
    assert_eq!(client.read(&mut [0; 1]).unwrap(), 0);
    app.stop().unwrap();
}
