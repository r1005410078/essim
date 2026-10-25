//! 每个业务 IPC 在反序列化时持有工程会话租约，覆盖异步任务整个生命周期。
use tauri::{
    ipc::{CommandArg, CommandItem, InvokeBody, InvokeError},
    Manager,
};
/// 参数由 IPC session 字段及受管理引擎解析，不接受客户端构造租约。
pub struct ProjectAccess {
    _guard: essim_infrastructure::project::ProjectClientGuard,
}
impl<'a> CommandArg<'a, tauri::Wry> for ProjectAccess {
    fn from_command(item: CommandItem<'a, tauri::Wry>) -> Result<Self, InvokeError> {
        let webview = item.message.webview_ref();
        let windows = webview.state::<crate::window_context::Windows>();
        windows.ensure_open().map_err(InvokeError::from)?;
        let context = windows
            .context(webview.label())
            .map_err(InvokeError::from)?;
        if context.closing.load(std::sync::atomic::Ordering::Acquire) {
            return Err(InvokeError::from("WINDOW_CLOSING"));
        }
        let engine = &context.engine;
        let session = match item.message.payload() {
            InvokeBody::Json(v) => v
                .get("projectSession")
                .and_then(serde_json::Value::as_str)
                .unwrap_or(""),
            _ => "",
        };
        engine
            .project_client(session)
            .map(|guard| Self { _guard: guard })
            .map_err(InvokeError::from)
    }
}
