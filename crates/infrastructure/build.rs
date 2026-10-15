fn main() {
    println!("cargo:rerun-if-env-changed=LIBMODBUS_DIR");
    if std::env::var_os("CARGO_FEATURE_VENDORED").is_none() {
        if let Some(prefix) = std::env::var_os("LIBMODBUS_DIR") {
            let lib = std::path::PathBuf::from(prefix).join("lib");
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
        }
    }
}
