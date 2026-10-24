//! 每个业务 IPC 在反序列化时持有工程会话租约，覆盖异步任务整个生命周期。
use tauri::{
    ipc::{CommandArg, CommandItem, InvokeBody, InvokeError},
    Manager, Runtime,
};
/// 参数由 IPC session 字段及受管理引擎解析，不接受客户端构造租约。
pub struct ProjectAccess {
    _guard: essim_infrastructure::project::ProjectClientGuard,
}
impl<'a, R: Runtime> CommandArg<'a, R> for ProjectAccess {
    fn from_command(item: CommandItem<'a, R>) -> Result<Self, InvokeError> {
        let engine = item.message.webview_ref().state::<super::Engine>();
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
