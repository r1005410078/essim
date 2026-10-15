//! 普通寄存器读写示例；不依赖 ESSim 或 Tauri。
use libmodbus_rs::{HoldingRegisters, TcpContext};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};
fn main() -> std::io::Result<()> {
    let (socket, mut wire) = UnixStream::pair()?;
    let mut context = TcpContext::with_socket(socket, Duration::from_millis(100))?;
    let mut mapping = HoldingRegisters::new(17, 1)?;
    wire.write_all(&[0, 1, 0, 0, 0, 6, 1, 6, 0, 17, 255, 255])?;
    let request = context.receive()?;
    let n = context.reply(&request, &mut mapping)?;
    let mut reply = vec![0; n];
    wire.read_exact(&mut reply)?;
    assert_eq!(mapping.get(17)?, 65535);
    println!(
        "address=17 value={} response={:02X?}",
        mapping.get(17)?,
        reply
    );
    Ok(())
}
