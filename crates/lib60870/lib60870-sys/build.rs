use lib60870_src::{Artifacts, Build, LinkMode, Selection};
use std::{env, path::PathBuf};
fn main() {
    for name in [
        "LIB60870_DIR",
        "CC",
        "CFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "AR",
        "SDKROOT",
        "MACOSX_DEPLOYMENT_TARGET",
        "CMAKE",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    println!("cargo:rerun-if-changed=../lib60870-src/vendor");
    println!("cargo:rerun-if-changed=native/abi.c");
    let target = env::var("TARGET").unwrap();
    assert!(
        env::var("HOST").unwrap() == target
            && (target.ends_with("apple-darwin") || target.contains("linux-gnu")),
        "未支持的 lib60870 target，禁止宿主库回退"
    );
    let a = match lib60870_src::select(
        cfg!(feature = "vendored"),
        env::var_os("LIB60870_DIR").as_deref(),
    )
    .unwrap_or_else(|e| panic!("{e}"))
    {
        Selection::Vendored => Build::new(
            PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("native"),
            LinkMode::Static,
        )
        .target(&target)
        .profile(env::var("PROFILE").unwrap())
        .build()
        .unwrap_or_else(|e| panic!("{e}")),
        Selection::Dynamic(prefix) => {
            println!("cargo:rerun-if-changed={}", prefix.display());
            Artifacts::load(&prefix, &target, LinkMode::Dynamic).unwrap_or_else(|e| panic!("{e}"))
        }
    };
    cc::Build::new()
        .file("native/abi.c")
        .include(a.include.join("lib60870"))
        .warnings_into_errors(true)
        .compile("essim_60870_abi");
    let static_link = a.manifest.mode == LinkMode::Static;
    println!("cargo:rustc-link-search=native={}", a.lib.display());
    println!(
        "cargo:rustc-link-lib={}=lib60870",
        if static_link { "static" } else { "dylib" }
    );
    if target.contains("linux") {
        println!("cargo:rustc-link-lib=pthread");
        println!("cargo:rustc-link-lib=m");
    }
    if !static_link {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", a.lib.display());
    }
    println!("cargo:include={}", a.include.display());
    println!("cargo:lib_dir={}", a.lib.display());
    println!(
        "cargo:runtime_library={}",
        if static_link {
            String::new()
        } else {
            a.library.display().to_string()
        }
    );
    println!(
        "cargo:link_mode={}",
        if static_link { "static" } else { "dynamic" }
    );
}
