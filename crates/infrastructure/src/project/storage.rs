//! 同文件系统的持久决策与幂等安装；不提供用户配置历史。
use super::format::Manifest;
use essim_core::domain::error::Error;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs::{self, File, OpenOptions},
    hash::{Hash, Hasher},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};
pub(super) fn io(e: impl std::fmt::Display) -> Error {
    Error::new("PROJECT_IO", format!("工程存储失败：{e}"))
}
pub(super) fn checked(root: &Path, relative: &str) -> Result<PathBuf, Error> {
    let p = Path::new(relative);
    if relative.is_empty()
        || p.components().any(|c| !matches!(c, Component::Normal(_)))
        || relative.contains('\\')
    {
        return Err(Error::new("PROJECT_PATH", "工程路径必须位于工程目录内"));
    }
    let mut current = root.to_path_buf();
    for c in p.components() {
        current.push(c);
        match fs::symlink_metadata(&current) {
            Ok(m) if m.file_type().is_symlink() => {
                return Err(Error::new("PROJECT_PATH", "工程内部路径不允许符号链接"))
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
    }
    Ok(current)
}
fn sync_dir(p: &Path) -> Result<(), Error> {
    #[cfg(unix)]
    {
        File::open(p).and_then(|f| f.sync_all()).map_err(io)?;
    }
    #[cfg(not(unix))]
    {
        let _ = p;
    }
    Ok(())
}
pub(super) fn ensure_dir(path: &Path) -> Result<(), Error> {
    match fs::metadata(path) {
        Ok(m) if m.is_dir() => return Ok(()),
        Ok(_) => return Err(io("目标目录实际为文件")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => return Err(io(e)),
    }
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    ensure_dir(parent)?;
    match fs::create_dir(path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists && path.is_dir() => {}
        Err(e) => return Err(io(e)),
    }
    sync_dir(parent)?;
    sync_dir(path)
}
pub(super) fn lock(root: &Path) -> Result<File, Error> {
    let p = checked(root, ".local/project.lock")?;
    ensure_dir(p.parent().unwrap())?;
    let f = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(p)
        .map_err(io)?;
    f.try_lock()
        .map_err(|_| Error::new("PROJECT_IN_USE", "工程正在另一个窗口或进程中使用"))?;
    Ok(f)
}
fn bytes(p: &Path) -> Result<Vec<u8>, Error> {
    let f = File::open(p).map_err(io)?;
    if f.metadata().map_err(io)?.len() > 256 * 1024 * 1024 {
        return Err(Error::new(
            "PROJECT_FILE_LIMIT",
            "工程单文件超过 256 MiB 读取限制",
        ));
    }
    let mut b = Vec::new();
    f.take(256 * 1024 * 1024 + 1)
        .read_to_end(&mut b)
        .map_err(io)?;
    if b.len() > 256 * 1024 * 1024 {
        return Err(Error::new("PROJECT_FILE_LIMIT", "工程单文件超过读取限制"));
    }
    Ok(b)
}
pub(super) fn read_json<T: DeserializeOwned>(p: &Path) -> Result<T, Error> {
    serde_json::from_slice(&bytes(p)?)
        .map_err(|e| Error::new("PROJECT_FORMAT", format!("工程文件格式不正确：{e}")))
}
pub(super) fn json<T: Serialize>(v: &T) -> Result<Vec<u8>, Error> {
    serde_json::to_vec_pretty(v).map_err(io)
}
fn hash(b: &[u8]) -> u64 {
    let mut h = std::hash::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}
pub(super) fn fingerprints(root: &Path, m: &Manifest) -> Result<BTreeMap<String, u64>, Error> {
    std::iter::once("project.workspace".to_owned())
        .chain(m.resources.iter().map(|r| r.path.clone()))
        .chain(m.applied.iter().map(|r| r.path.clone()))
        .chain(m.assets.iter().map(|r| r.path.clone()))
        .map(|p| {
            let h = hash(&bytes(&checked(root, &p)?)?);
            Ok((p, h))
        })
        .collect()
}
fn write_synced(p: &Path, data: &[u8]) -> Result<(), Error> {
    #[cfg(test)]
    if p.file_name().and_then(|n| n.to_str()) == Some("prepared.json")
        && std::env::var_os("PROJECT_TEST_DISK_FULL").is_some()
    {
        let code = std::env::var("PROJECT_TEST_DISK_FULL")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(28);
        return Err(io(std::io::Error::from_raw_os_error(code)));
    }
    let parent = p.parent().unwrap();
    ensure_dir(parent)?;
    let mut f = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(p)
        .map_err(io)?;
    f.write_all(data).map_err(io)?;
    f.sync_all().map_err(io)?;
    sync_dir(parent)
}
#[derive(Serialize, Deserialize)]
struct Transaction {
    writes: BTreeMap<String, String>,
    deletes: Vec<String>,
}
fn install(root: &Path, t: &Transaction) -> Result<(), Error> {
    // 工程入口最后安装。恢复期间不允许业务读取部分新文件。
    for (path, staged) in t
        .writes
        .iter()
        .filter(|(p, _)| p.as_str() != "project.workspace")
        .chain(
            t.writes
                .iter()
                .filter(|(p, _)| p.as_str() == "project.workspace"),
        )
    {
        let target = checked(root, path)?;
        let parent = target.parent().unwrap();
        ensure_dir(parent)?;
        let tmp = parent.join(".install-pending");
        if tmp.exists() {
            fs::remove_file(&tmp).map_err(io)?;
        }
        let data = bytes(&checked(root, staged)?)?;
        write_synced(&tmp, &data)?;
        fs::rename(&tmp, &target).map_err(io)?;
        sync_dir(parent)?;
        crash(if path == "project.workspace" {
            "after_manifest"
        } else {
            "after_resource"
        });
    }
    for p in &t.deletes {
        let p = checked(root, p)?;
        match fs::remove_file(&p) {
            Ok(()) => {
                sync_dir(p.parent().unwrap())?;
                crash("after_delete");
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(io(e)),
        }
    }
    Ok(())
}
fn validate_transaction(t: &Transaction) -> Result<(), Error> {
    for p in t.writes.keys().chain(t.deletes.iter()) {
        if p != "project.workspace" && !p.starts_with("resources/") && !p.starts_with("assets/") {
            return Err(Error::new("PROJECT_TRANSACTION", "事务包含非资源路径"));
        }
    }
    for staged in t.writes.values() {
        if !staged
            .strip_prefix(".local/transactions/files/")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(Error::new("PROJECT_TRANSACTION", "事务暂存路径非法"));
        }
    }
    if !t.writes.contains_key("project.workspace") {
        return Err(Error::new("PROJECT_TRANSACTION", "事务缺少工程清单"));
    }
    Ok(())
}
pub(super) fn recover(root: &Path) -> Result<(), Error> {
    let dir = checked(root, ".local/transactions")?;
    let marker = checked(root, ".local/transactions/commit.json")?;
    if marker.exists() {
        let t: Transaction = read_json(&marker)?;
        validate_transaction(&t)?;
        install(root, &t)?;
        fs::remove_file(marker).map_err(io)?;
        sync_dir(&dir)?;
    }
    let pending = checked(root, ".local/transactions/prepared.json")?;
    if pending.exists() {
        fs::remove_file(pending).map_err(io)?;
        sync_dir(&dir)?;
    }
    let files = checked(root, ".local/transactions/files")?;
    if files.exists() {
        fs::remove_dir_all(files).map_err(io)?;
        sync_dir(&dir)?;
    }
    Ok(())
}
pub(super) fn commit(
    root: &Path,
    previous: &BTreeMap<String, u64>,
    mut files: BTreeMap<String, Vec<u8>>,
) -> Result<(), Error> {
    let deletes = previous
        .keys()
        .filter(|p| !files.contains_key(*p))
        .cloned()
        .collect();
    // 清单始终包含在决策中，其余资源仅写变化内容。
    files.retain(|p, b| p == "project.workspace" || previous.get(p) != Some(&hash(b)));
    let mut staged = BTreeMap::new();
    for (i, (path, data)) in files.into_iter().enumerate() {
        let stage = format!(".local/transactions/files/{i}");
        write_synced(&checked(root, &stage)?, &data)?;
        staged.insert(path, stage);
    }
    let t = Transaction {
        writes: staged,
        deletes,
    };
    validate_transaction(&t)?;
    crash("after_staging");
    let prepared = checked(root, ".local/transactions/prepared.json")?;
    write_synced(&prepared, &json(&t)?)?;
    crash("after_prepared");
    let marker = checked(root, ".local/transactions/commit.json")?;
    fs::rename(&prepared, &marker).map_err(io)?;
    sync_dir(marker.parent().unwrap())?;
    crash("after_decision");
    install(root, &t)?;
    fs::remove_file(&marker).map_err(io)?;
    sync_dir(marker.parent().unwrap())?;
    crash("after_cleanup");
    recover(root)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stage(root: &Path, items: Vec<(&str, &[u8])>, deletes: Vec<String>) -> Transaction {
        let mut writes = BTreeMap::new();
        for (i, (path, bytes)) in items.into_iter().enumerate() {
            let staged = format!(".local/transactions/files/{i}");
            write_synced(&root.join(&staged), bytes).unwrap();
            writes.insert(path.into(), staged);
        }
        Transaction { writes, deletes }
    }
    fn temp() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "project-txn-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&p).unwrap();
        p
    }
    #[test]
    fn undecided_is_discarded_and_decided_is_replayed() {
        for decided in [false, true] {
            let root = temp();
            fs::write(root.join("project.workspace"), b"old").unwrap();
            let t = stage(
                &root,
                vec![
                    ("project.workspace", b"new"),
                    ("resources/models/a.json", b"new-model"),
                ],
                vec![],
            );
            let name = if decided {
                "commit.json"
            } else {
                "prepared.json"
            };
            write_synced(
                &root.join(format!(".local/transactions/{name}")),
                &json(&t).unwrap(),
            )
            .unwrap();
            recover(&root).unwrap();
            recover(&root).unwrap();
            assert_eq!(
                fs::read(root.join("project.workspace")).unwrap(),
                if decided { b"new" } else { b"old" }
            );
            assert_eq!(root.join("resources/models/a.json").exists(), decided);
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn partial_install_and_delete_are_idempotent() {
        let root = temp();
        fs::create_dir_all(root.join("resources/models")).unwrap();
        fs::write(root.join("resources/models/new.json"), b"installed").unwrap();
        fs::write(root.join("resources/models/old.json"), b"old").unwrap();
        let t = stage(
            &root,
            vec![
                ("project.workspace", b"next"),
                ("resources/models/new.json", b"installed"),
            ],
            vec!["resources/models/old.json".into()],
        );
        write_synced(
            &root.join(".local/transactions/commit.json"),
            &json(&t).unwrap(),
        )
        .unwrap();
        recover(&root).unwrap();
        assert!(!root.join("resources/models/old.json").exists());
        assert_eq!(fs::read(root.join("project.workspace")).unwrap(), b"next");
        fs::remove_dir_all(root).unwrap();
    }
}

pub(super) fn atomic_local(path: &Path, data: &[u8]) -> Result<(), Error> {
    let parent = path.parent().unwrap();
    ensure_dir(parent)?;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let tmp = parent.join(format!(
        ".{}.{}-{}.pending",
        path.file_name().unwrap().to_string_lossy(),
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ));
    let result = (|| {
        write_synced(&tmp, data)?;
        fs::rename(&tmp, path).map_err(io)?;
        sync_dir(parent)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn crash(label: &str) {
    #[cfg(test)]
    if std::env::var("PROJECT_TEST_CRASH").ok().as_deref() == Some(label) {
        std::process::exit(73);
    }
    #[cfg(not(test))]
    let _ = label;
}
#[cfg(test)]
mod process_tests {
    use super::*;
    #[test]
    fn crash_writer_child() {
        let Some(root) = std::env::var_os("PROJECT_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let old = BTreeMap::from([
            ("project.workspace".into(), hash(b"old")),
            ("resources/models/old.json".into(), hash(b"old-model")),
        ]);
        let result = commit(
            &root,
            &old,
            BTreeMap::from([
                ("project.workspace".into(), b"new".to_vec()),
                ("resources/models/new.json".into(), b"new-model".to_vec()),
            ]),
        );
        if std::env::var_os("PROJECT_TEST_DISK_FULL").is_some() {
            assert!(result.is_err());
            std::process::exit(74);
        }
        result.unwrap();
        panic!("故障点未触发");
    }
    #[test]
    fn subprocess_crashes_never_expose_a_mixed_generation() {
        for phase in [
            "after_staging",
            "after_prepared",
            "after_decision",
            "after_resource",
            "after_manifest",
            "after_delete",
            "after_cleanup",
            "disk_full",
            "permission_denied",
        ] {
            let root =
                std::env::temp_dir().join(format!("project-crash-{}-{phase}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(root.join("resources/models")).unwrap();
            fs::write(root.join("project.workspace"), b"old").unwrap();
            fs::write(root.join("resources/models/old.json"), b"old-model").unwrap();
            let mut command = std::process::Command::new(std::env::current_exe().unwrap());
            command
                .args([
                    "--exact",
                    "project::storage::process_tests::crash_writer_child",
                    "--nocapture",
                ])
                .env("PROJECT_TEST_ROOT", &root);
            if matches!(phase, "disk_full" | "permission_denied") {
                command.env(
                    "PROJECT_TEST_DISK_FULL",
                    if phase == "disk_full" { "28" } else { "13" },
                );
            } else {
                command.env("PROJECT_TEST_CRASH", phase);
            }
            let output = command.output().unwrap();
            assert_eq!(
                output.status.code(),
                Some(if matches!(phase, "disk_full" | "permission_denied") {
                    74
                } else {
                    73
                }),
                "{phase}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            recover(&root).unwrap();
            recover(&root).unwrap();
            let committed = !matches!(
                phase,
                "after_staging" | "after_prepared" | "disk_full" | "permission_denied"
            );
            assert_eq!(
                fs::read(root.join("project.workspace")).unwrap(),
                if committed { b"new" } else { b"old" },
                "{phase}"
            );
            assert_eq!(root.join("resources/models/new.json").exists(), committed);
            assert_eq!(root.join("resources/models/old.json").exists(), !committed);
            fs::remove_dir_all(root).unwrap();
        }
    }
}
