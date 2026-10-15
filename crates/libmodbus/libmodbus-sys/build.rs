mod build_support;
use std::env;
fn main() {
    for key in ["LIBMODBUS_DIR", "LIBMODBUS_PREFIX"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
    for name in ["native/mapping.c", "build_support.rs"] {
        println!("cargo:rerun-if-changed={name}");
    }
    let target = env::var("TARGET").unwrap();
    if env::var("HOST").unwrap() != target
        || !(target.ends_with("apple-darwin") || target.contains("linux-gnu"))
    {
        panic!("target {target} 尚未验证，禁止回退宿主库或截断 Windows SOCKET");
    }
    let mode = build_support::select(
        cfg!(feature = "vendored"),
        env::var("LIBMODBUS_DIR").ok().as_deref(),
        env::var("LIBMODBUS_PREFIX").ok().as_deref(),
    )
    .unwrap_or_else(|e| panic!("{e}"));
    let (include, lib, runtime, is_static) = match mode {
        build_support::Selection::Vendored => {
            #[cfg(feature = "vendored")]
            {
                println!("cargo:rerun-if-changed=../libmodbus-src/vendor");
                let a = libmodbus_src::Build::new(
                    std::path::PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("native"),
                    libmodbus_src::LinkMode::Static,
                )
                .target(&target)
                .build()
                .unwrap_or_else(|e| panic!("{e}"));
                (a.include, a.lib, String::new(), true)
            }
            #[cfg(not(feature = "vendored"))]
            unreachable!()
        }
        build_support::Selection::Dynamic(prefix) => {
            println!("cargo:rerun-if-changed={}", prefix.display());
            let (i, l, f) =
                build_support::dynamic(&prefix, &target).unwrap_or_else(|e| panic!("{e}"));
            let runtime = l.join(f).display().to_string();
            (i, l, runtime, false)
        }
    };
    cc::Build::new()
        .file("native/mapping.c")
        .include(&include)
        .warnings_into_errors(true)
        .compile("libmodbus_rs_mapping");
    println!("cargo:rustc-link-search=native={}", lib.display());
    println!(
        "cargo:rustc-link-lib={}=modbus",
        if is_static { "static" } else { "dylib" }
    );
    if !is_static {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    }
    println!("cargo:include={}", include.display());
    println!("cargo:lib_dir={}", lib.display());
    println!("cargo:runtime_library={runtime}");
    println!(
        "cargo:link_mode={}",
        if is_static { "static" } else { "dynamic" }
    );
}
