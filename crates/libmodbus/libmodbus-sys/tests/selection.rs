#[path = "../build_support.rs"]
mod support;
#[test]
fn explicit_selection_never_falls_back_to_host() {
    assert_eq!(
        support::select(true, None, None).unwrap(),
        support::Selection::Vendored
    );
    assert!(support::select(false, None, None).is_err());
    assert!(support::select(true, Some("/tmp/library"), None).is_err());
    assert!(support::select(false, Some("relative"), None).is_err());
    assert!(support::select(true, None, Some("/opt/homebrew")).is_err());
    assert!(support::dynamic(std::path::Path::new("/nonexistent-libmodbus"), "invalid").is_err());
}
#[test]
fn rejects_invalid_provenance_and_corrupted_artifacts() {
    use sha2::{Digest, Sha256};
    use std::fs;
    let dir = std::env::temp_dir().join(format!("libmodbus-manifest-test-{}", std::process::id()));
    fs::create_dir_all(dir.join("include/modbus")).unwrap();
    fs::create_dir_all(dir.join("lib")).unwrap();
    fs::write(dir.join("include/modbus/modbus.h"), "").unwrap();
    fs::write(dir.join("include/modbus/modbus-version.h"), "").unwrap();
    fs::write(dir.join("lib/libmodbus.dylib"), b"fixture").unwrap();
    let good = serde_json::json!({"version":"3.2.0","commit":"a9b025d12289855490b10d77461c99e001abfc0f","archive_sha256":"0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa","target":"fixture-target","mode":"dynamic","library":"libmodbus.dylib","library_sha256":format!("{:x}",Sha256::digest(b"fixture"))});
    fs::write(dir.join("libmodbus-build.json"), good.to_string()).unwrap();
    assert!(support::dynamic(&dir, "fixture-target").is_ok());
    for (key, value) in [
        ("version", "wrong"),
        ("target", "wrong"),
        ("mode", "static"),
        ("library_sha256", "wrong"),
        ("library", "../escape"),
    ] {
        let mut invalid = good.clone();
        invalid[key] = value.into();
        fs::write(dir.join("libmodbus-build.json"), invalid.to_string()).unwrap();
        assert!(support::dynamic(&dir, "fixture-target").is_err());
    }
    fs::remove_dir_all(dir).unwrap();
}
