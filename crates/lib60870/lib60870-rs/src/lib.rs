//! lib60870-C 的线程绑定安全 API；首批覆盖 104 无线程服务端与两类监视信息。
//!
//! 不依赖 ESSim、Tauri 或 Tokio；调用者负责协议业务、tick 调度与资源预算。
//! 所有 C 句柄均不能跨线程，连接和请求只能在回调作用域内借用。
//! 默认关闭的 `g0-monitor` 仅用于验证自有总召数据及代际会话发送；不改变Request借用寿命。
//!
//! ```
//! use lib60870_rs::{Asdu, Cause, Point, Quality};
//! let mut data = Asdu::new(47, Cause::Spontaneous, false)?;
//! // 公共地址 47、IOA 100、值 12.5 均为示例约定。
//! data.add(Point::measured(100, 12.5, Quality::GOOD)?)?;
//! assert_eq!(data.type_id(), 13); // M_ME_NC_1
//! # Ok::<(), std::io::Error>(())
//! ```
mod asdu;
mod server;
pub use asdu::{Asdu, Cause, Point, Quality};
use lib60870_sys as sys;
#[cfg(feature = "g0-monitor")]
pub use server::{Interrogation, SessionToken};
pub use server::{Request, Server};
use std::{io, marker::PhantomData, rc::Rc};
type ThreadBound = PhantomData<Rc<()>>;
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn failed(message: &str) -> io::Error {
    io::Error::other(message)
}
fn parameters() -> sys::AppParameters {
    sys::AppParameters {
        type_id: 1,
        vsq: 1,
        cot: 2,
        originator: 0,
        ca: 2,
        ioa: 3,
        max_asdu: 249,
    }
}
/// 返回实际 C 运行时版本；当前固定标签 2.4.1 的宏仍返回 (2, 4, 0)。
pub fn runtime_version() -> (i32, i32, i32) {
    // SAFETY: 无参只读查询；链接来源由 sys 校验。
    let v = unsafe { sys::Lib60870_getLibraryVersionInfo() };
    (v.major, v.minor, v.patch)
}
