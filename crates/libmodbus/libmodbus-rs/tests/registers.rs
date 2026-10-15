use libmodbus_rs::{HoldingRegisters, TcpContext};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};
#[test]
fn mapping_is_generic_and_checks_bounds() {
    assert!(HoldingRegisters::new(65535, 2).is_err());
    assert!(HoldingRegisters::new(0, 0).is_err());
    let mut m = HoldingRegisters::new(17, 2).unwrap();
    m.set(17, 65535).unwrap();
    assert_eq!(m.get(17).unwrap(), 65535);
    assert!(m.set(19, 1).is_err());
}
#[test]
fn normal_writes_are_allowed_by_the_generic_library() {
    let (socket, mut transport) = UnixStream::pair().unwrap();
    transport
        .set_read_timeout(Some(Duration::from_secs(1)))
        .unwrap();
    let mut ctx = TcpContext::with_socket(socket, Duration::from_millis(100)).unwrap();
    let mut map = HoldingRegisters::new(17, 1).unwrap();
    let request = [0, 1, 0, 0, 0, 6, 1, 6, 0, 17, 255, 255];
    transport.write_all(&request).unwrap();
    let req = ctx.receive().unwrap();
    assert_eq!(ctx.reply(&req, &mut map).unwrap(), 12);
    let mut response = [0; 12];
    transport.read_exact(&mut response).unwrap();
    assert_eq!(response, request);
    assert_eq!(map.get(17).unwrap(), 65535);
}

#[test]
fn four_areas_are_independent_and_bounded() {
    use libmodbus_rs::{AddressRange, Mapping};
    let range = AddressRange::new(17, 2).unwrap();
    assert!(AddressRange::new(0, 0).is_err());
    assert!(AddressRange::new(65535, 2).is_err());
    let mut m = Mapping::new(Some(range), Some(range), Some(range), Some(range)).unwrap();
    assert!(!m.coil(17).unwrap());
    assert!(!m.discrete(17).unwrap());
    assert_eq!(m.holding(17).unwrap(), 0);
    assert_eq!(m.input(17).unwrap(), 0);
    m.set_coil(17, true).unwrap();
    m.set_input(17, 65535).unwrap();
    assert!(!m.discrete(17).unwrap());
    assert_eq!(m.holding(17).unwrap(), 0);
    assert!(m.coil(16).is_err());
    assert!(m.set_discrete(19, true).is_err());
    assert!(m.set_holding(19, 1).is_err());
    assert!(m.input(19).is_err());
    // 末地址合法，完整 u16 和两个位值均可在本地保存。
    let last = AddressRange::new(65535, 1).unwrap();
    let mut edge = Mapping::new(Some(last), Some(last), Some(last), Some(last)).unwrap();
    edge.set_coil(65535, true).unwrap();
    edge.set_discrete(65535, true).unwrap();
    edge.set_holding(65535, 65535).unwrap();
    edge.set_input(65535, 65535).unwrap();
    assert!(edge.coil(65535).unwrap());
    assert!(edge.discrete(65535).unwrap());
    assert_eq!(edge.holding(65535).unwrap(), 65535);
    assert_eq!(edge.input(65535).unwrap(), 65535);
    let empty = Mapping::new(None, None, None, None).unwrap();
    assert!(empty.coil(0).is_err());
    assert!(empty.discrete(0).is_err());
    assert!(empty.holding(0).is_err());
    assert!(empty.input(0).is_err());
}

#[test]
fn four_area_protocol_reads_and_single_multiple_writes() {
    use libmodbus_rs::{AddressRange, Mapping};
    let range = AddressRange::new(17, 10).unwrap();
    let mut map = Mapping::new(Some(range), Some(range), Some(range), Some(range)).unwrap();
    map.set_discrete(17, true).unwrap();
    map.set_input(17, 0x1234).unwrap();
    let (socket, mut wire) = UnixStream::pair().unwrap();
    wire.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    let mut ctx = TcpContext::with_socket(socket, Duration::from_millis(100)).unwrap();
    let mut exchange = |pdu: &[u8], expected: &[u8]| {
        let mut frame = vec![0, 1, 0, 0, 0, (1 + pdu.len()) as u8, 1];
        frame.extend_from_slice(pdu);
        wire.write_all(&frame).unwrap();
        let req = ctx.receive().unwrap();
        let n = ctx.reply(&req, &mut map).unwrap();
        let mut response = vec![0; n];
        wire.read_exact(&mut response).unwrap();
        let mut wanted = vec![0, 1, 0, 0, 0, (1 + expected.len()) as u8, 1];
        wanted.extend_from_slice(expected);
        assert_eq!(response, wanted);
    };
    exchange(&[5, 0, 17, 255, 0], &[5, 0, 17, 255, 0]);
    exchange(&[1, 0, 17, 0, 2], &[1, 1, 1]);
    exchange(&[15, 0, 17, 0, 2, 1, 2], &[15, 0, 17, 0, 2]);
    exchange(&[1, 0, 17, 0, 2], &[1, 1, 2]);
    // 跨字节的 10 个线圈，按低位优先打包；第二字节仅使用低 2 位。
    exchange(&[15, 0, 17, 0, 10, 2, 0x81, 3], &[15, 0, 17, 0, 10]);
    exchange(&[1, 0, 17, 0, 10], &[1, 2, 0x81, 3]);
    exchange(&[5, 0, 17, 0, 1], &[0x85, 3]); // 非法线圈写入值
    exchange(&[1, 0, 17, 0, 1], &[1, 1, 1]);
    exchange(&[2, 0, 17, 0, 2], &[2, 1, 1]);
    exchange(&[6, 0, 17, 255, 255], &[6, 0, 17, 255, 255]);
    exchange(&[3, 0, 17, 0, 1], &[3, 2, 255, 255]);
    exchange(&[16, 0, 17, 0, 2, 4, 0, 7, 0, 8], &[16, 0, 17, 0, 2]);
    exchange(&[3, 0, 17, 0, 2], &[3, 4, 0, 7, 0, 8]);
    exchange(&[4, 0, 17, 0, 1], &[4, 2, 0x12, 0x34]);
    exchange(&[1, 0, 27, 0, 1], &[0x81, 2]);
    exchange(&[2, 0, 27, 0, 1], &[0x82, 2]);
    exchange(&[3, 0, 27, 0, 1], &[0x83, 2]);
    exchange(&[4, 0, 27, 0, 1], &[0x84, 2]);
}
