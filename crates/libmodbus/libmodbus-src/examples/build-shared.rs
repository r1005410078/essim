fn main() -> std::io::Result<()> {
    let out = std::env::args_os().nth(1).ok_or_else(|| {
        std::io::Error::other(
            "用法：cargo run -p libmodbus-src --example build-shared -- <独立输出目录>",
        )
    })?;
    let artifacts = libmodbus_src::Build::new(out, libmodbus_src::LinkMode::Dynamic).build()?;
    println!("LIBMODBUS_DIR={}", artifacts.prefix.display());
    Ok(())
}
