//! 会话导航，不参与协议聚合或应用差异。
use crate::domain::{error::Error, identity::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 会话文件夹，不参与协议应用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// 稳定身份。
    pub id: FolderId,
    /// 可重复的显示名称。
    pub name: String,
    /// 根目录为空。
    pub parent: Option<FolderId>,
}
/// 根目录通过缺省成员表示，旧空目录可直接读取。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Navigation {
    /// 文件夹集合。
    pub folders: Vec<Folder>,
    /// 非根目录端点成员关系。
    pub members: BTreeMap<EndpointId, FolderId>,
}

impl Navigation {
    /// 新增或覆盖文件夹；提交前校验父引用和整个祖先链，不留下半次移动。
    /// # Errors
    /// 身份／名称为空、目标不存在或移动产生循环时失败。
    pub fn save_folder(&mut self, folder: Folder) -> Result<(), Error> {
        if folder.id.0.trim().is_empty() || folder.name.trim().is_empty() {
            return Err(Error::new("INVALID_FOLDER", "文件夹身份和名称不能为空"));
        }
        let mut parent = folder.parent.as_ref();
        let mut seen = std::collections::BTreeSet::new();
        seen.insert(&folder.id);
        while let Some(id) = parent {
            if !seen.insert(id) {
                return Err(Error::new("FOLDER_CYCLE", "不能移动到自身或子孙目录"));
            }
            parent = self
                .folders
                .iter()
                .find(|f| &f.id == id)
                .ok_or_else(|| Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"))?
                .parent
                .as_ref();
        }
        if let Some(old) = self.folders.iter_mut().find(|f| f.id == folder.id) {
            *old = folder;
        } else {
            self.folders.push(folder);
        }
        Ok(())
    }
    /// 删除空文件夹，成员存在时禁止级联删除。
    /// # Errors
    /// 文件夹不存在、存在子目录或端点成员时失败，内容不变。
    pub fn delete_folder(&mut self, id: &FolderId) -> Result<(), Error> {
        if !self.folders.iter().any(|f| &f.id == id) {
            return Err(Error::new("FOLDER_NOT_FOUND", "文件夹不存在"));
        }
        if self.folders.iter().any(|f| f.parent.as_ref() == Some(id))
            || self.members.values().any(|p| p == id)
        {
            return Err(Error::new("FOLDER_NOT_EMPTY", "请先移出或删除文件夹内容"));
        }
        self.folders.retain(|f| &f.id != id);
        Ok(())
    }
    /// 移动已确认存在的端点成员；端点存在性由应用层跨聚合检查。
    /// # Errors
    /// 目标父目录不存在时失败，旧成员关系不变。
    pub fn move_endpoint(&mut self, id: EndpointId, parent: Option<FolderId>) -> Result<(), Error> {
        if let Some(parent) = parent {
            if !self.folders.iter().any(|f| f.id == parent) {
                return Err(Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"));
            }
            self.members.insert(id, parent);
        } else {
            self.members.remove(&id);
        }
        Ok(())
    }
}
