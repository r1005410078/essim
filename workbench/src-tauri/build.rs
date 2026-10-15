fn main() {
    println!("cargo:rerun-if-env-changed=LIBMODBUS_DIR");
    if std::env::var_os("CARGO_FEATURE_VENDORED").is_none() {
        if let Some(prefix) = std::env::var_os("LIBMODBUS_DIR") {
            let lib = std::path::PathBuf::from(prefix).join("lib");
            if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
                println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
            }
            // Development executables run directly from target. The macOS
            // bundling helper removes this absolute path before signing.
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
        }
    }
    tauri_build::build();
}
