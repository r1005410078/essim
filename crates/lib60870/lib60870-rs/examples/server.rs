//! 有界教学服务端：CA=47、IOA=100、值=12.5 均为示例约定，不是标准设备点表。
use lib60870_rs::{Asdu, Cause, Point, Quality, Server};
use std::{
    io::{self, BufRead},
    time::Duration,
};
fn data(cause: Cause) -> io::Result<Asdu> {
    let mut a = Asdu::new(47, cause, false)?;
    a.add(Point::measured(100, 12.5, Quality::GOOD)?)?;
    Ok(a)
}
fn main() -> io::Result<()> {
    let address = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:24040".into())
        .parse()
        .map_err(|_| io::Error::other("需要 IP:port"))?;
    let mut server = Server::bind(address, |request| {
        if request.type_id() != 100 {
            return false;
        }
        let valid = request.cause() == 6
            && request.common_address() == 47
            && request.interrogation_qualifier().ok() == Some(20);
        if !valid {
            return request.confirm(true).is_ok();
        }
        let result = (|| {
            request.confirm(false)?;
            request.send(&data(Cause::Interrogated)?)?;
            request.finish()
        })();
        if let Err(e) = result {
            eprintln!("总召响应失败：{e}");
        }
        true
    })?;
    println!("READY {address}");
    let (quit, stopped) = std::sync::mpsc::sync_channel(1);
    // stdin 不持有协议句柄；独立线程仅把 quit／EOF 转为通知。
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            if line.map_or(true, |line| line.trim() == "quit") {
                break;
            }
        }
        let _ = quit.send(());
    });
    loop {
        if stopped.try_recv().is_ok() {
            break;
        }
        server.tick()?;
        std::thread::sleep(Duration::from_millis(1));
    }
    server.stop();
    Ok(())
}
