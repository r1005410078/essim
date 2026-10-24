//! 本机状态与最近工程；不进入静态资源清单。
use super::{storage, ProjectInfo};
use crate::workspace::WorkspaceEngine;
use essim_core::domain::error::Error;
use serde_json::Value;
use std::path::{Path, PathBuf};
/// 最近工程设置，应用数据目录中的独立本机记录。
pub struct RecentProject {
    path: PathBuf,
}
/// 本机最近工程条目；路径是定位信息，打开时仍需重新验证。
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentEntry {
    /// 工程入口的规范路径。
    pub entry: PathBuf,
    /// 上次确认的工程显示名称。
    pub name: String,
    /// 最后成功打开时间，Unix 毫秒。
    pub opened_at: u64,
}
#[derive(Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecentSettings {
    format: u32,
    last_opened: Option<PathBuf>,
    entries: Vec<RecentEntry>,
}
static SETTINGS: std::sync::Mutex<()> = std::sync::Mutex::new(());
impl RecentProject {
    /// 指定应用设置目录，不读取或创建默认工程。
    pub fn new(settings: &Path) -> Self {
        Self {
            path: settings.join("recent-project.json"),
        }
    }
    fn settings(&self) -> Result<RecentSettings, Error> {
        if !self.path.exists() {
            return Ok(RecentSettings {
                format: 1,
                ..Default::default()
            });
        }
        let v: Value = storage::read_json(&self.path)?;
        if v.is_null() || v.is_string() {
            let old: Option<PathBuf> = serde_json::from_value(v).map_err(storage::io)?;
            let entries = old
                .iter()
                .map(|p| RecentEntry {
                    entry: p.clone(),
                    name: p
                        .parent()
                        .and_then(Path::file_name)
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into(),
                    opened_at: 0,
                })
                .collect();
            return Ok(RecentSettings {
                format: 1,
                last_opened: old,
                entries,
            });
        }
        let mut result: RecentSettings = serde_json::from_value(v).map_err(storage::io)?;
        if result.format != 1 {
            return Err(Error::new("RECENT_FORMAT", "最近工程设置格式不支持"));
        }
        result.entries.truncate(10);
        Ok(result)
    }
    fn write(&self, value: &RecentSettings) -> Result<(), Error> {
        storage::ensure_dir(self.path.parent().unwrap())?;
        storage::atomic_local(&self.path, &storage::json(value)?)
    }
    /// 读取自动恢复入口；显式关闭工程后为空。
    /// # Errors
    /// 设置损坏或不可读。
    pub fn load(&self) -> Result<Option<PathBuf>, Error> {
        let _g = SETTINGS.lock().map_err(storage::io)?;
        Ok(self.settings()?.last_opened)
    }
    /// 读取至多十条最近成功工程。
    /// # Errors
    /// 设置损坏或不可读。
    pub fn entries(&self) -> Result<Vec<RecentEntry>, Error> {
        let _g = SETTINGS.lock().map_err(storage::io)?;
        Ok(self.settings()?.entries)
    }
    /// 记录成功入口或明确关闭；关闭保留最近列表。
    /// # Errors
    /// 设置损坏、目录无法创建或写入失败。
    pub fn save(&self, entry: Option<&Path>) -> Result<(), Error> {
        self.save_named(entry, None)
    }
    /// 记录成功打开后的权威名称，路径按存在时的规范位置去重。
    /// # Errors
    /// 设置损坏或写入失败。
    pub fn save_named(&self, entry: Option<&Path>, name: Option<&str>) -> Result<(), Error> {
        let _g = SETTINGS.lock().map_err(storage::io)?;
        let mut settings = self.settings()?;
        settings.last_opened = entry.map(|p| p.canonicalize().unwrap_or_else(|_| p.to_path_buf()));
        if let Some(p) = &settings.last_opened {
            settings.entries.retain(|e| &e.entry != p);
            settings.entries.insert(
                0,
                RecentEntry {
                    entry: p.clone(),
                    name: name.map(str::to_owned).unwrap_or_else(|| {
                        p.parent()
                            .and_then(Path::file_name)
                            .unwrap_or_default()
                            .to_string_lossy()
                            .into()
                    }),
                    opened_at: std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .try_into()
                        .unwrap_or(u64::MAX),
                },
            );
            settings.entries.truncate(10);
        }
        self.write(&settings)
    }
    /// 只移出本机列表，不删除工程文件。
    /// # Errors
    /// 设置损坏或写入失败。
    pub fn remove(&self, entry: &Path) -> Result<(), Error> {
        let _g = SETTINGS.lock().map_err(storage::io)?;
        let mut settings = self.settings()?;
        settings.entries.retain(|e| e.entry != entry);
        if settings.last_opened.as_deref() == Some(entry) {
            settings.last_opened = None;
        }
        self.write(&settings)
    }
}

impl WorkspaceEngine {
    /// 读取当前工程本机布局或恢复草稿，不改变配置。
    /// # Errors
    /// 工程身份不匹配、文件损坏或读取失败。
    pub fn read_project_local(&self, id: &str, kind: &str) -> Result<Option<Value>, Error> {
        let _guard = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "工程状态不可用"))?;
        let info = self
            .project_info()?
            .ok_or_else(|| Error::new("PROJECT_REQUIRED", "请先打开工程"))?;
        let p = local_path(&info, id, kind)?;
        if !p.exists() {
            return Ok(None);
        }
        Ok(Some(storage::read_json(&p)?))
    }
    /// 保存当前工程本机状态，拒绝旧工程的延迟请求。
    /// # Errors
    /// 身份不匹配、状态过大或写入失败。
    pub fn write_project_local(&self, id: &str, kind: &str, value: &Value) -> Result<(), Error> {
        let _guard = self
            .workspace
            .lock()
            .map_err(|_| Error::new("STATE_POISONED", "工程状态不可用"))?;
        let info = self
            .project_info()?
            .ok_or_else(|| Error::new("PROJECT_REQUIRED", "请先打开工程"))?;
        let p = local_path(&info, id, kind)?;
        let b = storage::json(value)?;
        let limit = if kind == "workspace" {
            2 * 1024 * 1024
        } else {
            64 * 1024 * 1024
        };
        if b.len() > limit {
            return Err(Error::new(
                "PROJECT_LOCAL_LIMIT",
                "本机恢复数据超过保存限制",
            ));
        }
        storage::atomic_local(&p, &b)
    }
}
fn local_path(info: &ProjectInfo, id: &str, kind: &str) -> Result<PathBuf, Error> {
    if id != info.session {
        return Err(Error::new("PROJECT_CHANGED", "工程已切换，忽略旧请求"));
    }
    let relative = match kind {
        "workspace" => ".local/workspace.json",
        "recovery" => ".local/recovery/drafts.json",
        _ => return Err(Error::new("PROJECT_LOCAL_KIND", "未知本机状态类型")),
    };
    storage::checked(info.entry.parent().unwrap(), relative)
}
