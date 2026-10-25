#[test]
fn linked_library_matches_header_version_in_pinned_upstream() {
    // SAFETY: 两个无参查询无资源所有权，链接由唯一 sys 包保证。
    let (v, header) = unsafe {
        (
            lib60870_sys::Lib60870_getLibraryVersionInfo(),
            lib60870_sys::essim_60870_header_version(),
        )
    };
    assert_eq!((v.major, v.minor, v.patch), (2, 4, 0));
    assert_eq!(header, v.major * 10000 + v.minor * 100 + v.patch);
}
