# 动态 Unit ID 验收（2026-10-15）

## 实现与范围

同一 TCP 端点下增加单元。当前产品配置范围 1–247（不等同于 Modbus TCP 所有 Unit ID 用途）；默认单元 1，每个新单元初始 SOC 为 650，各自使用保持寄存器零基地址 100。地址、倍率与初值均为教学约定。旧单点 API 默认指向单元 1。

新增命令在 worker 创建独立 Native/context/mapping 后确认，随后发布包含全部 units 的快照。重复、非法 ID 被拒绝。C 对象由 worker 独占，停止释放映射后，重启按会话状态重建；退出应用不持久化。未知单元请求仍关闭连接，不将数据路由到单元 1。

界面在通信栏提供 Unit ID 下拉框、数字输入与 ＋，停止和监听时均可添加；成功后选中新单元。每个单元保留独立表格草稿和生效值。应用 SOC 将选中 unitId 传入统一客户端；Tauri 适配器是唯一前端 IPC 入口。

## 验证

本机 macOS arm64 / aarch64-apple-darwin，Rust 1.96.0，libmodbus 3.2.0。

- 静态和显式动态分别运行 `cargo test -p essim-core -p essim-infrastructure --offline`（动态增加 `--no-default-features` 并设置现有 `LIBMODBUS_DIR`）：各 3 个核心测试、6 个 TCP/订阅测试通过。
- 新增 TCP 用例覆盖非法/重复 ID、停止时添加、监听中添加、不同 SOC 读回（单元 1=650、2=720、3=810）、未知单元、失败不污染集合、停止重启保留。
- `pnpm --filter @essim/workbench test`：8 个测试通过，包括草稿隔离、相同生效值时切换、日志快照不覆盖草稿、非法/重复 ID 及 IPC 定向传参。
- `pnpm --filter @essim/workbench build`、`cargo check --workspace --offline`、`pnpm --filter @essim/workbench tauri build --debug --no-bundle` 通过；Vite 仍提示现有较大 chunk。
- 修改的 Rust 文件通过 rustfmt 检查；OpenSpec 严格校验通过。

## 桌面与独立客户端

使用新构建二进制的隔离本地验证 app（target/unit-check/ESSimUnitCheck.app），通过系统 WebView 实际操作：重复 ID 显示“Unit ID 已存在”；停止状态添加单元 4 后选中 4；在 127.0.0.1:1532 监听中添加单元 5 后仍为监听状态，单元数增加并选中 5；下拉切换回 1；停止按钮恢复“已停止”。

独立 PyModbus 3.11.3 使用现有 Python 环境执行：

```sh
/private/tmp/essim-pymodbus-314/bin/python scripts/verify_modbus.py --port 1532 --unit-id 5 --expected 650
```

通过 SOC 读取、非法地址异常 02、FC06/FC10 写入拒绝异常 02、FC04 不支持异常 01、重连后读回 650。关键实际报文（十六进制）：

```text
TX 00 01 00 00 00 06 05 03 00 64 00 01
RX 00 01 00 00 00 05 05 03 02 02 8A
TX 00 03 00 00 00 06 05 06 00 64 00 7B
RX 00 03 00 00 00 03 05 86 02
TX 00 06 00 00 00 06 05 03 00 64 00 01
RX 00 06 00 00 00 05 05 03 02 02 8A
```

桌面验证覆盖添加、重复拒绝、切换、通信与停机；本轮桌面自动化未完成 canvas 单元格编辑，多单元不同值的读回由 TCP 集成测试验证，草稿与目标 ID 由前端测试验证。不宣称已完成“桌面编辑多个不同值→独立客户端分别读回”的全链路验收。

## 未验证范围

未完成真实 EMS、Linux/Windows、规模/性能、跨端点多设备、多客户端、删除单元、磁盘持久化及通用点表配置。本轮桌面使用静态构建；动态模式做 Rust 回归，未重新做动态桌面/打包验收。临时 Python 环境和验证 app 路径不是正式用户启动入口。
