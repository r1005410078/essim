//! 桌面进程唯一诊断服务与受限 IPC；目录由 Tauri 解析。
use essim_infrastructure::diagnostics::{Diagnostics, FrontendEvent, Level, Status, WriterGuard};
use std::{
    sync::Mutex,
    time::{Duration, Instant},
};
use tauri::Manager;
pub(crate) struct ApplicationLog {
    pub log: Diagnostics,
    pub guard: WriterGuard,
    rate: Mutex<Rate>,
}
impl ApplicationLog {
    pub fn new(directory: Result<std::path::PathBuf, tauri::Error>) -> Self {
        let (log, guard) = match directory {
            Ok(path) => Diagnostics::start(path, env!("CARGO_PKG_VERSION")),
            Err(error) => Diagnostics::unavailable(&error.to_string()),
        };
        Self {
            log,
            guard,
            rate: Mutex::new(Rate {
                start: Instant::now(),
                count: 0,
            }),
        }
    }
}
#[tauri::command]
pub(crate) fn query_application_log(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ApplicationLog>,
) -> Result<Status, String> {
    authorize(&window)?;
    Ok(state.log.status())
}
#[tauri::command]
pub(crate) fn submit_application_log(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ApplicationLog>,
    record: FrontendEvent,
) -> Result<(), String> {
    authorize(&window)?;
    record.validate().map_err(str::to_owned)?;
    let mut rate = state.rate.lock().map_err(|_| "诊断限流状态不可用")?;
    if !rate.accept(Instant::now()) {
        return Err("诊断请求已限流".into());
    }
    drop(rate);
    state.log.event(
        record.level,
        "frontend",
        &record.event,
        &record.message,
        record.stack.as_deref(),
    );
    Ok(())
}
#[tauri::command]
pub(crate) async fn open_application_log_directory(
    window: tauri::WebviewWindow,
    state: tauri::State<'_, ApplicationLog>,
) -> Result<(), String> {
    authorize(&window)?;
    let directory = state.log.directory().to_owned();
    if directory.as_os_str().is_empty() {
        return Err("系统日志目录不可用".into());
    }
    tauri::async_runtime::spawn_blocking(move || {
        #[cfg(target_os = "macos")]
        let mut command = std::process::Command::new("open");
        #[cfg(target_os = "windows")]
        let mut command = std::process::Command::new("explorer.exe");
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        let mut command = std::process::Command::new("xdg-open");
        command
            .arg(directory)
            .status()
            .map_err(|e| e.to_string())
            .and_then(|s| {
                if s.success() {
                    Ok(())
                } else {
                    Err("无法打开系统日志目录".into())
                }
            })
    })
    .await
    .map_err(|_| "打开日志目录任务失败".to_string())?
}
pub(crate) fn event(app: &tauri::AppHandle, level: Level, event: &str, message: &str) {
    if let Some(state) = app.try_state::<ApplicationLog>() {
        state
            .log
            .event(level, "essim_workbench", event, message, None);
    }
}

/// 命令结果在拥有者边界记录一次；校验／取消不作为系统故障。
pub(crate) fn result<T>(
    log: &Diagnostics,
    event: &str,
    operation: &str,
    endpoint: Option<&str>,
    result: &Result<T, essim_core::domain::error::Error>,
) {
    use essim_infrastructure::diagnostics::EventContext;
    let error = result.as_ref().err();
    let level = if let Some(error) = error {
        if error.code.contains("CANCEL")
            || error.code.contains("VALID")
            || error.code.contains("INVALID")
            || matches!(
                error.code.as_str(),
                "EMPTY_UNIT_MODEL" | "NOT_APPLIED" | "CAPABILITY_UNAVAILABLE"
            )
        {
            Level::Debug
        } else {
            Level::Error
        }
    } else {
        Level::Info
    };
    log.operation(
        level,
        event,
        if error.is_some() {
            "操作失败"
        } else {
            "操作完成"
        },
        &EventContext {
            operation_id: Some(operation),
            endpoint_id: endpoint.or_else(|| error.and_then(|e| e.endpoint_id.as_deref())),
            unit_id: error.and_then(|e| e.unit_id.as_deref()),
            error_code: error.map(|e| e.code.as_str()),
            ..Default::default()
        },
    );
}

struct Rate {
    start: Instant,
    count: u32,
}
impl Rate {
    fn accept(&mut self, now: Instant) -> bool {
        if now.duration_since(self.start) >= Duration::from_secs(1) {
            self.start = now;
            self.count = 0;
        }
        if self.count >= 20 {
            return false;
        }
        self.count += 1;
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frontend_rate_and_dto_are_bounded() {
        let now = Instant::now();
        let mut rate = Rate {
            start: now,
            count: 0,
        };
        for _ in 0..20 {
            assert!(rate.accept(now));
        }
        assert!(!rate.accept(now));
        assert!(rate.accept(now + Duration::from_secs(1)));
        let bad =
            serde_json::json!({"level":"ERROR","event":"error","message":"test","path":"/tmp"});
        assert!(serde_json::from_value::<FrontendEvent>(bad).is_err());
        let bad = serde_json::json!({"level":"FATAL","event":"error","message":"test"});
        assert!(serde_json::from_value::<FrontendEvent>(bad).is_err());
        assert!(FrontendEvent {
            level: Level::Error,
            event: "error".into(),
            message: "x".repeat(16385),
            stack: None
        }
        .validate()
        .is_err());
    }
}

/// 工程入口尚无业务 commandId 时分配进程内唯一操作身份。
pub(crate) fn next_operation(name: &str) -> String {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    format!(
        "{}-{name}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    )
}

// 自定义 IPC 再核实实际 WebView 的注册上下文，不信任调用者传来的窗口标签。
fn authorize(window: &tauri::WebviewWindow) -> Result<(), String> {
    if window
        .app_handle()
        .try_state::<crate::window_context::Windows>()
        .is_some_and(|registry| registry.context(window.label()).is_ok())
    {
        Ok(())
    } else {
        Err("当前窗口无应用诊断权限".into())
    }
}
