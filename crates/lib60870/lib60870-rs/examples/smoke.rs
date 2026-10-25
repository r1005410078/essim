//! 使用 Rust 安全封装启动真实 104 服务端，以独立原始字节客户端核对协议子集。
//! CA=47、遥测 IOA=100、单点 IOA=200 及数值均为教学约定，不是标准设备点表。
//! 客户端仅为此示例的测试驱动，不提供产品级 104 客户端实现。
use lib60870_rs::{runtime_version, Asdu, Cause, Point, Quality, Request, Server};
use std::{
    cell::{Cell, RefCell},
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    rc::Rc,
    thread,
    time::{Duration, Instant},
};

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn single(cause: Cause, value: bool) -> io::Result<Asdu> {
    let mut asdu = Asdu::new(47, cause, false)?;
    asdu.add(Point::single(200, value, Quality::GOOD)?)?;
    Ok(asdu)
}

fn measured(cause: Cause, value: f32) -> io::Result<Asdu> {
    let mut asdu = Asdu::new(47, cause, false)?;
    asdu.add(Point::measured(100, value, Quality::GOOD)?)?;
    Ok(asdu)
}

fn interrogate(request: &mut Request<'_>) -> io::Result<bool> {
    if request.type_id() != 100 {
        return Ok(false);
    }
    let accepted = request.cause() == 6
        && request.common_address() == 47
        && request.interrogation_qualifier()? == 20;
    request.confirm(!accepted)?;
    if accepted {
        request.send(&single(Cause::Interrogated, true)?)?;
        request.send(&measured(Cause::Interrogated, 12.5)?)?;
        request.finish()?;
    }
    Ok(accepted)
}

fn read_apdu(peer: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut header = [0; 2];
    peer.read_exact(&mut header)?;
    if header[0] != 0x68 || !(4..=253).contains(&header[1]) {
        return Err(invalid("APDU 起始符或长度错误"));
    }
    let mut body = vec![0; header[1] as usize];
    peer.read_exact(&mut body)?;
    Ok(body)
}

fn expect(actual: &[u8], expected: &[u8], step: &str) -> io::Result<()> {
    if actual != expected {
        return Err(invalid(&format!(
            "{step} 不匹配：actual={actual:02x?}, expected={expected:02x?}"
        )));
    }
    println!("PASS {step}");
    Ok(())
}

fn read_information(peer: &mut TcpStream) -> io::Result<Vec<u8>> {
    // 允许协议正常插入 S 帧；限制次数，避免错误驱动无限等待。
    for _ in 0..32 {
        let body = read_apdu(peer)?;
        if body[0] & 1 == 0 {
            return Ok(body[4..].to_vec());
        }
        if body[0] & 3 != 1 {
            return Err(invalid("等待 I 帧时收到意外 U 帧"));
        }
    }
    Err(invalid("等待信息帧超过示例上限"))
}

