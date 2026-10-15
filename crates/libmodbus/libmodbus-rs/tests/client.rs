use libmodbus_rs::ModbusClient;
use std::{
    io::{Read, Write},
    net::TcpListener,
    time::Duration,
};
const WAIT: Duration = Duration::from_millis(100);
#[test]
fn rejects_invalid_ranges_without_sending_and_preserves_exception_session() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut req = [0; 12];
        s.read_exact(&mut req).unwrap();
        assert_eq!(&req[6..], &[7, 3, 0, 100, 0, 1]);
        let mut response = req[..7].to_vec();
        response[5] = 3;
        response.extend([0x83, 2]);
        s.write_all(&response).unwrap();
        s.read_exact(&mut req).unwrap();
        response = req[..7].to_vec();
        response[5] = 5;
        response.extend([3, 2, 0x12, 0x34]);
        // 正常分段响应不能被当成两条消息。
        s.write_all(&response[..8]).unwrap();
        s.write_all(&response[8..]).unwrap();
    });
    let mut c = ModbusClient::tcp(addr, 7, WAIT, WAIT).unwrap();
    assert!(c.read_holding(0, 0).is_err());
    assert!(c.read_holding(0, 126).is_err());
    assert!(c.read_coil(0, 2001).is_err());
    assert!(c.read_input(65535, 2).is_err());
    assert!(c.write_coils(0, &vec![true; 1969]).is_err());
    assert!(c.write_holdings(0, &vec![1; 124]).is_err());
    let e = c.read_holding(100, 1).unwrap_err();
    assert_eq!(libmodbus_rs::exception_code(&e), Some(2));
    assert_eq!(c.read_holding(100, 1).unwrap(), [0x1234]);
    worker.join().unwrap();
}
#[test]
fn timeout_closes_session_and_does_not_retry_a_write() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        let (mut s, _) = listener.accept().unwrap();
        s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        let mut req = [0; 12];
        s.read_exact(&mut req).unwrap();
        assert_eq!(req[7], 6);
        let mut b = [0];
        assert_eq!(s.read(&mut b).unwrap(), 0);
    });
    let mut c = ModbusClient::tcp(addr, 1, WAIT, WAIT).unwrap();
    assert_eq!(
        c.write_holding(10, 42).unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
    assert_eq!(
        c.read_holding(10, 1).unwrap_err().kind(),
        std::io::ErrorKind::NotConnected
    );
    worker.join().unwrap();
}
mod support;
#[test]
fn rtu_reads_four_areas_writes_and_switches_station() {
    let mut pty = support::Pty::new();
    let cfg = libmodbus_rs::RtuConfig::new(&pty.path, 19200, 'N', 8, 2, 7, 50, 2).unwrap();
    let mut client = ModbusClient::rtu(&cfg, WAIT).unwrap();
    assert!(client.set_unit(0).is_err());
    assert!(client.set_unit(248).is_err());
    let (done, finished) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        for (request, response) in [
            (vec![7, 1, 0, 100, 0, 2], vec![7, 1, 1, 1]),
            (vec![7, 2, 0, 100, 0, 2], vec![7, 2, 1, 2]),
            (vec![7, 3, 0, 100, 0, 2], vec![7, 3, 4, 0, 42, 0, 43]),
            (vec![7, 4, 0, 100, 0, 1], vec![7, 4, 2, 0, 99]),
            (vec![7, 5, 0, 100, 255, 0], vec![7, 5, 0, 100, 255, 0]),
            (vec![7, 6, 0, 100, 0, 12], vec![7, 6, 0, 100, 0, 12]),
            (vec![7, 15, 0, 100, 0, 2, 1, 2], vec![7, 15, 0, 100, 0, 2]),
            (
                vec![7, 16, 0, 100, 0, 2, 4, 0, 55, 0, 56],
                vec![7, 16, 0, 100, 0, 2],
            ),
            (vec![8, 3, 0, 100, 0, 1], vec![8, 0x83, 2]),
        ] {
            assert_eq!(pty.read(request.len() + 2), support::adu(&request));
            pty.send(&response);
        }
        finished.recv().unwrap();
    });
    assert_eq!(client.read_coil(100, 2).unwrap(), [true, false]);
    assert_eq!(client.read_discrete(100, 2).unwrap(), [false, true]);
    assert_eq!(client.read_holding(100, 2).unwrap(), [42, 43]);
    assert_eq!(client.read_input(100, 1).unwrap(), [99]);
    client.write_coil(100, true).unwrap();
    client.write_holding(100, 12).unwrap();
    client.write_coils(100, &[false, true]).unwrap();
    client.write_holdings(100, &[55, 56]).unwrap();
    client.set_unit(8).unwrap();
    assert_eq!(
        libmodbus_rs::exception_code(&client.read_holding(100, 1).unwrap_err()),
        Some(2)
    );
    done.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
