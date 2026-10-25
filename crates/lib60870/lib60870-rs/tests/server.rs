use lib60870_rs::{Asdu, Cause, Point, Quality, Server};
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant},
};
fn free_address() -> SocketAddr {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
}
fn apdu(socket: &mut TcpStream) -> Vec<u8> {
    let mut header = [0; 2];
    socket.read_exact(&mut header).unwrap();
    assert_eq!(header[0], 0x68);
    let mut body = vec![0; header[1] as usize];
    socket.read_exact(&mut body).unwrap();
    body
}
#[test]
fn wire_interrogation_scoped_replies_and_drop_release_listener() {
    // 所有地址和值都是测试约定；报文字节依据 IEC 104 的 STARTDT 和 C_IC_NA_1 格式。
    let address = free_address();
    let (ready, started) = mpsc::sync_channel(1);
    let done = Arc::new(AtomicBool::new(false));
    let stop = done.clone();
    let thread = std::thread::spawn(move || {
        let mut server = Server::bind(address, |request| {
            assert_eq!(request.type_id(), 100);
            assert_eq!(request.common_address(), 47);
            assert_eq!(request.interrogation_qualifier().unwrap(), 20);
            request.confirm(false).unwrap();
            let mut data = Asdu::new(47, Cause::Interrogated, false).unwrap();
            data.add(Point::measured(100, 12.5, Quality::GOOD).unwrap())
                .unwrap();
            request.send(&data).unwrap();
            request.finish().unwrap();
            true
        })
        .unwrap();
        ready.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(4);
        while !stop.load(Ordering::Acquire) && Instant::now() < until {
            server.tick().unwrap();
            std::thread::sleep(Duration::from_millis(1));
        }
    });
    started.recv_timeout(Duration::from_secs(3)).unwrap();
    let mut peer = TcpStream::connect(address).unwrap();
    peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
    peer.write_all(&[0x68, 4, 7, 0, 0, 0]).unwrap();
    assert_eq!(apdu(&mut peer), [11, 0, 0, 0]);
    peer.write_all(&[0x68, 14, 0, 0, 0, 0, 100, 1, 6, 0, 47, 0, 0, 0, 0, 20])
        .unwrap();
    let mut received = vec![];
    while received.len() < 3 {
        let frame = apdu(&mut peer);
        if frame[0] & 1 == 0 {
            received.push(frame[4..].to_vec());
        }
    }
    assert_eq!(received[0], [100, 1, 7, 0, 47, 0, 0, 0, 0, 20]);
    assert_eq!(
        received[1],
        [13, 1, 20, 0, 47, 0, 100, 0, 0, 0, 0, 72, 65, 0]
    );
    assert_eq!(received[2], [100, 1, 10, 0, 47, 0, 0, 0, 0, 20]);
    done.store(true, Ordering::Release);
    thread.join().unwrap();
    drop(peer);
    let _rebound = TcpListener::bind(address).unwrap();
}
#[test]
fn callback_panic_becomes_error_without_unwinding_into_c() {
    let address = free_address();
    let mut server = Server::bind(address, |_| panic!("injected callback panic")).unwrap();
    let (release, wait) = mpsc::sync_channel(1);
    let client = std::thread::spawn(move || {
        let mut peer = TcpStream::connect(address).unwrap();
        peer.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        peer.write_all(&[0x68, 4, 7, 0, 0, 0]).unwrap();
        assert_eq!(apdu(&mut peer), [11, 0, 0, 0]);
        peer.write_all(&[0x68, 14, 0, 0, 0, 0, 100, 1, 6, 0, 47, 0, 0, 0, 0, 20])
            .unwrap();
        wait.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    let until = Instant::now() + Duration::from_secs(3);
    let error = loop {
        match server.tick() {
            Err(e) => break e,
            Ok(()) => {
                assert!(Instant::now() < until);
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    };
    assert!(error.to_string().contains("panic"));
    assert!(!server.is_running());
    release.send(()).unwrap();
    client.join().unwrap();
    drop(server);
    let _rebound = TcpListener::bind(address).unwrap();
}
#[test]
fn occupied_port_and_zero_port_are_reported_without_resource_leak() {
    let busy = TcpListener::bind("127.0.0.1:0").unwrap();
    assert!(Server::bind(busy.local_addr().unwrap(), |_| false).is_err());
    assert!(Server::bind("127.0.0.1:0".parse().unwrap(), |_| false).is_err());
}

#[test]
fn outgoing_queue_refuses_overflow_instead_of_replacing_oldest_event() {
    let mut server = Server::bind(free_address(), |_| false).unwrap();
    let mut data = Asdu::new(47, Cause::Spontaneous, false).unwrap();
    data.add(Point::single(100, true, Quality::GOOD).unwrap())
        .unwrap();
    for _ in 0..64 {
        server.enqueue(&data).unwrap();
    }
    assert!(server.enqueue(&data).is_err());
    assert_eq!(server.queued(), 64);
    server.stop();
    assert!(server.enqueue(&data).is_err());
}