fn client(address: SocketAddr) -> io::Result<()> {
    let mut peer = TcpStream::connect_timeout(&address, Duration::from_secs(1))?;
    peer.set_read_timeout(Some(Duration::from_secs(1)))?;
    peer.set_write_timeout(Some(Duration::from_secs(1)))?;
    peer.write_all(&[0x68, 4, 7, 0, 0, 0])?;
    expect(&read_apdu(&mut peer)?, &[11, 0, 0, 0], "STARTDT")?;

    // 分段交付一次 C_IC_NA_1 总召；字节格式与包内安全编码逻辑独立。
    let gi = [0x68, 14, 0, 0, 0, 0, 100, 1, 6, 0, 47, 0, 0, 0, 0, 20];
    peer.write_all(&gi[..5])?;
    thread::sleep(Duration::from_millis(10));
    peer.write_all(&gi[5..])?;
    for (step, expected) in [
        ("总召 ACT_CON", vec![100, 1, 7, 0, 47, 0, 0, 0, 0, 20]),
        ("总召单点 true", vec![1, 1, 20, 0, 47, 0, 200, 0, 0, 1]),
        (
            "总召遥测 12.5",
            vec![13, 1, 20, 0, 47, 0, 100, 0, 0, 0, 0, 72, 65, 0],
        ),
        ("总召 ACT_TERM", vec![100, 1, 10, 0, 47, 0, 0, 0, 0, 20]),
        ("主动上送单点 false", vec![1, 1, 3, 0, 47, 0, 200, 0, 0, 0]),
        (
            "主动上送遥测 14.0",
            vec![13, 1, 3, 0, 47, 0, 100, 0, 0, 0, 0, 96, 65, 0],
        ),
    ] {
        expect(&read_information(&mut peer)?, &expected, step)?;
    }
    peer.write_all(&[0x68, 4, 1, 0, 12, 0])?; // S 帧确认六条 I 帧。

    // 本示例只支持站总召，分组总召 QOI=21 明确负确认，无自动重试。
    peer.write_all(&[0x68, 14, 2, 0, 12, 0, 100, 1, 6, 0, 47, 0, 0, 0, 0, 21])?;
    expect(
        &read_information(&mut peer)?,
        &[100, 1, 0x47, 0, 47, 0, 0, 0, 0, 21],
        "不支持的总召负确认",
    )?;
    peer.write_all(&[0x68, 4, 1, 0, 14, 0])?;
    peer.write_all(&[0x68, 4, 0x43, 0, 0, 0])?;
    expect(&read_apdu(&mut peer)?, &[0x83, 0, 0, 0], "TESTFR")?;
    peer.write_all(&[0x68, 4, 0x13, 0, 0, 0])?;
    expect(&read_apdu(&mut peer)?, &[0x23, 0, 0, 0], "STOPDT")?;
    Ok(())
}

fn main() -> io::Result<()> {
    // Server 首版要求非零端口；先由系统选择临时端口，再让封装实际监听。
    let reservation = TcpListener::bind("127.0.0.1:0")?;
    let address = reservation.local_addr()?;
    drop(reservation);
    let queried = Rc::new(Cell::new(false));
    let completed = queried.clone();
    let callback_error = Rc::new(RefCell::new(None));
    let failure = callback_error.clone();
    let mut server = Server::bind(address, move |request| match interrogate(request) {
        Ok(accepted) => {
            if accepted {
                completed.set(true);
            }
            // 类型100的负确认也由本处理器完成，不让上游再发未知类型响应。
            request.type_id() == 100
        }
        Err(error) => {
            *failure.borrow_mut() = Some(error);
            true
        }
    })?;
    println!("lib60870 runtime={:?}, listen={address}", runtime_version());
    let peer = thread::spawn(move || client(address));
    let deadline = Instant::now() + Duration::from_secs(5);
    let server_result = (|| {
        while !peer.is_finished() {
            if Instant::now() >= deadline {
                return Err(io::Error::new(io::ErrorKind::TimedOut, "示例整体测试超时"));
            }
            server.tick()?;
            if let Some(error) = callback_error.borrow_mut().take() {
                return Err(error);
            }
            if queried.replace(false) {
                server.enqueue(&single(Cause::Spontaneous, false)?)?;
                server.enqueue(&measured(Cause::Spontaneous, 14.)?)?;
            }
            thread::sleep(Duration::from_millis(1));
        }
        Ok(())
    })();
    // 即使测试失败也先释放服务端，唤醒可能还在读取的客户端，然后回收线程。
    server.stop();
    drop(server);
    let client_result = peer
        .join()
        .map_err(|_| io::Error::other("测试客户端 panic"))?;
    server_result?;
    client_result?;
    let _rebound = TcpListener::bind(address)?;
    println!("PASS Drop 后端口重绑");
    println!("104 封装 Examples 全部通过（回环字节级子集，不代表完整协议认证）");
    Ok(())
}
