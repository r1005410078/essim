# libmodbus 四类数据区验收（2026-10-15）

## 范围与环境

本轮扩展通用安全封装，支持线圈、离散输入、保持寄存器、输入寄存器的独立映射。当前 SOC 示例仍只使用地址 100 的保持寄存器；不新增点表 UI、IPC 或设备点位。示例和测试地址 17、100 均为教学/测试约定。

本机 macOS arm64，Rust 1.96.0，target 为 aarch64-apple-darwin；上游仍为内置的 libmodbus 3.2.0，无 vendor 修改。静态使用 vendored，动态使用此前显式构建的 target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install。

## 测试过程与结果

先增加 AddressRange/Mapping 测试，确认因新 API 不存在而编译失败，再实现封装及 C 辅助函数。

执行：

```sh
cargo test -p libmodbus-rs -p libmodbus-sys -p essim-infrastructure --offline
LIBMODBUS_DIR="$PWD/target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install" cargo test -p libmodbus-rs -p libmodbus-sys -p essim-infrastructure --no-default-features --offline
cargo doc -p libmodbus-rs -p libmodbus-sys --features libmodbus-sys/vendored --no-deps --offline
openspec validate extract-libmodbus-workspace --strict
```

静态和动态各通过 14 个单元/集成测试、4 个文档测试。首次 TCP 回归因沙箱禁止本地端口绑定而失败，获得测试权限后重跑通过。修改的四个 Rust 文件通过 rustfmt --check --edition 2021；文档构建及 OpenSpec 严格校验通过。

覆盖：

- 四类区初始值、相同地址互不污染、未分配区错误、空/越界配置拒绝、地址 65535 和完整 u16 范围。
- 内部 socketpair 的实际 libmodbus receive/reply 路径：FC01/02/03/04 读取、FC05/06 单写、FC0F/10 多写。
- 10 个线圈跨字节低位优先打包、非法线圈值异常 03、四区非法地址异常 02；写输出区后输入区值不变。
- C 辅助函数的非法数据区、空指针、越界和位值拒绝。
- 原 SOC TCP 五项回归：更新与生命周期、分段/合并/非法帧、额外连接、订阅有界及慢客户端停机。

测试中的示例 PDU（十六进制；完整 MBAP 由测试固定提供）：

| 操作 | 请求 PDU | 预期且实际返回的 PDU |
| --- | --- | --- |
| 写 10 个线圈 | 0F 00 11 00 0A 02 81 03 | 0F 00 11 00 0A |
| 读回 10 个线圈 | 01 00 11 00 0A | 01 02 81 03 |
| 读输入寄存器 | 04 00 11 00 01 | 04 02 12 34 |
| 读不存在的输入地址 | 04 00 1B 00 01 | 84 02 |

## 边界

新增四区测试使用固定请求和预期字节经过 C 库，不是新增独立客户端互操作证据；本轮未重跑 PyModbus、Tauri 桌面或打包验收，未测试真实 EMS、Linux/Windows、RTU 或规模性能。原独立互操作记录仅证明其记录的 SOC 范围。
