//! 将固定源码构建为私有动态前缀，不安装到系统。
fn main() -> std::io::Result<()> {
    let out = std::env::args_os()
        .nth(1)
        .ok_or_else(|| std::io::Error::other("需要输出目录"))?;
    let artifacts = lib60870_src::Build::new(out, lib60870_src::LinkMode::Dynamic).build()?;
    println!("{}", artifacts.prefix.display());
    Ok(())
}
