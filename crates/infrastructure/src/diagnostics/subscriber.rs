use super::*;
use tracing_log::NormalizeEvent;
use tracing_subscriber::{
    layer::{Context, SubscriberExt},
    Layer,
};
struct DiagnosticLayer(Diagnostics);
#[derive(Default)]
struct Visitor {
    message: String,
    event: String,
    stack: Option<String>,
    context: std::collections::BTreeMap<String, String>,
}
impl tracing::field::Visit for Visitor {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        // 不序列化任意结构体；仅收集显式约定的文本字段。
        if matches!(field.name(), "message" | "event" | "stack") {
            self.record_str(
                field,
                &record::format_bounded(format_args!("{value:?}"), 8192),
            );
        }
    }
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        match field.name() {
            "message" => self.message = record::clip(value, 8192),
            "event" => self.event = record::clip(value, 128),
            "stack" => self.stack = Some(record::clip(value, 8192)),
            "operationId" | "projectId" | "endpointId" | "unitId" | "errorCode" => {
                self.context
                    .insert(field.name().into(), record::clip(value, 128));
            }
            _ => {}
        }
    }
}
impl<S: tracing::Subscriber> Layer<S> for DiagnosticLayer {
    fn on_event(&self, event: &tracing::Event<'_>, _: Context<'_, S>) {
        let normalized = event.normalized_metadata();
        let meta = normalized.as_ref().unwrap_or_else(|| event.metadata());
        let level = match *meta.level() {
            tracing::Level::TRACE => Level::Trace,
            tracing::Level::DEBUG => Level::Debug,
            tracing::Level::INFO => Level::Info,
            tracing::Level::WARN => Level::Warn,
            tracing::Level::ERROR => Level::Error,
        };
        if !self.0.filter.enabled(meta.target(), level) {
            return;
        }
        let mut visitor = Visitor::default();
        event.record(&mut visitor);
        let context = EventContext {
            operation_id: visitor.context.get("operationId").map(String::as_str),
            project_id: visitor.context.get("projectId").map(String::as_str),
            endpoint_id: visitor.context.get("endpointId").map(String::as_str),
            unit_id: visitor.context.get("unitId").map(String::as_str),
            error_code: visitor.context.get("errorCode").map(String::as_str),
        };
        let bytes = self.0.encode_context(
            level,
            meta.target(),
            if visitor.event.is_empty() {
                "rust_event"
            } else {
                &visitor.event
            },
            &visitor.message,
            visitor.stack.as_deref(),
            &context,
        );
        if self.0.tx.try_send(bytes).is_err() {
            self.0.shared.dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}
pub(super) fn install(log: Diagnostics) -> Result<(), String> {
    tracing::subscriber::set_global_default(
        tracing_subscriber::registry().with(DiagnosticLayer(log)),
    )
    .map_err(|e| e.to_string())?;
    tracing_log::LogTracer::init().map_err(|e| e.to_string())
}
