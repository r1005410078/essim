//! 文件工程仓储：清单负责逻辑组织，资源文件负责静态定义。
mod format;
mod storage;
use essim_core::{
    application::{catalog::Catalog, ports::ConfigRepository},
    domain::error::Error,
};
pub use format::{AssetEntry, Manifest, ResourceEntry, ResourceKind};
use std::{
    collections::BTreeMap,
    fs::File,
    path::{Path, PathBuf},
};

/// 持有工程独占写锁的文件仓储；析构释放内核锁，不以锁文件存在判断占用。
pub struct ProjectRepository {
    root: PathBuf,
    _lock: File,
    manifest: Manifest,
    catalog: Catalog,
    fingerprints: BTreeMap<String, u64>,
    blocked: bool,
}
impl ProjectRepository {
    /// 在空目录创建工程并保存已保存配置，不改变调用者的草稿。
    /// # Errors
    /// 目录非空、配置非法、锁失败或持久提交失败。
    pub fn create(root: &Path, name: &str, catalog: Catalog) -> Result<Self, Error> {
        format::validate_catalog(&catalog)?;
        if name.trim().is_empty() {
            return Err(Error::new("PROJECT_NAME", "工程名称不能为空"));
        }
        storage::ensure_dir(root)?;
        if std::fs::read_dir(root)
            .map_err(storage::io)?
            .next()
            .is_some()
        {
            return Err(Error::new("PROJECT_NOT_EMPTY", "请选择空目录新建工程"));
        }
        let root = root.canonicalize().map_err(storage::io)?;
        let lock = storage::lock(&root)?;
        let manifest = Manifest::new(name);
        let mut r = Self {
            root,
            _lock: lock,
            manifest,
            catalog: Catalog::default(),
            fingerprints: BTreeMap::new(),
            blocked: false,
        };
        r.commit(catalog)?;
        Ok(r)
    }
    /// 打开唯一工程入口，先恢复已决事务，再完整校验资源和引用。
    /// # Errors
    /// 入口、格式、路径、身份、引用非法，工程占用或恢复失败。
    pub fn open(entry: &Path) -> Result<Self, Error> {
        if entry.file_name().and_then(|s| s.to_str()) != Some("project.workspace") {
            return Err(Error::new("PROJECT_ENTRY", "请选择 project.workspace"));
        }
        let root = entry
            .parent()
            .ok_or_else(|| Error::new("PROJECT_ENTRY", "工程目录不存在"))?
            .canonicalize()
            .map_err(storage::io)?;
        let lock = storage::lock(&root)?;
        storage::recover(&root)?;
        let manifest: Manifest =
            storage::read_json(&storage::checked(&root, "project.workspace")?)?;
        manifest.validate()?;
        let catalog = format::decode(&root, &manifest)?;
        let fingerprints = storage::fingerprints(&root, &manifest)?;
        Ok(Self {
            root,
            _lock: lock,
            manifest,
            catalog,
            fingerprints,
            blocked: false,
        })
    }
    /// 当前工程清单，只读访问，名称和虚拟目录以此为准。
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub(crate) fn catalog_ref(&self) -> &Catalog {
        &self.catalog
    }
    /// 工程根目录的规范化绝对路径。
    pub fn root(&self) -> &Path {
        &self.root
    }
}
impl ConfigRepository for ProjectRepository {
    fn ensure_available(&self) -> Result<(), Error> {
        if self.blocked {
            Err(Error::new(
                "PROJECT_RECOVERY_REQUIRED",
                "工程提交结果待恢复，请关闭并重新打开",
            ))
        } else {
            Ok(())
        }
    }
    fn load(&self) -> Catalog {
        self.catalog.clone()
    }
    fn commit(&mut self, catalog: Catalog) -> Result<(), Error> {
        format::validate_catalog(&catalog)?;
        let (manifest, files) = format::encode(&self.manifest, &catalog)?;
        self.persist(manifest, files, catalog)
    }
}
mod budget;
pub(crate) use budget::BudgetRepository;

mod engine;
pub use engine::{ProjectDocumentInfo, ProjectInfo};
mod local;
pub use local::RecentProject;
mod runtime;
pub use runtime::RuntimeDataView;
pub(crate) use runtime::{RunRecorder, RunWriter};

