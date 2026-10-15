# libmodbus 开发依赖（首轮接入历史记录）

> 2026-10-15 已由 [三包封装](../../crates/libmodbus/README.md) 替代当前构建方式。以下保留首轮 Homebrew 接入证据，不再是当前开发配置。

2026-10-14 在 macOS 26.4.1 / arm64 接入并验证 **3.2.0**。

- 官方标签：[v3.2.0](https://github.com/stephane/libmodbus/releases/tag/v3.2.0)。完整标签提交由官方仓库 `git ls-remote` 核实：`a9b025d12289855490b10d77461c99e001abfc0f`。
- 源码包：https://github.com/stephane/libmodbus/archive/refs/tags/v3.2.0.tar.gz。
- Homebrew 3.2.0 公式记录的源码 SHA-256：`0c61007b2815daf452618f8b877d15cdcd376b71ad5dbd06b330d70f53b2ccaa`。本次使用已安装 bottle，没有重新下载源码包，因此不宣称亲自计算了该归档的校验和。
- 实际库：`/opt/homebrew/Cellar/libmodbus/3.2.0/lib/libmodbus.5.dylib`；头文件 `/opt/homebrew/include/modbus`。构建默认前缀 `/opt/homebrew`，可用 `LIBMODBUS_PREFIX` 显式替换；C 编译检查头文件版本，创建上下文检查运行库版本。
- 许可：[LGPL-2.1-or-later](https://github.com/stephane/libmodbus/blob/v3.2.0/COPYING.LESSER)。本次动态链接开发库；尚未完成可分发安装包的依赖复制、许可附带与用户替换库方案。`--bundles app` 只用于本机桌面验证，不能当作独立分发安装包。

## 获取与构建

本机已有依赖，工程不自动全局安装。缺少依赖时由开发者显式安装 `brew install libmodbus`，并核实当前公式仍为 3.2.0；其他版本会被拒绝。需要完全固定来源时，从上述标签包构建：安装 C 编译器、autoconf、automake、libtool，再运行上游 `autogen.sh`、`configure --prefix=<开发目录>`、`make`、`make install`。设置 `LIBMODBUS_PREFIX` 指向该目录后运行 Cargo。构建前缀需要提供 `include/modbus` 与 `lib`，macOS 动态库安装名必须指向实际库位置。

Linux：采用同样的源码构建前缀和 `.so` 动态链接；发行版包常为其他版本，不能静默替代 3.2.0。当前未执行 Linux 构建/运行，Unix socketpair 与文件描述符路径仍需要平台验收。

Windows：当前明确拒绝编译该适配器。上游 [#863](https://github.com/stephane/libmodbus/issues/863) 报告 3.2.0 SOCKET / FD_SETSIZE 检查问题；本项目没有 Windows 验证证据，也不把 `SOCKET` 强转为 `i32`。后续须采用工具链匹配的 DLL/import library、Windows 内部传输适配及句柄测试。

## 核查与封装决定

实际使用 `modbus_new_tcp`、`modbus_mapping_new_start_address`、`modbus_set_socket`、`modbus_set_indication_timeout`、`modbus_set_byte_timeout`、`modbus_receive`、`modbus_reply`、`modbus_reply_exception`、`modbus_mapping_free`、`modbus_free`。mapping 布局只在包含官方头文件的 C 薄层中访问。

[固定版本接收代码](https://github.com/stephane/libmodbus/blob/v3.2.0/src/modbus.c) 按功能码推导接收长度，不能仅靠它完成 MBAP 字节流隔离。Rust 外层验证协议标识、长度与已支持的 PDU 形状后，通过 worker 内部 Unix socketpair 将完整请求交给 `modbus_receive`；`modbus_reply` 的真实输出由另一端读取，再有界发送给外部 TCP 客户端。这避免捏造响应日志，也使外部 TCP 停机不受 C 的阻塞读影响；没有自己编码寄存器应答。

所有外部 TCP、内部 socketpair 均由 Rust 唯一持有；C 只借用内部 fd，不调用 `modbus_close`。C 上下文仅在协议线程创建/销毁，Rust wrapper 使用 `PhantomData<Rc<()>>` 禁止跨线程移动；无 C 回调。先释放 mapping/context，再释放借用 socket。初始化失败注入覆盖获取 context 后、mapping 后的清理，循环创建/销毁覆盖 100 次；未做 ASan/泄漏检测，不宣称有完整内存安全证明。

监听/外部读取非阻塞，每轮检查消息；内部读写最长 100ms，半帧等待 200ms，对外发送最长 100ms。停机发送控制消息，轮询退出，worker 收回所有资源后 join；本实现不需要另一个线程 shutdown C 使用的句柄。
