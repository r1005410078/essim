use sha2::{Digest, Sha256};
use std::{
    fs,
    path::{Path, PathBuf},
};
#[derive(Debug, PartialEq, Eq)]
pub enum Selection {
    Vendored,
    Dynamic(PathBuf),
}
pub fn select(
    vendored: bool,
    directory: Option<&str>,
    legacy: Option<&str>,
) -> Result<Selection, String> {
    if legacy.is_some() {
        return Err("LIBMODBUS_PREFIX 已废弃，请使用 vendored 或 LIBMODBUS_DIR".into());
    }
    match (vendored, directory) {
        (true, None) => Ok(Selection::Vendored),
        (false, Some(p)) if Path::new(p).is_absolute() => Ok(Selection::Dynamic(p.into())),
        (true, Some(_)) => Err("vendored 与 LIBMODBUS_DIR 冲突，请只选一种来源".into()),
        _ => Err("需要 vendored，或 LIBMODBUS_DIR 指向自行编译动态库的绝对前缀".into()),
    }
}
pub fn dynamic(prefix: &Path, target: &str) -> Result<(PathBuf, PathBuf, String), String> {
    let prefix = fs::canonicalize(prefix).map_err(|e| e.to_string())?;
    let m: serde_json::Value = serde_json::from_slice(
        &fs::read(prefix.join("libmodbus-build.json"))
            .map_err(|e| format!("缺少自行编译清单：{e}"))?,
    )
    .map_err(|e| e.to_string())?;
    for (key, value) in [
        ("version", "3.2.0"),
        ("commit", "a9b025d12289855490b10d77461c99e001abfc0f"),
        (
            "archive_sha256",
            "0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa",
        ),
        ("target", target),
        ("mode", "dynamic"),
    ] {
        if m[key].as_str() != Some(value) {
            return Err(format!("构建清单 {key} 不匹配"));
        }
    }
    let filename = m["library"].as_str().ok_or("清单缺少库名")?;
    if Path::new(filename).components().count() != 1 || !filename.starts_with("libmodbus") {
        return Err("非法库名".into());
    }
    let include = prefix.join("include");
    let lib = prefix.join("lib");
    for name in ["modbus.h", "modbus-version.h"] {
        if !include.join("modbus").join(name).is_file() {
            return Err(format!("缺少头文件 {name}"));
        }
    }
    let hash = format!(
        "{:x}",
        Sha256::digest(fs::read(lib.join(filename)).map_err(|e| e.to_string())?)
    );
    if m["library_sha256"].as_str() != Some(&hash) {
        return Err("动态库校验和不匹配".into());
    }
    Ok((include, lib, filename.into()))
}
