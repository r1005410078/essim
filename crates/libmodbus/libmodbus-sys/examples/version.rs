fn main() {
    unsafe {
        println!(
            "{}.{}.{}",
            libmodbus_sys::libmodbus_version_major,
            libmodbus_sys::libmodbus_version_minor,
            libmodbus_sys::libmodbus_version_micro
        );
    }
}
