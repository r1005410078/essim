mod support;
use libmodbus_rs::{AddressRange, Exception, Mapping, RtuConfig, RtuContext};
use std::{io::Write, time::Duration};
use support::{adu, Pty};
fn config(path: &str) -> RtuConfig {
    RtuConfig::new(path, 19200, 'N', 8, 2, 1, 50, 2).unwrap()
}
#[test]
fn invalid_config_and_missing_device_fail_before_a_live_context() {
    for (path, baud, parity, data, stop, slave, rx, byte) in [
        ("", 19200, 'N', 8, 2, 1, 50, 2),
        ("a\0b", 19200, 'N', 8, 2, 1, 50, 2),
        ("/missing", 0, 'N', 8, 2, 1, 50, 2),
        ("/missing", 19200, 'X', 8, 2, 1, 50, 2),
        ("/missing", 19200, 'N', 7, 2, 1, 50, 2),
        ("/missing", 19200, 'N', 8, 3, 1, 50, 2),
        ("/missing", 19200, 'N', 8, 2, 0, 50, 2),
        ("/missing", 19200, 'N', 8, 2, 248, 50, 2),
        ("/missing", 19200, 'N', 8, 2, 1, 0, 2),
        ("/missing", 19200, 'N', 8, 2, 1, 50, 0),
        ("/missing", 19200, 'N', 8, 2, 1, 50, 3),
    ] {
        assert!(RtuConfig::new(path, baud, parity, data, stop, slave, rx, byte).is_err());
    }
    assert!(RtuContext::open(&config("/definitely/not/a/serial/device")).is_err());
    // connect 已打开普通文件但 tcgetattr 失败的路径，也须正确清理。
    let path = std::env::temp_dir().join(format!("essim-not-serial-{}", std::process::id()));
    std::fs::write(&path, []).unwrap();
    assert!(RtuContext::open(&config(path.to_str().unwrap())).is_err());
    std::fs::remove_file(path).unwrap();
}
#[test]
fn actual_rtu_four_areas_writes_broadcast_and_exception_silence() {
    let mut pty = Pty::new();
    let mut ctx = RtuContext::open(&config(&pty.path)).unwrap();
    let range = Some(AddressRange::new(17, 10).unwrap());
    let mut map = Mapping::new(range, range, range, range).unwrap();
    map.set_discrete(17, true).unwrap();
    map.set_input(17, 0x1234).unwrap();
    let mut exchange = |body: &[u8], expected: &[u8]| {
        pty.send(body);
        let req = ctx.receive().unwrap().unwrap();
        let n = ctx.reply(&req, &mut map).unwrap();
        assert_eq!(pty.read(n), adu(expected));
    };
    exchange(&[1, 5, 0, 17, 255, 0], &[1, 5, 0, 17, 255, 0]);
    exchange(&[1, 1, 0, 17, 0, 1], &[1, 1, 1, 1]);
    exchange(&[1, 15, 0, 17, 0, 10, 2, 0x81, 3], &[1, 15, 0, 17, 0, 10]);
    exchange(&[1, 1, 0, 17, 0, 10], &[1, 1, 2, 0x81, 3]);
    exchange(&[1, 2, 0, 17, 0, 1], &[1, 2, 1, 1]);
    exchange(&[1, 6, 0, 17, 255, 255], &[1, 6, 0, 17, 255, 255]);
    exchange(&[1, 3, 0, 17, 0, 1], &[1, 3, 2, 255, 255]);
    exchange(&[1, 16, 0, 17, 0, 2, 4, 0, 7, 0, 8], &[1, 16, 0, 17, 0, 2]);
    exchange(&[1, 4, 0, 17, 0, 1], &[1, 4, 2, 0x12, 0x34]);
    exchange(&[1, 3, 0, 30, 0, 1], &[1, 0x83, 2]);
    pty.send(&[0, 6, 0, 17, 0, 99]);
    let req = ctx.receive().unwrap().unwrap();
    assert!(req.is_broadcast());
    assert_eq!(ctx.reply(&req, &mut map).unwrap(), 0);
    assert_eq!(map.holding(17).unwrap(), 99);
    assert!(!pty.ready(Duration::from_millis(20)));
    pty.send(&[0, 6, 0, 17, 0, 88]);
    let req = ctx.receive().unwrap().unwrap();
    assert_eq!(
        ctx.reply_exception(&req, Exception::IllegalAddress)
            .unwrap(),
        0
    );
    assert_eq!(map.holding(17).unwrap(), 99);
    assert!(!pty.ready(Duration::from_millis(20)));
}
#[test]
fn crc_partial_foreign_and_back_to_back_frames_recover() {
    let mut pty = Pty::new();
    let mut ctx = RtuContext::open(&config(&pty.path)).unwrap();
    let mut map = Mapping::new(None, None, Some(AddressRange::new(17, 1).unwrap()), None).unwrap();
    let frame = adu(&[1, 3, 0, 17, 0, 1]);
    let mut corrupt = frame.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    pty.wire.write_all(&corrupt).unwrap();
    assert!(ctx.receive().is_err());
    pty.wire.write_all(&frame[..3]).unwrap();
    assert!(ctx.receive().is_err());
    pty.send(&[2, 3, 0, 17, 0, 1]);
    assert!(ctx.receive().unwrap().is_none());
    assert!(ctx.receive().unwrap().is_none()); // 上游等待并忽略另一个站的 confirmation，有限超时
    pty.wire
        .write_all(&[frame.clone(), frame.clone()].concat())
        .unwrap();
    for _ in 0..2 {
        let req = ctx.receive().unwrap().unwrap();
        let n = ctx.reply(&req, &mut map).unwrap();
        assert_eq!(pty.read(n), adu(&[1, 3, 2, 0, 0]));
    }
    // 有限超时后，下一条完整请求仍能处理。
    assert_eq!(
        ctx.receive().unwrap_err().kind(),
        std::io::ErrorKind::TimedOut
    );
}

#[test]
fn nonreading_serial_peer_fails_nonblocking_write_and_drop_returns() {
    let mut pty = Pty::new();
    let mut ctx = RtuContext::open(&config(&pty.path)).unwrap();
    let mut map =
        Mapping::new(None, None, Some(AddressRange::new(17, 125).unwrap()), None).unwrap();
    pty.send(&[1, 3, 0, 17, 0, 125]);
    let req = ctx.receive().unwrap().unwrap();
    let now = std::time::Instant::now();
    let mut full = false;
    for _ in 0..2000 {
        if ctx.reply(&req, &mut map).is_err() {
            full = true;
            break;
        }
    }
    assert!(full, "未填满 PTY 输出队列，测试没有覆盖发送受阻");
    drop(ctx);
    assert!(now.elapsed() < Duration::from_secs(1));
}
#[test]
fn fragmented_valid_frame_is_received() {
    let pty = Pty::new();
    let mut ctx = RtuContext::open(&config(&pty.path)).unwrap();
    let frame = adu(&[1, 3, 0, 17, 0, 1]);
    let mut wire = pty.wire.try_clone().unwrap();
    let writer = std::thread::spawn(move || {
        wire.write_all(&frame[..3]).unwrap();
        std::thread::sleep(Duration::from_micros(200));
        wire.write_all(&frame[3..]).unwrap();
    });
    assert_eq!(
        ctx.receive().unwrap().unwrap().as_bytes(),
        adu(&[1, 3, 0, 17, 0, 1])
    );
    writer.join().unwrap();
    assert!(!pty.ready(Duration::from_millis(5)));
}
