fn main() {
    // 仅本地动态示例／测试使用显式前缀；应用打包另设相对 RUNPATH。
    if std::env::var("DEP_60870_LINK_MODE").as_deref() == Ok("dynamic") {
        let lib = std::env::var("DEP_60870_LIB_DIR").expect("动态库目录元数据缺失");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{lib}");
    }
}
