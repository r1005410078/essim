use lib60870_src::{select, Build, LinkMode, Selection};
#[test]
fn source_selection_never_falls_back_to_system() {
    assert!(select(false, None).is_err());
    assert!(select(true, Some("/tmp/native".as_ref())).is_err());
    assert!(select(false, Some("".as_ref())).is_err());
    assert!(select(false, Some("relative/native".as_ref())).is_err());
    assert_eq!(select(true, None).unwrap(), Selection::Vendored);
    assert_eq!(
        select(false, Some("/tmp/native".as_ref())).unwrap(),
        Selection::Dynamic("/tmp/native".into())
    );
}
#[test]
fn unsupported_target_and_profile_fail_before_native_build() {
    assert!(Build::new("/unused", LinkMode::Static)
        .target("x86_64-pc-windows-msvc")
        .build()
        .is_err());
    assert!(Build::new("/unused", LinkMode::Static)
        .profile("unknown")
        .build()
        .is_err());
}

#[test]
fn real_artifacts_reject_tampering_and_build_keeps_vendor_pristine() {
    use lib60870_src::{host_target, source_digest, Artifacts, SOURCE_TREE_SHA256};
    use std::fs;
    let root = std::env::temp_dir().join(format!(
        "essim-60870-build-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let a = Build::new(&root, LinkMode::Static).build().unwrap();
    let target = host_target().unwrap();
    assert_eq!(source_digest().unwrap(), SOURCE_TREE_SHA256);
    assert!(Artifacts::load(&a.prefix, "wrong-target", LinkMode::Static).is_err());
    assert!(Artifacts::load(&a.prefix, &target, LinkMode::Dynamic).is_err());
    let original = fs::read(&a.library).unwrap();
    fs::write(&a.library, b"corrupt library").unwrap();
    assert!(Artifacts::load(&a.prefix, &target, LinkMode::Static).is_err());
    fs::write(&a.library, original).unwrap();
    let header = a.include.join("lib60870/cs104_slave.h");
    let original = fs::read(&header).unwrap();
    fs::write(&header, b"corrupt header").unwrap();
    assert!(Artifacts::load(&a.prefix, &target, LinkMode::Static).is_err());
    fs::write(header, original).unwrap();
    let manifest = a.prefix.join("lib60870-build.json");
    let mut m: serde_json::Value = serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
    m["library"] = "../escape".into();
    fs::write(&manifest, m.to_string()).unwrap();
    assert!(Artifacts::load(&a.prefix, &target, LinkMode::Static).is_err());
    fs::remove_dir_all(root).unwrap();
}
