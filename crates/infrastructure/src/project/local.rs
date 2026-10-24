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
impl RecentProject {
    /// 指定应用设置目录，不读取或创建默认工程。
    pub fn new(settings: &Path) -> Self {
        Self {
            path: settings.join("recent-project.json"),
        }
    }
    /// 读取最近成功打开的工程入口；不存在设置时返回空。
    /// # Errors
    /// 设置损坏或不可读。
    pub fn load(&self) -> Result<Option<PathBuf>, Error> {
        if !self.path.exists() {
            return Ok(None);
        }
        let v: Option<PathBuf> = storage::read_json(&self.path)?;
        Ok(v)
    }
    /// 可靠替换最近工程入口；关闭工程时清空，避免重开已明确关闭工程。
    /// # Errors
    /// 设置目录无法创建或写入。
    pub fn save(&self, entry: Option<&Path>) -> Result<(), Error> {
        let parent = self.path.parent().unwrap();
        storage::ensure_dir(parent)?;
        storage::atomic_local(&self.path, &storage::json(&entry)?)
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
