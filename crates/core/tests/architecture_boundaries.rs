//! 架构约束回归，防止领域反向引用外层、兼容门面重新积累逻辑及查询触发写入。
use std::{fs, path::Path};
#[test]
fn domain_sources_do_not_import_outer_layers() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/domain");
    for path in rust_sources(&root) {
        if path.extension().is_some_and(|x| x == "rs") {
            let source = fs::read_to_string(&path).unwrap();
            for forbidden in [
                "crate::application",
                "crate::infrastructure",
                "essim_infrastructure::",
                "tauri::",
            ] {
                assert!(
                    !source.contains(forbidden),
                    "{}不应引用{forbidden}",
                    path.display()
                );
            }
        }
    }
}
#[test]
fn compatibility_facades_only_export_and_queries_have_no_write_ports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    for path in [
        root.join("domain/foundation.rs"),
        root.join("application/workspace.rs"),
    ] {
        let source = fs::read_to_string(&path).unwrap();
        for forbidden in ["fn ", "impl ", "struct ", "enum "] {
            assert!(!source.contains(forbidden), "{}只能重导出", path.display());
        }
    }
    let queries = rust_sources(&root.join("application/queries"))
        .into_iter()
        .map(|path| fs::read_to_string(path).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
    for forbidden in [
        "ConfigRepository",
        "ApplyBoundary",
        "commands::",
        ".commit(",
        ".stop(",
        ".execute(",
    ] {
        assert!(!queries.contains(forbidden), "只读查询不应引入{forbidden}");
    }
}

fn rust_sources(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths = Vec::new();
    for entry in fs::read_dir(root).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            paths.extend(rust_sources(&path));
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            paths.push(path);
        }
    }
    paths
}
