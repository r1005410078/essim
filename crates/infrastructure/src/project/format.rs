//! 首版工程清单 DTO 与领域映射，格式号不代表配置历史版本。
use super::storage;
use essim_core::{
    application::catalog::{effective_model, Catalog},
    domain::{
        device::Device,
        endpoint::EndpointConfig,
        error::Error,
        identity::*,
        model::CollectionModel,
        navigation::{Folder, FolderScope, Navigation},
        point_table::validate_addresses,
        values::{migrate, UnitValues},
    },
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
/// 独立资源文件类别。
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    /// 当前采集模型。
    Model,
    /// 模拟器及端点、Unit定义。
    Simulator,
    /// 设备实体。
    Device,
    /// 用户 Markdown 文档。
    Document,
}
/// 逻辑资源到物理文件的映射；名称只存在清单中。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceEntry {
    /// 稳定身份。
    pub id: String,
    /// 资源类型。
    pub kind: ResourceKind,
    /// 用户可见名称，不含存储后缀。
    pub display_name: String,
    /// 虚拟父目录；根级为空。
    pub folder_id: Option<FolderId>,
    /// 工程内相对路径。
    pub path: String,
}
/// 文档等资源引用的附件，不作为配置聚合。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssetEntry {
    /// 稳定附件身份。
    pub id: String,
    /// 原始文件显示名称。
    pub display_name: String,
    /// 工程内相对存储路径。
    pub path: String,
}
/// 当前已应用端点的独立快照定位，不进入用户资源树。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppliedEntry {
    /// 端点稳定身份。
    pub id: String,
    /// 工程内快照路径。
    pub path: String,
}
/// 工程入口与虚拟目录清单，唯一打开入口为 project.workspace。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Manifest {
    /// 工程附件映射，内容保留原始字节。
    #[serde(default)]
    pub assets: Vec<AssetEntry>,
    /// 文件格式标记，不提供配置选版。
    pub format_version: u32,
    /// 工程稳定身份。
    pub project_id: String,
    /// 工程显示名称。
    pub display_name: String,
    /// 虚拟目录定义。
    pub folders: Vec<Folder>,
    /// 用户资源目录。
    pub resources: Vec<ResourceEntry>,
    /// 当前已应用配置的独立定位。
    pub applied: Vec<AppliedEntry>,
}
impl Manifest {
    pub(super) fn new(name: &str) -> Self {
        Self {
            assets: vec![],
            format_version: 1,
            project_id: format!(
                "p-{}-{}",
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ),
            display_name: name.into(),
            folders: vec![],
            resources: vec![],
            applied: vec![],
        }
    }
    pub(super) fn validate(&self) -> Result<(), Error> {
        if self.format_version != 1 {
            return Err(Error::new(
                "PROJECT_FORMAT_VERSION",
                "不支持的工程文件格式，请使用兼容的应用",
            ));
        }
        if self.project_id.trim().is_empty() || self.display_name.trim().is_empty() {
            return Err(invalid("工程身份和名称不能为空"));
        }
        let n = Navigation {
            folders: self.folders.clone(),
            ..Default::default()
        };
        let mut ids = BTreeSet::new();
        for f in &self.folders {
            if !ids.insert(f.id.0.clone()) {
                return Err(invalid("目录身份重复"));
            }
            let mut checked = n.clone();
            checked.save_folder(f.clone())?;
        }
        let mut paths = BTreeSet::new();
        let mut resources = BTreeSet::new();
        for r in &self.resources {
            if r.id.is_empty()
                || r.id.len() > 96
                || r.display_name.trim().is_empty()
                || !resources.insert(r.id.clone())
                || !paths.insert(&r.path)
            {
                return Err(invalid("资源身份、名称或路径重复／为空"));
            }
            if r.path != resource_path(r.kind, &r.id) {
                return Err(invalid("资源路径与类型或稳定身份不匹配"));
            }
            if let Some(parent) = &r.folder_id {
                let scope = match r.kind {
                    ResourceKind::Model => FolderScope::Models,
                    ResourceKind::Simulator => FolderScope::Simulators,
                    _ => return Err(invalid("此类资源暂不支持虚拟父目录")),
                };
                n.check_parent(scope, Some(parent))?;
            }
        }
        for a in &self.assets {
            if a.id.is_empty()
                || !resources.insert(a.id.clone())
                || a.display_name.trim().is_empty()
                || a.path != asset_path(&a.id, &a.display_name)?
                || !paths.insert(&a.path)
            {
                return Err(invalid("附件映射非法"));
            }
        }
        let mut applied = BTreeSet::new();
        for a in &self.applied {
            if !applied.insert(&a.id) || a.path != format!("resources/applied/{}.json", hex(&a.id))
            {
                return Err(invalid("已应用快照身份或路径非法"));
            }
        }
        Ok(())
    }
}
fn invalid(message: &str) -> Error {
    Error::new("PROJECT_INVALID", message)
}
pub(super) fn hex(id: &str) -> String {
    id.as_bytes().iter().map(|b| format!("{b:02x}")).collect()
}
pub(super) fn resource_path(kind: ResourceKind, id: &str) -> String {
    let (dir, ext) = match kind {
        ResourceKind::Model => ("models", "json"),
        ResourceKind::Simulator => ("simulators", "json"),
        ResourceKind::Device => ("devices", "json"),
        ResourceKind::Document => ("documents", "md"),
    };
    format!("resources/{dir}/{}.{ext}", hex(id))
}
pub(super) fn asset_path(id: &str, name: &str) -> Result<String, Error> {
    let suffix = name.rsplit_once('.').map(|(_, s)| s).unwrap_or("bin");
    if id.is_empty()
        || id.len() > 96
        || suffix.is_empty()
        || suffix.len() > 16
        || !suffix.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        return Err(invalid("附件身份或后缀不合法"));
    }
    Ok(format!(
        "assets/{}.{}",
        hex(id),
        suffix.to_ascii_lowercase()
    ))
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AppliedSnapshot {
    endpoint: EndpointConfig,
    models: BTreeMap<UnitId, CollectionModel>,
}
pub(super) fn validate_catalog(c: &Catalog) -> Result<(), Error> {
    let mut ids = BTreeSet::new();
    for m in &c.models {
        if !ids.insert(m.id.0.clone()) {
            return Err(invalid("模型身份重复"));
        }
        m.validate()?;
        validate_addresses(&m.points)?;
    }
    for d in &c.devices {
        if d.id.0.is_empty() || d.name.trim().is_empty() || !ids.insert(d.id.0.clone()) {
            return Err(invalid("设备身份重复或名称为空"));
        }
    }
    let mut units = BTreeSet::new();
    for e in &c.endpoints {
        if !ids.insert(e.id.0.clone()) {
            return Err(invalid("资源身份重复"));
        }
        e.validate()?;
        for u in &e.units {
            if !units.insert(&u.id) {
                return Err(invalid("Unit 身份跨端点重复"));
            }
            let m = effective_model(c, u)?;
            validate_addresses(&m.points)?;
            if u.device
                .as_ref()
                .is_some_and(|id| !c.devices.iter().any(|d| &d.id == id))
            {
                return Err(invalid("设备引用不存在"));
            }
        }
    }
    let n = &c.navigation;
    let mut folders = BTreeSet::new();
    for f in &n.folders {
        if !folders.insert(&f.id) {
            return Err(invalid("文件夹身份重复"));
        }
        let mut checked = n.clone();
        checked.save_folder(f.clone())?;
    }
    for (id, parent) in &n.members {
        if !c.endpoints.iter().any(|e| &e.id == id) {
            return Err(invalid("目录端点成员不存在"));
        }
        n.check_parent(FolderScope::Simulators, Some(parent))?;
    }
    for (id, parent) in &n.model_members {
        if !c.models.iter().any(|m| &m.id == id) {
            return Err(invalid("目录模型成员不存在"));
        }
        n.check_parent(FolderScope::Models, Some(parent))?;
    }
    let mut applied = BTreeSet::new();
    let mut applied_units = BTreeSet::new();
    for e in &c.applied {
        if !applied.insert(&e.id) || !c.endpoints.iter().any(|saved| saved.id == e.id) {
            return Err(invalid("已应用端点重复或不存在"));
        }
        e.validate()?;
        for u in &e.units {
            if !applied_units.insert(&u.id) {
                return Err(invalid("已应用 Unit 身份重复"));
            }
            let model = c
                .values
                .get(&u.id)
                .and_then(|v| v.model.as_ref())
                .ok_or_else(|| invalid("已应用 Unit 缺少有效模型快照"))?;
            if model.id != u.model.id {
                return Err(invalid("已应用模型身份不匹配"));
            }
            model.validate()?;
            validate_addresses(&model.points)?;
        }
    }
    Ok(())
}
pub(super) fn encode(
    old: &Manifest,
    c: &Catalog,
) -> Result<(Manifest, BTreeMap<String, Vec<u8>>), Error> {
    let mut m = old.clone();
    m.folders = c.navigation.folders.clone();
    m.resources.retain(|r| r.kind == ResourceKind::Document);
    m.applied.clear();
    let mut files = BTreeMap::new();
    fn add<T: Serialize>(
        m: &mut Manifest,
        files: &mut BTreeMap<String, Vec<u8>>,
        kind: ResourceKind,
        id: &str,
        name: &str,
        parent: Option<FolderId>,
        value: &T,
    ) -> Result<(), Error> {
        let path = resource_path(kind, id);
        let mut value = serde_json::to_value(value).map_err(storage::io)?;
        value
            .as_object_mut()
            .ok_or_else(|| invalid("资源定义不是对象"))?
            .remove("name");
        files.insert(path.clone(), storage::json(&value)?);
        m.resources.push(ResourceEntry {
            id: id.into(),
            kind,
            display_name: name.into(),
            folder_id: parent,
            path,
        });
        Ok(())
    }
    for model in &c.models {
        add(
            &mut m,
            &mut files,
            ResourceKind::Model,
            &model.id.0,
            &model.name,
            c.navigation.model_members.get(&model.id).cloned(),
            model,
        )?;
    }
    for e in &c.endpoints {
        add(
            &mut m,
            &mut files,
            ResourceKind::Simulator,
            &e.id.0,
            &e.name,
            c.navigation.members.get(&e.id).cloned(),
            e,
        )?;
    }
    for d in &c.devices {
        add(
            &mut m,
            &mut files,
            ResourceKind::Device,
            &d.id.0,
            &d.name,
            None,
            d,
        )?;
    }
    for e in &c.applied {
        let models = e
            .units
            .iter()
            .map(|u| {
                Ok((
                    u.id.clone(),
                    c.values
                        .get(&u.id)
                        .and_then(|v| v.model.as_ref())
                        .ok_or_else(|| invalid("快照缺失"))?
                        .as_ref()
                        .clone(),
                ))
            })
            .collect::<Result<_, Error>>()?;
        let path = format!("resources/applied/{}.json", hex(&e.id.0));
        files.insert(
            path.clone(),
            storage::json(&AppliedSnapshot {
                endpoint: e.clone(),
                models,
            })?,
        );
        m.applied.push(AppliedEntry {
            id: e.id.0.clone(),
            path,
        });
    }
    m.validate()?;
    files.insert("project.workspace".into(), storage::json(&m)?);
    Ok((m, files))
}
pub(super) fn decode(root: &Path, m: &Manifest) -> Result<Catalog, Error> {
    let mut c = Catalog {
        navigation: Navigation {
            folders: m.folders.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    for r in &m.resources {
        let p = storage::checked(root, &r.path)?;
        if r.kind == ResourceKind::Document {
            std::fs::read(p).map_err(storage::io)?;
            continue;
        }
        let mut v: Value = storage::read_json(&p)?;
        v.as_object_mut()
            .ok_or_else(|| invalid("资源内容必须是对象"))?
            .insert("name".into(), Value::String(r.display_name.clone()));
        match r.kind {
            ResourceKind::Model => {
                let item: CollectionModel = serde_json::from_value(v).map_err(storage::io)?;
                if item.id.0 != r.id {
                    return Err(invalid("模型身份与清单不一致"));
                }
                if let Some(p) = &r.folder_id {
                    c.navigation
                        .model_members
                        .insert(item.id.clone(), p.clone());
                }
                c.models.push(item);
            }
            ResourceKind::Simulator => {
                let item: EndpointConfig = serde_json::from_value(v).map_err(storage::io)?;
                if item.id.0 != r.id {
                    return Err(invalid("端点身份与清单不一致"));
                }
                if let Some(p) = &r.folder_id {
                    c.navigation.members.insert(item.id.clone(), p.clone());
                }
                c.endpoints.push(item);
            }
            ResourceKind::Device => {
                let item: Device = serde_json::from_value(v).map_err(storage::io)?;
                if item.id.0 != r.id {
                    return Err(invalid("设备身份与清单不一致"));
                }
                c.devices.push(item);
            }
            ResourceKind::Document => unreachable!(),
        }
    }
    for a in &m.applied {
        let s: AppliedSnapshot = storage::read_json(&storage::checked(root, &a.path)?)?;
        if s.endpoint.id.0 != a.id || s.models.len() != s.endpoint.units.len() {
            return Err(invalid("应用快照与清单不一致"));
        }
        for u in &s.endpoint.units {
            let model = s
                .models
                .get(&u.id)
                .ok_or_else(|| invalid("应用快照缺少 Unit"))?;
            let values = migrate(None, &UnitValues::default(), model, true)?.values;
            c.values.insert(u.id.clone(), values);
        }
        c.applied.push(s.endpoint);
    }
    validate_catalog(&c)?;
    Ok(c)
}
