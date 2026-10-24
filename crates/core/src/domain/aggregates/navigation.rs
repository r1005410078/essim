//! 会话导航，不参与协议聚合或应用差异。
use crate::domain::{error::Error, identity::*};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
/// 导航目录的业务范围，避免模型与模拟器混入同一分支。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FolderScope {
    /// 模拟器目录，也是旧格式的缺省范围。
    #[default]
    Simulators,
    /// 采集模型目录。
    Models,
}
fn default_icon() -> String {
    "folder".into()
}
fn default_color() -> String {
    "#477fbc".into()
}
/// 参与批量导航移动的稳定身份，不接受树显示路径。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum NavigationNode {
    /// 目录及其所有后代。
    Folder {
        /// 目录身份。
        id: FolderId,
    },
    /// 完整模拟器，不能单独迁移Unit。
    Endpoint {
        /// 模拟器身份。
        id: EndpointId,
    },
    /// 采集模型。
    Model {
        /// 模型身份。
        id: ModelId,
    },
}
/// 会话文件夹，不参与协议应用。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// 稳定身份。
    pub id: FolderId,
    /// 可重复的显示名称。
    pub name: String,
    /// 根目录为空。
    pub parent: Option<FolderId>,
    /// 所属导航分支；缺省为模拟器。
    #[serde(default)]
    pub scope: FolderScope,
    /// 本地图标注册键，不接收SVG代码。
    #[serde(default = "default_icon")]
    pub icon: String,
    /// 独立图标颜色，只接受六位十六进制RGB。
    #[serde(default = "default_color")]
    pub color: String,
}
/// 根目录通过缺省成员表示，旧空目录可直接读取。
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Navigation {
    /// 文件夹集合。
    pub folders: Vec<Folder>,
    /// 非根目录端点成员关系。
    pub members: BTreeMap<EndpointId, FolderId>,
    /// 分组模型的会话文件夹归属，不改变模型引用。
    #[serde(default, rename = "modelMembers")]
    pub model_members: BTreeMap<ModelId, FolderId>,
}

