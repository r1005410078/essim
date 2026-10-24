# 树与停靠库手工回归夹具

使用本工作树的 Vite 开发服务器访问：

- `/tests/manual/docking-regression.html`：实际 Dockview 默认渲染；将模型 A 输入改为“保留的草稿”，依次分屏、合并、切换 B、切回 A。再次分屏后输入应保留，顶部 mounts 的 A/B 均为 1，unmounts 为 0。
- `/tests/manual/tree-regression.html`：实际 NavigationTree／TreeStatusLayer，200 个模型，明确标注的测试状态。滚动、点击权威路径刷新、折叠目录，再拖动模型 002 到目录，检查层级与状态。每行最多一个 `.navigation-status`；路径刷新与虚拟行复用后不得残留旧节点状态。

这些页面不包含 Rust 后端。树移动使用前端夹具模拟权威目录更新，不能作为 Rust 原子事务、真实运行或 macOS 原生手势验收。它们不属于产品入口，也不参与单入口生产构建。真实事务及运行证据见 docs/test-records/2026-10-24-resource-tree-docking.md。


`native-pointer.swift`是本次用户明确授权后使用的macOS辅助输入工具，仅匹配独立测试壳主窗口；不是产品代码，不由构建或测试自动执行。它使用现有辅助功能权限发送真实鼠标事件，参数为Retina截图的四个物理像素坐标，拒绝窗口外坐标。使用前须明确获得本次原生事件操作授权；不能用本文件替代用户授权。实际步骤、坐标和截图见上述验收记录。