fn corrupt_rtu_reply_closes_the_channel() {
    let mut pty = support::Pty::new();
    let cfg = libmodbus_rs::RtuConfig::new(&pty.path, 19200, 'N', 8, 2, 7, 50, 2).unwrap();
    let mut client = ModbusClient::rtu(&cfg, WAIT).unwrap();
    let (done, finished) = std::sync::mpsc::channel();
    let worker = std::thread::spawn(move || {
        pty.read(8);
        pty.wire.write_all(&[7, 3, 2, 0, 1, 0, 0]).unwrap();
        finished.recv().unwrap();
    });
    assert!(client.read_holding(100, 1).is_err());
    assert_eq!(
        client.read_holding(100, 1).unwrap_err().kind(),
        std::io::ErrorKind::NotConnected
    );
    done.send(()).unwrap();
    worker.join().unwrap();
}
#[test]
fn wrong_transaction_function_and_truncated_reply_are_rejected() {
    for mode in 0..3 {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let worker = std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            s.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut req = [0; 12];
            s.read_exact(&mut req).unwrap();
            let mut response = req[..7].to_vec();
            response[5] = 5;
            response.extend([3, 2, 0, 42]);
            match mode {
                0 => response[1] ^= 1,
                1 => response[7] = 4,
                _ => response.truncate(9),
            }
            s.write_all(&response).unwrap();
        });
        let mut c = ModbusClient::tcp(addr, 7, WAIT, WAIT).unwrap();
        assert!(c.read_holding(100, 1).is_err());
        assert_eq!(
            c.read_holding(100, 1).unwrap_err().kind(),
            std::io::ErrorKind::NotConnected
        );
        worker.join().unwrap();
    }
}
#[test]
fn invalid_timeouts_fail_before_open_and_rtu_timeout_releases_serial() {
    let addr = "127.0.0.1:0".parse().unwrap();
    assert_eq!(
        ModbusClient::tcp(addr, 1, Duration::ZERO, WAIT)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        ModbusClient::tcp(addr, 1, WAIT, Duration::ZERO)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let missing = libmodbus_rs::RtuConfig::new("/missing", 19200, 'N', 8, 2, 7, 10, 1).unwrap();
    assert_eq!(
        ModbusClient::rtu(&missing, Duration::ZERO)
            .err()
            .unwrap()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    let mut pty = support::Pty::new();
    let config = libmodbus_rs::RtuConfig::new(&pty.path, 19200, 'N', 8, 2, 7, 10, 1).unwrap();
    let mut client = ModbusClient::rtu(&config, WAIT).unwrap();
    assert_eq!(
        client.read_holding(100, 1).unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
    assert_eq!(pty.read(8), support::adu(&[7, 3, 0, 100, 0, 1]));
    assert_eq!(
        client.read_holding(100, 1).unwrap_err().kind(),
        std::io::ErrorKind::NotConnected
    );
    // 超时已经关闭原 context，可重新打开；不复用故障对象，不自动重发。
    let next = ModbusClient::rtu(&config, WAIT).unwrap();
    drop(next);
}
