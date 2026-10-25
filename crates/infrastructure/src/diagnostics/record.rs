use serde::{Deserialize, Serialize};

/// 诊断级别，按严重程度排序。
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "UPPERCASE")]
pub enum Level {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
impl Level {
    pub(super) fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "trace" => Some(Self::Trace),
            "debug" => Some(Self::Debug),
            "info" => Some(Self::Info),
            "warn" => Some(Self::Warn),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}
/// 前端仅能提交已知文本字段，不能指定路径或任意对象。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FrontendEvent {
    pub level: Level,
    pub event: String,
    pub message: String,
    pub stack: Option<String>,
}
impl FrontendEvent {
    /// 校验 IPC 文本边界；不将超大请求传入编码队列。
    /// # Errors
    /// 已编码载荷超过 16 KiB、事件类别为空或字段超过限制。
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.event.is_empty()
            || self.event.len() > 128
            || self.message.len() > 8192
            || self.stack.as_ref().is_some_and(|s| s.len() > 8192)
            || serde_json::to_vec(self).map_or(true, |v| v.len() > super::MAX_RECORD)
        {
            return Err("诊断请求超过文本边界");
        }
        Ok(())
    }
}
/// 启动过滤器；应用默认与第三方默认分开，不支持任意正则表达式。
#[derive(Clone, Debug)]
pub struct Filter {
    app: Level,
    external: Level,
    modules: Vec<(String, Level)>,
}
impl Filter {
    /// 返回过滤器及非法配置标志；非法配置整体回退默认值。
    pub fn parse(value: Option<&str>, debug: bool) -> (Self, bool) {
        let default = Self {
            app: if debug { Level::Debug } else { Level::Info },
            external: Level::Warn,
            modules: vec![],
        };
        let Some(value) = value else {
            return (default, false);
        };
        let mut result = default.clone();
        for item in value.split(',') {
            if let Some((module, level)) = item.split_once('=') {
                if module.is_empty()
                    || !module
                        .chars()
                        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
                {
                    return (default, true);
                }
                let Some(level) = Level::parse(level) else {
                    return (default, true);
                };
                result.modules.push((module.into(), level));
            } else {
                let Some(level) = Level::parse(item) else {
                    return (default, true);
                };
                result.app = level;
                result.external = level;
            }
        }
        result
            .modules
            .sort_by_key(|(name, _)| std::cmp::Reverse(name.len()));
        (result, false)
    }
    /// 判断目标模块与级别是否启用。
    pub fn enabled(&self, target: &str, level: Level) -> bool {
        let threshold = self
            .modules
            .iter()
            .find(|(m, _)| target == m || target.starts_with(&format!("{m}::")))
            .map(|(_, l)| *l)
            .unwrap_or(if target.starts_with("essim") || target == "frontend" {
                self.app
            } else {
                self.external
            });
        level >= threshold
    }
}
/// 限制 UTF-8 字节数，避免切断字符。
pub(super) fn clip(text: &str, limit: usize) -> String {
    let mut end = text.len().min(limit);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].into()
}
/// 清理约定敏感键、URL 参数与本机用户根目录；不保证任意自然语言脱敏。
pub(super) fn clean(text: &str, limit: usize) -> String {
    let mut text = clip(text, limit);
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        let home = home.to_string_lossy();
        if !home.is_empty() {
            text = text.replace(home.as_ref(), "~");
        }
    }
    text.split_whitespace()
        .map(|word| {
            let lower = word.to_ascii_lowercase();
            if ["password=", "token=", "secret=", "authorization="]
                .iter()
                .any(|key| lower.starts_with(key))
            {
                format!(
                    "{}[REDACTED]",
                    word.split_once('=')
                        .map(|(k, _)| format!("{k}="))
                        .unwrap_or_default()
                )
            } else if word.contains("://") {
                word.split(['?', '#']).next().unwrap_or("").to_owned()
            } else {
                word.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filter_defaults_and_invalid_fallback() {
        let (f, invalid) = Filter::parse(None, false);
        assert!(!invalid);
        assert!(!f.enabled("essim_core", Level::Debug));
        assert!(f.enabled("essim_core", Level::Info));
        assert!(!f.enabled("hyper", Level::Info));
        let (f, invalid) = Filter::parse(Some("info,essim_core=trace"), false);
        assert!(!invalid);
        assert!(f.enabled("essim_core::model", Level::Trace));
        let (f, invalid) = Filter::parse(Some("essim_core=typo"), false);
        assert!(invalid);
        assert!(!f.enabled("essim_core", Level::Debug));
    }
    #[test]
    fn sensitive_context_and_utf8() {
        assert_eq!(clip("中ab", 2), "");
        assert_eq!(
            clean("https://host/path?token=abc#x password=abc", 1024),
            "https://host/path password=[REDACTED]"
        );
        assert!(serde_json::from_str::<FrontendEvent>(
            r#"{"level":"ERROR","event":"error","message":"oops","stack":null,"path":"/tmp"}"#
        )
        .is_err());
    }
}

/// 对第三方格式化参数直接限长，避免先构造任意大的临时字符串。
pub(super) fn format_bounded(arguments: std::fmt::Arguments<'_>, limit: usize) -> String {
    struct Limited {
        value: String,
        limit: usize,
    }
    impl std::fmt::Write for Limited {
        fn write_str(&mut self, text: &str) -> std::fmt::Result {
            let room = self.limit.saturating_sub(self.value.len());
            self.value.push_str(&clip(text, room));
            if text.len() > room {
                Err(std::fmt::Error)
            } else {
                Ok(())
            }
        }
    }
    let mut output = Limited {
        value: String::new(),
        limit,
    };
    let _ = std::fmt::write(&mut output, arguments);
    output.value
}
