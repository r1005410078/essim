fn main() {
    // Local tests/examples need the explicit dynamic prefix in their RUNPATH.
    // App packaging supplies its own relative runpath, never this development path.
    if let Ok(mode) = std::env::var("DEP_MODBUS_LINK_MODE") {
        if mode == "dynamic" {
            let lib = std::env::var("DEP_MODBUS_LIB_DIR").unwrap();
            println!("cargo:rustc-link-arg=-Wl,-rpath,{lib}");
        }
    }
}
