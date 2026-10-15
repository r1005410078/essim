//! 本项目 C 辅助函数的非法参数与边界验证。
use libmodbus_sys as sys;
#[test]
fn local_helper_rejects_invalid_area_address_and_bit_value() {
    // SAFETY: 分配合法的四区映射，检查非空；整个测试串行访问且最终释放一次。
    let map = unsafe { sys::modbus_mapping_new_start_address(17, 1, 17, 1, 17, 1, 17, 1) };
    assert!(!map.is_null());
    let mut value = 2u16;
    // SAFETY: map 存活且独占，value 为有效不重叠的 u16；非法参数应由辅助函数拒绝。
    unsafe {
        for area in 0..4 {
            assert_eq!(sys::libmodbus_rs_value(map, area, 16, &mut value, 0), -1);
            assert_eq!(sys::libmodbus_rs_value(map, area, 18, &mut value, 0), -1);
        }
        for area in 0..2 {
            assert_eq!(sys::libmodbus_rs_value(map, area, 17, &mut value, 1), -1);
            assert_eq!(
                std::io::Error::last_os_error().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
        assert_eq!(sys::libmodbus_rs_value(map, 4, 17, &mut value, 0), -1);
        assert_eq!(
            sys::libmodbus_rs_value(map, 0, 17, std::ptr::null_mut(), 0),
            -1
        );
        assert_eq!(
            sys::libmodbus_rs_value(std::ptr::null_mut(), 0, 17, &mut value, 0),
            -1
        );
        sys::modbus_mapping_free(map);
    }
}
