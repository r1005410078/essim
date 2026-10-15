//! 固定 libmodbus 源码的离线构建辅助 API，不包含运行时 FFI。
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    env, fs, io,
    path::{Path, PathBuf},
    process::Command,
};
pub const VERSION: &str = "3.2.0";
pub const COMMIT: &str = "a9b025d12289855490b10d77461c99e001abfc0f";
pub const ARCHIVE_SHA256: &str = "0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa";
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LinkMode {
    Static,
    Dynamic,
}
fn error(s: impl Into<String>) -> io::Error {
    io::Error::other(s.into())
}
pub fn host_target() -> io::Result<String> {
    let out = Command::new(env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .arg("-vV")
        .output()?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .find_map(|l| l.strip_prefix("host: ").map(str::to_owned))
        .ok_or_else(|| error("无法确定 Rust HOST target"))
}
pub fn file_sha256(path: &Path) -> io::Result<String> {
    Ok(format!("{:x}", Sha256::digest(fs::read(path)?)))
}
fn entries(path: &Path) -> io::Result<Vec<PathBuf>> {
    let mut v = fs::read_dir(path)?
        .map(|e| e.map(|x| x.path()))
        .collect::<io::Result<Vec<_>>>()?;
    v.sort();
    Ok(v)
}
fn digest_tree(path: &Path, base: &Path, hasher: &mut Sha256) -> io::Result<()> {
    for p in entries(path)? {
        hasher.update(p.strip_prefix(base).unwrap().to_string_lossy().as_bytes());
        if p.is_dir() {
            digest_tree(&p, base, hasher)?;
        } else {
            hasher.update(fs::read(&p)?);
        }
    }
    Ok(())
}
pub fn source_digest() -> io::Result<String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/libmodbus");
    let mut h = Sha256::new();
    digest_tree(&path, &path, &mut h)?;
    Ok(format!("{:x}", h.finalize()))
}
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for p in entries(src)? {
        let dest = dst.join(p.file_name().unwrap());
        if p.is_dir() {
            copy_tree(&p, &dest)?;
        } else {
            fs::copy(p, dest)?;
        }
    }
    Ok(())
}
#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub version: String,
    pub commit: String,
    pub archive_sha256: String,
    pub source_tree_sha256: String,
    pub target: String,
    pub mode: LinkMode,
    pub profile: String,
    pub build_key: String,
    pub library: String,
    pub library_sha256: String,
}
#[derive(Debug)]
pub struct Artifacts {
    pub prefix: PathBuf,
    pub include: PathBuf,
    pub lib: PathBuf,
    pub library: PathBuf,
    pub manifest: Manifest,
}
impl Artifacts {
    pub fn load(prefix: &Path, target: &str, mode: LinkMode) -> io::Result<Self> {
        let prefix = fs::canonicalize(prefix)?;
        let m: Manifest = serde_json::from_slice(&fs::read(prefix.join("libmodbus-build.json"))?)
            .map_err(|e| error(format!("构建清单无效：{e}")))?;
        if m.version != VERSION
            || m.commit != COMMIT
            || m.archive_sha256 != ARCHIVE_SHA256
            || m.target != target
            || m.mode != mode
        {
            return Err(error("libmodbus 来源/版本/target/链接模式不匹配"));
        }
        if Path::new(&m.library).components().count() != 1 || !m.library.starts_with("libmodbus") {
            return Err(error("无效的库文件名"));
        }
        let include = prefix.join("include");
        let lib = prefix.join("lib");
        let library = lib.join(&m.library);
        if !include.join("modbus/modbus.h").is_file()
            || !include.join("modbus/modbus-version.h").is_file()
            || file_sha256(&library)? != m.library_sha256
        {
            return Err(error("libmodbus 库或头文件缺失/校验失败"));
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
pub struct Build {
    out: PathBuf,
    mode: LinkMode,
    target: Option<String>,
    profile: String,
    compiler: Option<String>,
}
impl Build {
    pub fn new(out: impl Into<PathBuf>, mode: LinkMode) -> Self {
        Self {
            out: out.into(),
            mode,
            target: None,
            profile: env::var("PROFILE").unwrap_or_else(|_| "debug".into()),
            compiler: None,
        }
    }
    pub fn compiler(mut self, path: impl Into<String>) -> Self {
        self.compiler = Some(path.into());
        self
    }
    pub fn target(mut self, target: impl Into<String>) -> Self {
        self.target = Some(target.into());
        self
    }
    pub fn profile(mut self, profile: impl Into<String>) -> Self {
        self.profile = profile.into();
        self
    }
    pub fn build(self) -> io::Result<Artifacts> {
        if !matches!(self.profile.as_str(), "debug" | "release") {
            return Err(error("profile 只支持 debug/release"));
        }
        let host = host_target()?;
        let target = self.target.unwrap_or_else(|| host.clone());
        if target != host || !(target.ends_with("apple-darwin") || target.contains("linux-gnu")) {
            return Err(error(format!(
                "target {target} 未验证或不支持交叉构建（HOST {host}）；不会回退宿主库"
            )));
        }
        let cc = self
            .compiler
            .unwrap_or_else(|| env::var("CC").unwrap_or_else(|_| "cc".into()));
        let tools = [
            cc.clone(),
            env::var("CXX").unwrap_or_else(|_| "c++".into()),
            "make".into(),
            "autoconf".into(),
            "automake".into(),
            "autoreconf".into(),
        ];
        let mut config = format!("CC={cc};");
        for tool in tools {
            let output = Command::new(&tool)
                .arg("--version")
                .output()
                .map_err(|e| error(format!("需要构建工具 {tool}：{e}")))?;
            if !output.status.success() {
                return Err(error(format!("构建工具不可用：{tool}")));
            }
            config += &String::from_utf8_lossy(&output.stdout);
        }
        // autoreconf will select glibtoolize on macOS and libtoolize on Linux.
        let lt = if target.ends_with("apple-darwin") {
            "glibtoolize"
        } else {
            "libtoolize"
        };
        let output = Command::new(lt)
            .arg("--version")
            .output()
            .map_err(|e| error(format!("需要 GNU libtool（{lt}）：{e}")))?;
        if !output.status.success() {
            return Err(error(format!("构建工具不可用：{lt}")));
        }
        config += &String::from_utf8_lossy(&output.stdout);
        let source = source_digest()?;
        config += &format!("{target}:{:?}:{}:{source}", self.mode, self.profile);
        for key in [
            "CC",
            "CXX",
            "AR",
            "RANLIB",
            "CFLAGS",
            "CPPFLAGS",
            "CXXFLAGS",
            "LDFLAGS",
            "SDKROOT",
            "MACOSX_DEPLOYMENT_TARGET",
        ] {
            config += &format!("{key}={:?};", env::var_os(key));
        }
        let key = format!("{:x}", Sha256::digest(config.as_bytes()));
        fs::create_dir_all(&self.out)?;
        let out =
            fs::canonicalize(self.out)?.join(format!("{target}-{:?}-{}", self.mode, self.profile));
        fs::create_dir_all(&out)?;
        let prefix = out.join("install");
        if let Ok(a) = Artifacts::load(&prefix, &target, self.mode) {
            if a.manifest.build_key == key {
                return Ok(a);
            }
        }
        let work = out.join("build").join(&key);
        let src = work.join("source");
        fs::create_dir_all(&work)?;
        if src.exists() {
            fs::remove_dir_all(&src)?;
        }
        copy_tree(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("vendor/libmodbus"),
            &src,
        )?;
        let log = out.join("build.log");
        fs::write(&log, format!("target={target}\nmode={:?}\n", self.mode))?;
        run(Command::new("sh").arg("autogen.sh").current_dir(&src), &log)?;
        let mut configure = Command::new(src.join("configure"));
        configure.env("CC", &cc);
        configure
            .current_dir(&work)
            .arg(format!("--prefix={}", prefix.display()))
            .arg("--disable-tests");
        match self.mode {
            LinkMode::Static => {
                configure.args(["--enable-static", "--disable-shared"]);
            }
            LinkMode::Dynamic => {
                configure.args(["--disable-static", "--enable-shared"]);
            }
        }
        if self.profile == "debug" {
            configure.arg("--enable-debug");
        }
        run(&mut configure, &log)?;
        run(
            Command::new("make")
                .arg(format!(
                    "-j{}",
                    env::var("NUM_JOBS")
                        .ok()
                        .and_then(|v| v.parse::<u32>().ok())
                        .unwrap_or(4)
                        .clamp(1, 8)
                ))
                .current_dir(&work),
            &log,
        )?;
        // No host installation: make install writes only into the private prefix.
        if prefix.exists() {
            fs::remove_dir_all(&prefix)?;
        }
        run(Command::new("make").arg("install").current_dir(&work), &log)?;
        let library = match self.mode {
            LinkMode::Static => "libmodbus.a".to_owned(),
            LinkMode::Dynamic => entries(&prefix.join("lib"))?
                .into_iter()
                .find(|p| {
                    p.file_name()
                        .unwrap()
                        .to_string_lossy()
                        .starts_with("libmodbus")
                        && !fs::symlink_metadata(p).unwrap().file_type().is_symlink()
                        && (p.extension().is_some_and(|x| x == "dylib")
                            || p.file_name().unwrap().to_string_lossy().contains(".so."))
                })
                .ok_or_else(|| error("未生成动态库"))?
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        };
        let libpath = prefix.join("lib").join(&library);
        if self.mode == LinkMode::Dynamic && target.ends_with("apple-darwin") {
            run(
                Command::new("install_name_tool")
                    .args(["-id", &format!("@rpath/{library}")])
                    .arg(&libpath),
                &log,
            )?;
        }
        let manifest = Manifest {
            version: VERSION.into(),
            commit: COMMIT.into(),
            archive_sha256: ARCHIVE_SHA256.into(),
            source_tree_sha256: source,
            target: target.clone(),
            mode: self.mode,
            profile: self.profile,
            build_key: key,
            library,
            library_sha256: file_sha256(&libpath)?,
        };
        fs::write(
            prefix.join("libmodbus-build.json"),
            serde_json::to_vec_pretty(&manifest)?,
        )?;
        Artifacts::load(&prefix, &target, self.mode)
    }
}
fn run(command: &mut Command, log: &Path) -> io::Result<()> {
    let output = command
        .output()
        .map_err(|e| error(format!("执行 {command:?} 失败：{e}")))?;
    use io::Write;
    let mut file = fs::OpenOptions::new().append(true).open(log)?;
    file.write_all(&output.stdout)?;
    file.write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(error(format!(
            "执行 {command:?} 失败，见 {}",
            log.display()
        )));
    }
    Ok(())
}