impl ProjectRepository {
    fn persist(
        &mut self,
        manifest: Manifest,
        mut files: BTreeMap<String, Vec<u8>>,
        catalog: Catalog,
    ) -> Result<(), Error> {
        if self.blocked {
            return Err(Error::new(
                "PROJECT_RECOVERY_REQUIRED",
                "工程提交待恢复，请关闭后重新打开",
            ));
        }
        manifest.validate()?;
        if !self.fingerprints.is_empty()
            && storage::fingerprints(&self.root, &self.manifest)? != self.fingerprints
        {
            return Err(Error::new(
                "PROJECT_EXTERNAL_CHANGE",
                "工程文件已被外部修改，请重新加载",
            ));
        }
        for path in manifest
            .resources
            .iter()
            .filter(|r| r.kind == ResourceKind::Document)
            .map(|r| &r.path)
            .chain(manifest.assets.iter().map(|a| &a.path))
        {
            if !files.contains_key(path) {
                files.insert(
                    path.clone(),
                    std::fs::read(storage::checked(&self.root, path)?).map_err(storage::io)?,
                );
            }
        }
        files.insert("project.workspace".into(), storage::json(&manifest)?);
        if files.values().any(|bytes| bytes.len() > 256 * 1024 * 1024) {
            return Err(Error::new(
                "PROJECT_FILE_LIMIT",
                "工程单文件超过 256 MiB 保存限制，原配置未修改",
            ));
        }
        if let Err(e) = storage::commit(&self.root, &self.fingerprints, files) {
            self.blocked = self.root.join(".local/transactions/commit.json").exists();
            if !self.blocked {
                self.blocked = storage::recover(&self.root).is_err()
                    || storage::fingerprints(&self.root, &self.manifest)
                        .map_or(true, |f| f != self.fingerprints);
            }
            return Err(if self.blocked {
                Error::new(
                    "PROJECT_RECOVERY_REQUIRED",
                    format!("保存结果待恢复，请关闭并重新打开工程：{}", e.message),
                )
            } else {
                e
            });
        }
        self.blocked = true;
        self.fingerprints = storage::fingerprints(&self.root, &manifest).map_err(|e| {
            Error::new(
                "PROJECT_RECOVERY_REQUIRED",
                format!("工程已提交，完成状态待恢复：{}", e.message),
            )
        })?;
        self.manifest = manifest;
        self.catalog = catalog;
        self.blocked = false;
        Ok(())
    }
    /// 保存用户 Markdown 文档；显示名称由清单管理，文件使用稳定身份。
    /// # Errors
    /// 身份冲突、名称或格式非法、外部改写、存储不可用。
    pub fn save_document(&mut self, id: &str, name: &str, content: &str) -> Result<(), Error> {
        let (mut m, mut files) = format::encode(&self.manifest, &self.catalog)?;
        if m.resources
            .iter()
            .any(|r| r.id == id && r.kind != ResourceKind::Document)
        {
            return Err(Error::new("PROJECT_ID", "资源身份已被其他类型占用"));
        }
        m.resources.retain(|r| r.id != id);
        let path = format::resource_path(ResourceKind::Document, id);
        m.resources.push(ResourceEntry {
            id: id.into(),
            kind: ResourceKind::Document,
            display_name: name.into(),
            folder_id: None,
            path: path.clone(),
        });
        files.insert(path, content.as_bytes().to_vec());
        self.persist(m, files, self.catalog.clone())
    }
    /// 保存工程附件的原始字节；附件不作为点表或运行值。
    /// # Errors
    /// 身份、名称后缀非法、内容过大或提交失败。
    pub fn save_asset(&mut self, id: &str, name: &str, content: &[u8]) -> Result<(), Error> {
        if content.len() > 64 * 1024 * 1024 {
            return Err(Error::new("PROJECT_ASSET_LIMIT", "附件超过 64 MiB 限制"));
        }
        let (mut m, mut files) = format::encode(&self.manifest, &self.catalog)?;
        m.assets.retain(|a| a.id != id);
        let path = format::asset_path(id, name)?;
        m.assets.push(AssetEntry {
            id: id.into(),
            display_name: name.into(),
            path: path.clone(),
        });
        files.insert(path, content.to_vec());
        self.persist(m, files, self.catalog.clone())
    }
}
