## Context

正式工程只有 `getBackendInfo` 的 IPC 骨架，`essim-core` 不依赖 Tauri，基础设施 crate 尚未实现协议。`prototype/` 为独立本地业务原型，不作为真实通信代码迁移来源。本提案的实现进度与证据见 tasks.md 和 docs/test-records；原型保持独立。

当前工作机为 macOS；实施时发现已安装 Homebrew libmodbus 3.2.0，`pkg-config` 不可用，因此采用显式前缀定位。先在当前平台完成可复现闭环，同时列出 Linux、Windows 的构建与运行验证项，未运行的平台标记未验证。

## Goals / Non-Goals

**Goals:**

- 独立客户端通过真实 TCP 读取教学 SOC；在工作台修改运行值后重新读取可观察新值。
- Rust 安全封装集中管理 C 上下文、寄存器映射和套接字；失败、断连、停机均可收回资源。
- 验证 Tauri 与未来远程适配可共用的业务接口，核心可脱离窗口运行。
- 将原始报文、异常响应及独立互操作结果记录为可复现证据。

**Non-Goals:**

- RTU、104、61850、策略、通用点表编辑/导入、多设备规模、SQLite、Qt 和 HTTP 服务。
- 本阶段不实现 EMS 客户端、Float32、多寄存器字序、外部控制动作或协议认证。
- 不宣称成熟库自动解决所有恶意帧、时序或跨平台兼容问题。

## Decisions

### 1. 固定 libmodbus 3.2.0 候选，先通过构建与报文验证门槛

官方发布页记录 v3.2.0 为 2026-07-02 发布版本，并包含请求长度、句柄生命周期等修复。以该发布标签为候选基线，实施时记录完整源码提交、发布包校验和、实际头文件及运行库版本；不能在构建时拉取 master 或静默使用不同系统版本。

先使用可明确指定路径的动态链接（macOS `.dylib`、Linux `.so`、Windows DLL/import library），由构建脚本核实头文件与库版本。Unix 优先 `pkg-config`，提供显式安装前缀路径作为备选；Windows 使用目标工具链匹配的已验证库，不混用 MSVC/MinGW 产物。构建前置任务记录如何获取、构建和定位库；不把系统全局安装当作代码构建脚本的隐式副作用。未接入运行时库时给出可操作错误，不能伪造引擎就绪。

库源码声明 LGPL-2.1-or-later，实施时保存其许可证与来源；动态链接并不免除分发义务，发布打包前单独记录依赖分发与可替换库方案。当前仅开发接入，不宣称已完成三平台安装包。

选择 libmodbus 而非自写 TCP 协议栈，符合复用成熟库的产品方向；选择薄层封装而非直接采用未经核查的第三方 Rust wrapper，便于明确本次最小 API 面与资源所有权。

### 2. 教学映射仅包含一个只读 SOC

| 项 | 教学约定 |
| --- | --- |
| 设备 ID / 点 ID | 稳定 ID，例如 `demo-bms` / `soc`，不使用行号 |
| 默认端点 | `127.0.0.1:1502`，可配置 IPv4 和 1–65535 端口 |
| Unit ID | 固定 1，本次不做网关路由；其他 ID 关闭该客户端连接并记录原因 |
| 数据区 | Holding Registers，FC03 |
| 零起始协议地址 | 100 / `0x0064`，不是手册式 `40001` 地址 |
| 类型与长度 | UInt16，1 个寄存器，无跨寄存器字序 |
| 工程换算 | 工程值 = 原始值 × 0.1%，初始 650 = 65.0% |
| 合法范围 | 原始整数 0–1000，工程值 0.0–100.0%，精度 0.1% |
| 访问 | 外部协议只读；工作台修改独立的运行值，不修改模型初始值 |

只有完整落在地址 100、长度 1 内的读取成功。FC03 越界返回异常 02，数量为 0 或超过规范上限返回异常 03；FC06/FC16 写 SOC 返回异常 02，其他正常请求的未支持功能返回异常 01。非法 MBAP/PDU 不保证异常响应，必须在有限时间内关闭连接且不改变状态。禁止把所有 libmodbus 默认支持的写功能自动暴露。

示例请求 `00 01 00 00 00 06 01 03 00 64 00 01`，初值响应 `00 01 00 00 00 05 01 03 02 02 8A`。事务号回显，单寄存器线上高字节在前。地址、倍率与这些点值均为教学约定，协议字段规则按 Modbus 官方应用/TCP 规范验证。

### 3. 分层与接口