impl Navigation {
    /// 新增或覆盖文件夹；提交前校验父引用和整个祖先链，不留下半次移动。
    /// # Errors
    /// 身份／名称为空、目标不存在或移动产生循环时失败。
    pub fn save_folder(&mut self, folder: Folder) -> Result<(), Error> {
        if folder.id.0.trim().is_empty() || folder.name.trim().is_empty() {
            return Err(Error::new("INVALID_FOLDER", "文件夹身份和名称不能为空"));
        }
        if folder.icon.is_empty()
            || folder.icon.len() > 64
            || !folder
                .icon
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || folder.color.len() != 7
            || !folder.color.starts_with('#')
            || !folder.color[1..].bytes().all(|b| b.is_ascii_hexdigit())
        {
            return Err(Error::new("INVALID_FOLDER_STYLE", "图标键或颜色格式不合法"));
        }
        if let Some(old) = self.folders.iter().find(|f| f.id == folder.id) {
            if old.scope != folder.scope {
                return Err(Error::new("FOLDER_SCOPE", "不能更改既有目录的分类"));
            }
        }
        let mut parent = folder.parent.as_ref();
        let mut seen = std::collections::BTreeSet::new();
        seen.insert(&folder.id);
        while let Some(id) = parent {
            if !seen.insert(id) {
                return Err(Error::new("FOLDER_CYCLE", "不能移动到自身或子孙目录"));
            }
            let ancestor = self
                .folders
                .iter()
                .find(|f| &f.id == id)
                .ok_or_else(|| Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"))?;
            if ancestor.scope != folder.scope {
                return Err(Error::new("FOLDER_SCOPE", "目标文件夹分类不匹配"));
            }
            parent = ancestor.parent.as_ref();
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
            || self.model_members.values().any(|p| p == id)
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
            if !self
                .folders
                .iter()
                .any(|f| f.id == parent && f.scope == FolderScope::Simulators)
            {
                return Err(Error::new("FOLDER_NOT_FOUND", "目标文件夹不存在"));
            }
            self.members.insert(id, parent);
        } else {
            self.members.remove(&id);
        }
        Ok(())
    }
    /// 校验目标目录的存在性和分类。
    /// # Errors
    /// 非根目标不存在或范围不符。
    pub fn check_parent(&self, scope: FolderScope, parent: Option<&FolderId>) -> Result<(), Error> {
        if let Some(id) = parent {
            if !self.folders.iter().any(|f| &f.id == id && f.scope == scope) {
                return Err(Error::new("FOLDER_SCOPE", "目标目录不存在或分类不匹配"));
            }
        }
        Ok(())
    }
    /// 原子调整最多256个顶层对象的所属目录，父子混选只移动顶层。
    /// 端点和模型存在性须由应用层跨聚合核实。
    /// # Errors
    /// 目录不存在、分类不符、成环或超限时全部保持原样。
    pub fn move_nodes(
        &mut self,
        nodes: &[NavigationNode],
        scope: FolderScope,
        parent: Option<FolderId>,
    ) -> Result<(), Error> {
        use std::collections::BTreeSet;
        self.check_parent(scope, parent.as_ref())?;
        let unique: BTreeSet<_> = nodes.iter().cloned().collect();
        let selected: BTreeSet<_> = unique
            .iter()
            .filter_map(|n| {
                if let NavigationNode::Folder { id } = n {
                    Some(id.clone())
                } else {
                    None
                }
            })
            .collect();
        let mut top = vec![];
        for node in &unique {
            let mut ancestor = match node {
                NavigationNode::Folder { id } => {
                    let f = self
                        .folders
                        .iter()
                        .find(|f| &f.id == id)
                        .ok_or_else(|| Error::new("FOLDER_NOT_FOUND", "源文件夹不存在"))?;
                    if f.scope != scope {
                        return Err(Error::new("FOLDER_SCOPE", "源目录分类不匹配"));
                    }
                    f.parent.as_ref()
                }
                NavigationNode::Endpoint { id } => {
                    if scope != FolderScope::Simulators {
                        return Err(Error::new("FOLDER_SCOPE", "模拟器不能放入模型目录"));
                    }
                    self.members.get(id)
                }
                NavigationNode::Model { id } => {
                    if scope != FolderScope::Models {
                        return Err(Error::new("FOLDER_SCOPE", "模型不能放入模拟器目录"));
                    }
                    self.model_members.get(id)
                }
            };
            let mut nested = false;
            let mut seen = BTreeSet::new();
            while let Some(id) = ancestor {
                if !seen.insert(id) {
                    return Err(Error::new("FOLDER_CYCLE", "原目录存在循环"));
                }
                if selected.contains(id) {
                    nested = true;
                    break;
                }
                ancestor = self
                    .folders
                    .iter()
                    .find(|f| &f.id == id)
                    .and_then(|f| f.parent.as_ref());
            }
            if !nested {
                top.push(node);
            }
        }
        if top.is_empty() || top.len() > 256 {
            return Err(Error::new("MOVE_LIMIT", "一次移动1至256个顶层对象"));
        }
        let mut next = self.clone();
        for node in top {
            match node {
                NavigationNode::Folder { id } => {
                    let mut f = next.folders.iter().find(|f| &f.id == id).unwrap().clone();
                    f.parent = parent.clone();
                    next.save_folder(f)?;
                }
                NavigationNode::Endpoint { id } => {
                    next.move_endpoint(id.clone(), parent.clone())?
                }
                NavigationNode::Model { id } => {
                    if let Some(p) = &parent {
                        next.model_members.insert(id.clone(), p.clone());
                    } else {
                        next.model_members.remove(id);
                    }
                }
            }
        }
        *self = next;
        Ok(())
    }
    /// 将旧模拟器目录中模型成员映射为独立模型目录，保留等价祖先链。
    /// # Errors
    /// 旧目录缺失、成环时拒绝，原导航不变。
    pub fn normalize_model_folders(&mut self) -> Result<(), Error> {
        fn ensure(
            n: &mut Navigation,
            id: &FolderId,
            seen: &mut std::collections::BTreeSet<FolderId>,
        ) -> Result<FolderId, Error> {
            let mut folder = n
                .folders
                .iter()
                .find(|f| &f.id == id)
                .cloned()
                .ok_or_else(|| Error::new("FOLDER_NOT_FOUND", "旧模型目录不存在"))?;
            if folder.scope == FolderScope::Models {
                return Ok(id.clone());
            }
            if !seen.insert(id.clone()) {
                return Err(Error::new("FOLDER_CYCLE", "旧目录存在循环"));
            }
            let parent = folder
                .parent
                .as_ref()
                .map(|p| ensure(n, p, seen))
                .transpose()?;
            let mut key = FolderId(format!("models:{}", id.0));
            let mut suffix = 0;
            while n.folders.iter().any(|f| {
                f.id == key
                    && !(f.scope == FolderScope::Models
                        && f.parent == parent
                        && f.name == folder.name)
            }) {
                suffix += 1;
                key = FolderId(format!("models:{}:{suffix}", id.0));
            }
            if !n.folders.iter().any(|f| f.id == key) {
                folder.id = key.clone();
                folder.scope = FolderScope::Models;
                folder.parent = parent;
                n.save_folder(folder)?;
            }
            seen.remove(id);
            Ok(key)
        }
        let mut next = self.clone();
        for (model, parent) in self.model_members.clone() {
            let id = ensure(&mut next, &parent, &mut Default::default())?;
            next.model_members.insert(model, id);
        }
        *self = next;
        Ok(())
    }
}
