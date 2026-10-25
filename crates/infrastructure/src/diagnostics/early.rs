//! 路径解析前的受限 stderr 缓冲与 panic 分派，不安装第二个 subscriber。
use super::*;
use std::{collections::VecDeque, sync::OnceLock};
static EARLY: OnceLock<Mutex<VecDeque<String>>> = OnceLock::new();
static LOG: OnceLock<Diagnostics> = OnceLock::new();
/// 在宿主创建窗口之前安装；原 hook 仍负责原始 stderr 与 panic 行为。
pub fn install_early_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let original = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            let message = info
                .payload()
                .downcast_ref::<&str>()
                .copied()
                .or_else(|| info.payload().downcast_ref::<String>().map(String::as_str))
                .unwrap_or("非文本 panic");
            let location = info
                .location()
                .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
                .unwrap_or_default();
            let text = format!("{} {location}", record::clip(message, 4096));
            if let Some(log) = LOG.get() {
                log.event(
                    Level::Error,
                    "essim",
                    "rust_panic",
                    &text,
                    Some(&record::format_bounded(
                        format_args!("{}", std::backtrace::Backtrace::force_capture()),
                        8192,
                    )),
                );
            } else {
                early_error(&text);
            }
            original(info);
        }));
    });
}
/// 路径解析前仅保留最近十六条、每条 1 KiB；不会无限积累。
pub fn early_error(message: &str) {
    let message = record::clean(message, 1024);
    eprintln!("ESSim 启动诊断：{message}");
    if let Some(log) = LOG.get() {
        log.event(Level::Error, "essim", "startup_error", &message, None);
        return;
    }
    if let Ok(mut buffer) = EARLY.get_or_init(|| Mutex::new(VecDeque::new())).try_lock() {
        if buffer.len() == 16 {
            buffer.pop_front();
        }
        buffer.push_back(message);
    }
}
pub(super) fn attach(log: &Diagnostics) {
    if LOG.set(log.clone()).is_err() {
        return;
    }
    if let Ok(mut buffer) = EARLY.get_or_init(|| Mutex::new(VecDeque::new())).try_lock() {
        for message in buffer.drain(..) {
            log.event(Level::Error, "essim", "early_startup_error", &message, None);
        }
    }
}