- 核心领域：SOC 值校验、稳定 ID、端点配置、生命周期与运行快照，不依赖 C 指针或 Tauri。
- 核心应用：`start`、`stop`、`setRuntimeValue`、`getSnapshot`、`subscribe/unsubscribe`；通过协议执行端口发消息。
- 基础设施：`protocols/modbus/ffi` 与 `adapter`，实现库调用和 TCP 生命周期。
- Tauri：持有应用服务句柄，把 command/channel 转为业务操作与快照。业务页面只依赖 `EssimClient`。
- 无界面入口：放在基础设施示例或专用验证二进制中，复用同一适配器与应用接口，不建立独立 HTTP 服务。

前端请求带稳定实例 ID；更新运行值带点 ID 与原始整数（UInt16），工程值仅用于输入换算与展示。批次编号使用 JS 可安全表达的整数或字符串，不能把无界限 u64 直接传为 JS number。错误带 `code/message`，字段校验含点/字段定位。状态中监听、客户端已连接与引擎可用分开表达，不能把依赖已编译视为从站已经监听。

### 4. 生命周期和状态所有权

单实例状态为 Stopped → Starting → Listening → Stopping → Stopped；启动或运行不可恢复错误进入 Failed，可明确重启。客户端连接数量作为独立字段，单实例首版最多处理一个活跃客户端，额外连接不允许接管当前上下文。启动成功只在真实 bind/listen 完成后返回；重复启动给出 AlreadyRunning，停止幂等；Failed 状态保留错误说明。

使用一个协议 worker 独占上下文、mapping 及运行状态，并执行核心领域校验；外部只能发送消息、查询快照和订阅事件，不直接跨线程访问 C 指针。worker 在请求处理之间及有界等待期间消费控制消息。运行值更新确认必须在 worker 更新状态与寄存器映射后发出，确认后的下一次有效请求必须读到新值。

接收帧与生成响应使用 `modbus_receive` / `modbus_reply`；仅在完整、已校验请求上进行 Unit ID、功能码与只读策略检查，不自行建设另一套通用编解码。调用边界检查返回长度与缓冲区上限，异常由 `modbus_reply_exception` 返回。须针对候选版本实际代码核实长度校验行为；不能仅凭 API 文档认定非法帧已被处理。

### 5. RAII、停机与阻塞边界

- 上下文、mapping 使用独立 RAII 包装，空指针转换为错误；mapping 数组访问集中在 ffi 内并有边界说明。
- worker 内创建和销毁 C 对象，不实现未经证明的 `Send/Sync`。本阶段不需要 C 回调；以后增加回调时必须防止 Rust panic 跨越 ABI。
- 监听 accept 使用可轮询/有界等待；接收采用 indication/byte timeout（建议 100ms），核实服务端超时语义，不拿客户端 response timeout 替代。
- 监听 socket 与已连接 socket 分别列明所有者。若采用 Rust accept 后 `modbus_set_socket`，转交原始句柄的所有权必须一次完成，不能 Rust TcpStream 与 `modbus_close` 双重释放。Windows SOCKET 不得未经核实截断为平台通用整数。
- 为停机准备受控的 socket shutdown 唤醒路径，不从另一线程释放上下文；取消消息 → 唤醒阻塞 I/O → worker 退出并关闭连接 → 释放 mapping/context → 关闭监听 → join 完成后才报告 Stopped。正常停机目标 1s 内完成，测试覆盖空闲 accept、空闲客户端、半帧及客户端不读取响应等情况。
- 初始化失败沿已获取资源逆序清理。收到断连或非法帧仅清理当前客户端，监听实例继续服务后续连接。

### 6. 最小工作台与观察

现有 Dockview 工作区增加一个单从站教学面板：IPv4/端口、启动/停止、SOC 原始值与工程值、更新运行值、监听状态和客户端数量。停止状态修改 SOC 作为本会话下次启动值，首次初始化为 650；重启保留会话运行值，退出程序不持久化。

快照先于增量订阅，事件带实例 ID 与序号；订阅取消释放资源，不能每打开一次页面增加不可回收线程。当前只推送生命周期、连接变化、运行值更新与错误，报文记录有界；不逐点无界推送。实际收发原始字节要区分已收到请求、成功发送响应与发送失败，不伪造响应记录。

### 7. 验证与证据

先写失败用例验证 SOC 范围、启停状态及更新确认，再实现。网络测试包括合法读取、越界、非法数量、只读拒绝、Unit ID 不匹配、分段/合并、非法长度、客户端断连重连、占用端口、空闲及半帧停机、停止后重新监听。TCP 分段/合并检验字节流处理，不称为网络丢包。

