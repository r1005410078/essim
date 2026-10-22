# serialport 来源与用途

- 版本：4.10.1；Cargo.lock 固定来源及校验值。
- 上游：https://github.com/serialport/serialport-rs
- 发布源码：https://crates.io/api/v1/crates/serialport/4.10.1/download
- 许可：MPL-2.0，正文见 LICENSE.txt。
- ESSim 仅调用未修改库的 `available_ports()` 发现设备，不用它打开现有 libmodbus 通信端口。
- 系统可能不提供设备名称或枚举虚拟串口；允许手动路径，不把枚举结果视为可连接保证。
