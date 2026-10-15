use libmodbus_src::{Build, LinkMode};
#[test]
fn rejects_cross_target_before_building() {
    let build = Build::new(
        std::env::temp_dir().join("essim-rejected-target"),
        LinkMode::Static,
    )
    .target("invalid-target");
    assert!(build.build().unwrap_err().to_string().contains("target"));
}

#[test]
fn missing_compiler_reports_tool_instead_of_using_system_library() {
    let build = Build::new(
        std::env::temp_dir().join("essim-missing-tool"),
        LinkMode::Static,
    )
    .compiler("/not/a/compiler");
    assert!(build
        .build()
        .unwrap_err()
        .to_string()
        .contains("需要构建工具"));
}
