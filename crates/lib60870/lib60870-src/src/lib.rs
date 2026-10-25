//! 固定 lib60870-C 源码的离线构建与产物校验；不执行协议通信。
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};

/// 上游标签版本；其运行时宏仍报告 2.4.0，见 SOURCE.md。
pub const VERSION: &str = "2.4.1";
/// 固定的官方提交。
pub const COMMIT: &str = "7a388e3e133999e1ca77ba7521d55d074b7cd2bc";
/// 下载归档的实际 SHA-256。
pub const ARCHIVE_SHA256: &str = "8bde18d06ae131cda2e49f8aa902ba1daaf1b32283afb39d741563e4291e21ff";
/// 完整归档解压后的源码树摘要；构建前校验，不接受未记录的 vendor 修改。
pub const SOURCE_TREE_SHA256: &str =
    "a4ac5fa2cbdd53bfc594d8d5c486ba6d384b1d203e939b36a521e676d9010d7d";
fn invalid(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
/// 原生库链接模式。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum LinkMode {
    /// 编译静态归档。
    Static,
    /// 编译共享库，供显式前缀模式使用。
    Dynamic,
}
fn entries(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut paths = fs::read_dir(path)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    paths.sort();
    Ok(paths)
}
fn tree_hash(path: &Path, base: &Path, hash: &mut Sha256) -> io::Result<()> {
    for entry in entries(path)? {
        if fs::symlink_metadata(&entry)?.file_type().is_symlink() {
            return Err(invalid("不接受源码／头文件符号链接"));
        }
        hash.update(
            entry
                .strip_prefix(base)
                .map_err(|e| invalid(e.to_string()))?
                .to_string_lossy()
                .as_bytes(),
        );
        if entry.is_dir() {
            tree_hash(&entry, base, hash)?;
        } else {
            hash.update(fs::read(entry)?);
        }
    }
    Ok(())
}
fn directory_hash(path: &Path) -> io::Result<String> {
    let mut hash = Sha256::new();
    tree_hash(path, path, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}
/// 获取完整 vendored 源码树摘要，用于确认构建未修改源码。
/// # Errors
/// 源码不可读或包含不接受的符号链接。
pub fn source_digest() -> io::Result<String> {
    directory_hash(&source())
}
fn source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/lib60870")
}
/// 计算文件 SHA-256。
/// # Errors
/// 文件不可读。
pub fn file_sha256(path: &Path) -> io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
/// 查询当前 Rust 编译器宿主 target。
/// # Errors
/// rustc 不可用或输出中没有 host。
pub fn host_target() -> io::Result<String> {
    let out = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("-vV")
        .output()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|s| s.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| invalid("无法确定 Rust HOST"))
}
/// 原生库来源选择。
#[derive(Debug, PartialEq, Eq)]
pub enum Selection {
    /// 从固定源码构建静态库。
    Vendored,
    /// 从用户明确给定的私有前缀加载动态库。
    Dynamic(PathBuf),
}
/// 校验 vendored 与显式动态前缀，禁止静默探测系统库。
/// # Errors
/// 两种来源冲突，或未选择任何来源。
pub fn select(vendored: bool, prefix: Option<&std::ffi::OsStr>) -> io::Result<Selection> {
    match (vendored, prefix) {
        (true, None) => Ok(Selection::Vendored),
        (false, Some(path)) if Path::new(path).is_absolute() => Ok(Selection::Dynamic(path.into())),
        (true, Some(_)) => Err(invalid("vendored 与 LIB60870_DIR 冲突")),
        _ => Err(invalid(
            "需要开启 vendored 或显式设置 LIB60870_DIR；不搜索系统库",
        )),
    }
}
/// 私有构建产物的来源、目标和摘要。
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    /// 标签版本。
    pub version: String,
    /// 官方完整提交。
    pub commit: String,
    /// 官方下载归档摘要。
    pub archive_sha256: String,
    /// 本次完整源码树摘要。
    pub source_tree_sha256: String,
    /// Rust target。
    pub target: String,
    /// 实际链接模式。
    pub mode: LinkMode,
    /// 库文件名，不包含目录。
    pub library: String,
    /// 实际库文件摘要。
    pub library_sha256: String,
    /// 头文件树摘要。
    pub headers_sha256: String,
    /// 编译器、配置、环境与源码共同确定的缓存键。
    pub build_key: String,
}
/// 已校验的构建产物路径。
#[derive(Debug)]
pub struct Artifacts {
    /// 私有安装前缀。
    pub prefix: PathBuf,
    /// 头文件根目录。
    pub include: PathBuf,
    /// 库目录。
    pub lib: PathBuf,
    /// 原生库路径。
    pub library: PathBuf,
    /// 来源清单。
    pub manifest: Manifest,
}
impl Artifacts {
    /// 加载并校验本构建器输出的私有前缀。
    /// # Errors
    /// 来源、目标、模式、文件名、库或头文件摘要不匹配。
    pub fn load(prefix: &Path, target: &str, mode: LinkMode) -> io::Result<Self> {
        let prefix = fs::canonicalize(prefix)?;
        let m: Manifest = serde_json::from_slice(&fs::read(prefix.join("lib60870-build.json"))?)?;
        let expected = library_name(target, mode);
        if m.version != VERSION
            || m.commit != COMMIT
            || m.archive_sha256 != ARCHIVE_SHA256
            || m.source_tree_sha256 != SOURCE_TREE_SHA256
            || m.source_tree_sha256 != source_digest()?
            || m.target != target
            || m.mode != mode
            || m.library != expected
        {
            return Err(invalid("lib60870 来源／目标／模式不匹配"));
        }
        let include = prefix.join("include");
        let lib = prefix.join("lib");
        let library = lib.join(&m.library);
        if !include.join("lib60870/cs104_slave.h").is_file()
            || directory_hash(&include)? != m.headers_sha256
            || file_sha256(&library)? != m.library_sha256
        {
            return Err(invalid("lib60870 产物／头文件摘要不匹配"));
        }
        if mode == LinkMode::Dynamic
            && target.contains("linux-gnu")
            && file_sha256(&lib.join(format!("liblib60870.so.{VERSION}")))? != m.library_sha256
        {
            return Err(invalid("lib60870 Linux SONAME 产物不匹配"));
        }
        Ok(Self {
            prefix,
            include,
            lib,
            library,
            manifest: m,
        })
    }
}
fn library_name(target: &str, mode: LinkMode) -> &'static str {
    match mode {
        LinkMode::Static => "liblib60870.a",
        LinkMode::Dynamic if target.ends_with("apple-darwin") => "liblib60870.dylib",
        LinkMode::Dynamic => "liblib60870.so",
    }
}
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in entries(from)? {
        let dest = to.join(entry.file_name().ok_or_else(|| invalid("无效源码路径"))?);
        if entry.is_dir() {
            copy_tree(&entry, &dest)?;
        } else {
            fs::copy(entry, dest)?;
        }
    }
    Ok(())
}
/// 上游 CMake 原生构建器；仅写入调用方提供的输出目录。
pub struct Build {
    out: PathBuf,
    mode: LinkMode,
    target: Option<String>,
    profile: String,
}
impl Build {
    /// 创建构建配置，不执行命令。
    pub fn new(out: impl Into<PathBuf>, mode: LinkMode) -> Self {
        Self {
            out: out.into(),
            mode,
            target: None,
            profile: "debug".into(),
        }
    }
    /// 指定目标；首版只接受本机 macOS／Linux GNU。
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
    /// 指定 debug 或 release。
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = profile.into();
        self
    }
    /// 离线构建原生库并写入来源清单；不安装到系统、不修改 vendor。
    /// # Errors
    /// 目标／profile 不受支持、工具缺失、源码不可读或 CMake 编译失败。
    pub fn build(self) -> io::Result<Artifacts> {
        let host = host_target()?;
        let target = self.target.unwrap_or_else(|| host.clone());
        if target != host || !(target.ends_with("apple-darwin") || target.contains("linux-gnu")) {
            return Err(invalid(format!(
                "不支持 target {target}（HOST {host}）；禁止回退宿主产物"
            )));
        }
        if !matches!(self.profile.as_str(), "debug" | "release") {
            return Err(invalid("profile 仅支持 debug／release"));
        }
        if source_digest()? != SOURCE_TREE_SHA256 {
            return Err(invalid("vendored 源码树不匹配固定归档"));
        }
        let cmake = env::var_os("CMAKE").unwrap_or_else(|| "cmake".into());
        let cc = env::var_os("CC").unwrap_or_else(|| "cc".into());
        let mut config = format!(
            "{target}:{:?}:{}:{}",
            self.mode,
            self.profile,
            source_digest()?
        );
        for tool in [&cmake, &cc] {
            let version = Command::new(tool).arg("--version").output()?;
            if !version.status.success() {
                return Err(invalid("构建工具版本查询失败"));
            }
            config += &String::from_utf8_lossy(&version.stdout);
        }
        for name in [
            "CC",
            "CFLAGS",
            "CPPFLAGS",
            "LDFLAGS",
            "AR",
            "SDKROOT",
            "MACOSX_DEPLOYMENT_TARGET",
            "CMAKE",
        ] {
            config += &format!("{name}={:?};", env::var_os(name));
        }
        let key = format!("{:x}", Sha256::digest(config.as_bytes()));
        fs::create_dir_all(&self.out)?;
        let out = fs::canonicalize(&self.out)?;
        let root = out.join(&key);
        let prefix = root.join("install");
        if let Ok(a) = Artifacts::load(&prefix, &target, self.mode) {
            if a.manifest.build_key == key {
                return Ok(a);
            }
        }
        let source_copy = root.join("source");
        if source_copy.exists() {
            fs::remove_dir_all(&source_copy)?;
        }
        copy_tree(&source(), &source_copy)?;
        let build = root.join("build");
        let log = root.join("build.log");
        fs::write(&log, &config)?;
        run(
            Command::new(&cmake)
                .arg("-S")
                .arg(source_copy.join("lib60870-C"))
                .arg("-B")
                .arg(&build)
                .arg("-DBUILD_EXAMPLES=OFF")
                .arg("-DBUILD_TESTS=OFF")
                .arg(format!("-DCMAKE_C_COMPILER={}", cc.to_string_lossy()))
                .arg(format!(
                    "-DCMAKE_BUILD_TYPE={}",
                    if self.profile == "release" {
                        "Release"
                    } else {
                        "Debug"
                    }
                )),
            &log,
        )?;
        run(
            Command::new(&cmake)
                .arg("--build")
                .arg(&build)
                .arg("--target")
                .arg(if self.mode == LinkMode::Static {
                    "lib60870"
                } else {
                    "lib60870-shared"
                })
                .arg("--parallel")
                .arg("4"),
            &log,
        )?;
        let lib = prefix.join("lib");
        let include = prefix.join("include");
        fs::create_dir_all(&lib)?;
        copy_tree(
            &source_copy.join("lib60870-C/src/inc/api"),
            &include.join("lib60870"),
        )?;
        copy_tree(
            &source_copy.join("lib60870-C/src/hal/inc"),
            &include.join("lib60870"),
        )?;
        copy_tree(
            &source_copy.join("lib60870-C/src/common/inc"),
            &include.join("lib60870"),
        )?;
        fs::copy(
            source_copy.join("lib60870-C/src/file-service/cs101_file_service.h"),
            include.join("lib60870/cs101_file_service.h"),
        )?;
        let name = library_name(&target, self.mode);
        let library = lib.join(name);
        fs::copy(build.join("src").join(name), &library)?;
        // 上游 Linux 共享库的 SONAME 含版本；保留实际加载名，而非只有链接时的 .so 名称。
        if self.mode == LinkMode::Dynamic && target.contains("linux-gnu") {
            fs::copy(&library, lib.join(format!("liblib60870.so.{VERSION}")))?;
        }
        if self.mode == LinkMode::Dynamic && target.ends_with("apple-darwin") {
            run(
                Command::new("install_name_tool")
                    .arg("-id")
                    .arg(format!("@rpath/{name}"))
                    .arg(&library),
                &log,
            )?;
        }
        let manifest = Manifest {
            version: VERSION.into(),
            commit: COMMIT.into(),
            archive_sha256: ARCHIVE_SHA256.into(),
            source_tree_sha256: source_digest()?,
            target: target.clone(),
            mode: self.mode,
            library: name.into(),
            library_sha256: file_sha256(&library)?,
            headers_sha256: directory_hash(&include)?,
            build_key: key,
        };
        fs::write(
            prefix.join("lib60870-build.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Artifacts::load(&prefix, &target, self.mode)
    }
}
fn run(command: &mut Command, log: &Path) -> io::Result<()> {
    use io::Write;
    let output = command.output()?;
    let mut file = fs::OpenOptions::new().append(true).open(log)?;
    file.write_all(&output.stdout)?;
    file.write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(invalid(format!(
            "命令 {command:?} 失败，详见 {}",
            log.display()
        )));
    }
    Ok(())
}
