# libmodbus 三包拆分验收（2026-10-15）

## 结果与边界

完成 `crates/libmodbus/{libmodbus-src,libmodbus-sys,libmodbus-rs}` 拆分并接回 ESSim。默认 vendored 源码静态；非 vendored 只接受显式自行编译动态前缀。基础设施不再有 unsafe/原始 FFI/C 编译，仍负责 SOC、访问策略、worker、帧门禁和真实报文。

本次环境为 macOS 26.4.1 / arm64、target aarch64-apple-darwin；rustc 1.96.0、Apple clang 21.0.0、Autoconf 2.73、Automake 1.19、GNU libtool 2.6.2。独立客户端为 PyModbus 3.11.3，运行于临时 Python 3.14.4 环境。只有 1 个实例/客户端、1 个点，没有规模或真实 EMS 验收。

Linux GNU 有本机 target 的构建路径，未实际执行；Windows、交叉编译、macOS universal 和 musl 明确拒绝。Windows 的配置生成、DLL/import library、静态 dllimport 宏、CRT 与 SOCKET 问题仍待独立适配，不宣称已支持。

## 来源、构建与模式检查

官方 v3.2.0 对应提交 `a9b025d12289855490b10d77461c99e001abfc0f`。实际下载归档 SHA-256：`0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa`。逐文件比较确认 vendor 的 132 个普通文件保持原始内容，没有补丁；完整来源见 [SOURCE.md](../../crates/libmodbus/libmodbus-src/SOURCE.md)。

使用独立空目录 `target/libmodbus-offline-proof` 在网络受限环境执行 `cargo run -p libmodbus-sys --features vendored --example version --offline --target-dir target/libmodbus-offline-proof`，完成源码静态构建并输出 3.2.0；生成程序没有动态 libmodbus 依赖。正常构建仅在私有前缀安装，不修改 vendor 或系统 libmodbus。依赖和工具预先准备；首次 Rust 依赖获取不在离线承诺之内。

动态产物由 `cargo run -p libmodbus-src --example build-shared --offline -- target/libmodbus-shared` 生成，本机前缀为 `target/libmodbus-shared/aarch64-apple-darwin-Dynamic-debug/install`。两种模式的构建树、头文件、产物和清单分开。sys 校验来源/版本/target/模式/库校验和，C 探针核对所用 API 签名与常量。

[feature 记录](2026-10-15-libmodbus-feature-check.txt) 证明默认入口包含源码构建依赖，关闭默认 feature 后不再启用它。[失败诊断](2026-10-15-libmodbus-selection-failures.txt) 记录缺来源、vendored 与动态目录冲突、旧 LIBMODBUS_PREFIX 的真实构建失败；没有宿主机回退。

## 自动测试

两种模式分别运行完整 workspace 测试，各 19 个用例通过（包含 3 个 compile-fail doctest）：

- 保留核心规则与全部 TCP 回归：初值/确认更新、异常、只读、Unit ID、分段/合并、非法长度、额外连接、断连恢复、停机/端口恢复。
- 通用映射的非法区间/越界、完整 UInt16 和普通可写寄存器，证明 rs 未固化 SOC 规则。
- context 部分初始化失败注入、100 次循环与借用 socket 仍有效、正常 Drop 后对端 EOF。
- 截断请求超时；禁止任意切片作为 Request、context 跨线程、借用 socket 早于 context 释放的编译失败用例。
- 缺编译器、无效 target、显式链接选择、构建清单/库损坏等失败分支。

原始结果：[静态测试](2026-10-15-libmodbus-static-tests.txt)、[动态测试](2026-10-15-libmodbus-dynamic-tests.txt)。Rust 格式检查、Cargo 成员/依赖方向检查通过；两个 Tauri 包构建时均通过现有前端类型检查与生产构建。没有修改前端业务或原型。

没有执行 ASan、独立泄漏检测或长期压力测试；资源循环通过不等于完整内存安全证明。

## 独立互操作与实际打包

两种模式均使用 PyModbus：初值 650；草稿编辑700而未应用仍读650；工具栏确认后读700；越界读异常02、FC06/FC16写异常02、FC04异常01；关闭后重连仍能读取。教学映射仍是 Unit ID1、保持寄存器零起始地址100、倍率0.1%，不是厂商标准地址。

无界面入口同样使用应用服务，stdin 更新确认后读700，quit 停机并可重新绑定端口：

- [静态 headless](2026-10-15-libmodbus-static-headless.txt)
- [动态 headless](2026-10-15-libmodbus-dynamic-headless.txt)

Tauri/NovaSheet 实测：

| 证据 | 静态 | 动态 |
| --- | --- | --- |
| 初值读取 | [650](2026-10-15-libmodbus-static-tauri-650.txt) | [650](2026-10-15-libmodbus-dynamic-tauri-650.txt) |
| 草稿隔离 | [未应用](2026-10-15-libmodbus-static-tauri-draft.txt) | [未应用](2026-10-15-libmodbus-dynamic-tauri-draft.txt) |
| 确认后读取 | [700](2026-10-15-libmodbus-static-tauri-700.txt) | [700](2026-10-15-libmodbus-dynamic-tauri-700.txt) |
| 实际加载位置 | [记录](2026-10-15-libmodbus-static-loaded.txt) | [记录](2026-10-15-libmodbus-dynamic-loaded.txt) |
| 桌面截图 | [截图](2026-10-15-libmodbus-static-desktop.png) | [截图](2026-10-15-libmodbus-dynamic-desktop.png) |

静态包无动态 libmodbus 依赖；动态包实际加载 `target/artifacts/dynamic/ESSim.app/Contents/Frameworks/libmodbus.5.dylib`，不是 Homebrew 或构建前缀。打包脚本移除开发 rpath，使用相对 Frameworks 路径，两个包均通过 ad-hoc 签名与 strict 验证后实际启动。停止均释放监听；动态停止后另建 socket 成功 bind1502。

静态 libmodbus 仍允许系统框架动态依赖，不称整个程序完全静态。包是本机开发验证产物，未做开发者身份签名、公证或公开分发。LGPL 原文与源码来源附在 Resources；重新链接/替换库及其他适用的公开分发材料仍须在发布前核实。[完整构建与使用说明](../../crates/libmodbus/README.md)。

本次自动审批曾因额度无法完成而阻止动态700的最后一次读回；该次没有执行。用户继续后通过相同审批流程重试并完成，记录中700来自实际成功读回。