至少使用一个与 libmodbus 不同实现的独立客户端（例如固定版本 PyModbus 或现有 EMS）读取 650，更新到 700 后读回 700，验证异常响应与重连。手写测试报文只作为专项测试，不能替代独立工具证据。记录平台、版本、参数、原始请求/响应、预期/实际结果与覆盖边界；没有性能测试就不报告规模。

## Risks / Trade-offs

- [候选库版本存在平台/报文处理缺陷] → 回归门槛先于业务接入；3.2.0 Windows SOCKET 问题已在上游报告，需确认解决状态并复现。遇到库缺陷记录并选择经验证补丁/版本，先更新提案，不静默放宽测试。
- [阻塞 C I/O 延迟更新和停机] → 有界轮询、超时和 shutdown 唤醒；用更新确认与停机时限测试验证。
- [动态库找不到或 ABI 不匹配] → 固定头文件/运行库、构建检查和启动诊断，记录实际平台验证。
- [mapping 默认写入破坏只读点] → 完整请求策略门禁和写入拒绝回归，不能直接无条件调用 modbus_reply。
- [首版单客户端覆盖有限] → 明确不是多设备/多连接完成状态，后续独立提案扩展。

## Migration Plan

无配置数据库迁移。正式骨架逐步增加依赖、安全封装、核心用例、无界面验证和 IPC/UI；每阶段先测试。保留原有核心信息查询和 `prototype/`。运行失败停止实例并释放资源；回退时移除本变更的协议入口和依赖，不修改原型或其他项目。

## Open Questions

- 当前 macOS、后续 Linux/Windows 的可复现 C 库构建路径在第一项实施任务确定，Windows 已知 SOCKET 问题未经本机验证。
- 独立互操作先选能获取的客户端，实施记录必须固定具体版本；真实 EMS 不可用时不得声称 EMS 验证通过。
- 停机唤醒所需平台 API 和所有权转交必须结合固定版本头文件/源码证明；若无法满足时限，调整设计后再继续。

## References

- [v3.2.0 发布](https://github.com/stephane/libmodbus/releases/tag/v3.2.0)
- [源码许可说明](https://github.com/stephane/libmodbus/blob/v3.2.0/README.md) 与 [LGPL 文本](https://github.com/stephane/libmodbus/blob/v3.2.0/COPYING.LESSER)
- [modbus_new_tcp](https://libmodbus.org/reference/modbus_new_tcp/)、[mapping](https://libmodbus.org/reference/modbus_mapping_new_start_address/)、[receive](https://libmodbus.org/reference/modbus_receive/)、[reply_exception](https://libmodbus.org/reference/modbus_reply_exception/)、[indication timeout](https://libmodbus.org/reference/modbus_set_indication_timeout/)
- [Windows 3.2.0 SOCKET 问题报告](https://github.com/stephane/libmodbus/issues/863)（问题报告不是本项目验证结论）
- [Modbus 官方规范入口](https://www.modbus.org/modbus-specifications)

## 实施决定（2026-10-14）

- 构建使用 `LIBMODBUS_PREFIX` / 本机 Homebrew 默认前缀，固定头文件与运行库 3.2.0；不引入 pkg-config 的隐式依赖。Linux 未执行；Windows 明确拒绝编译，等待 SOCKET 边界验证。
- 外部 TCP 使用 Rust 非阻塞传输；完整帧通过同一 worker 的 Unix socketpair 交给 libmodbus receive/reply。该库生成的真实响应通过 socketpair 读出，再有界发送给客户端，区别发送成功/失败。避免重复实现寄存器应答及伪造报文。
- Rust 唯一拥有所有 socket，C 只借用内部 fd。取消通过每 5ms 控制消息轮询及 100ms I/O 上限完成，无跨线程释放或 shutdown C 对象；退出资源回收后 join。
- 前端序号/代次使用十进制字符串；队列为 64 个完整快照，满队列合并，Tauri 订阅桥查询最新快照补齐最后状态；日志最多 128 条。应用退出自动停止。
- 按用户要求表格复用原型 vendor NovaSheet core 0.17.3 / canvas2d 0.6.6 / react 0.4.14。草稿和已生效值分开。该快照 React 适配器未转发 `isCellEditable`，改用支持的 `cellTypes` 只读类型限制模型与确认值，不修改 NovaSheet 项目或原型。其 vendor 开发 JSX 调用在生产包通过 importer 定向运行时兼容层处理。
