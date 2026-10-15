# ESSim 正式工程

当前使用 pnpm 11.1.2、Turborepo、TypeScript、React、Vite、Tailwind CSS 4；`packages/ui` 通过官方 shadcn CLI 安装共享组件，包含 Button、Input、Label、Separator、Tooltip、Dialog、DropdownMenu；主题支持明暗变量和动画样式。工作台使用 Dockview。NovaSheet 已复用原型 vendor 快照用于单 SOC 教学点表；`@pierre/trees` 尚未迁移。

## 依赖方向

- 前端页面依赖 `EssimClient`，应用组装层选择 Tauri 适配器。
- `workbench/src-tauri` 负责 Tauri 命令与 DTO 转换，依赖核心与基础设施。
- `essim-core` 定义领域和应用用例，不依赖 Tauri、SQLite 或 C 协议库。
- `essim-infrastructure` 依赖核心，实现 libmodbus 协议适配及有界 worker；数据库未接入。

已实现核心信息、快照、启动、停止、SOC 更新与订阅/取消命令。单实例 ID 为 `demo-bms`，点 ID 为 `soc`。Tauri 持有应用服务，耗时命令在 blocking task 等待协议 worker 确认；订阅线程通过 channel 推送完整快照。序号/代次用字符串传输，前端过滤旧事件。浏览器无后端明确报错。

## 后续替换传输

增加 Rust 服务入口和 `remote.ts`，在应用组装层选择 HTTP/WebSocket 适配器。业务组件不直接导入 Tauri。远程连接地址、重连和断线提示属于必要新增界面；不承诺纯粹零界面修改。配置、运行值、协议线程的所有权由 Rust 核心掌握。

## 当前范围

已接入真实 Modbus TCP 单寄存器教学映射，未实现 SQLite、104/61850、串口、通用点表或策略。macOS 完成独立客户端与 Tauri 闭环；其他平台和大表行为未验证。资源所有权、内部 socketpair、有界接收与依赖版本见 [接入说明](../test-records/libmodbus-dependency.md)。

## 添加 shadcn/ui 组件

根目录执行 `pnpm ui:add <组件名>`，例如 `pnpm ui:add select`。CLI 固定在根开发依赖及锁文件中；工作台与 UI 包的 `components.json` 统一使用 new-york、slate、lucide，Tailwind 4 无单独配置文件。组件安装到 `packages/ui/src/components`，工作台使用 `@essim/ui/components/<名称>` 导入。共享样式位于 `packages/ui/src/styles.css`，工作台负责 Tailwind 入口与共享源码扫描；全局 TooltipProvider 延迟 400ms。Vite 与 TypeScript 的 `@/` 别名均指向工作台源码。

官方新组件使用 `cn` 与 `radix-ui` 包；保留 `@essim/ui/lib/utils` 导出供项目代码使用。新增依赖应属于实际使用它的 workspace；不要仅安装到根目录或其他应用。安装后运行 `pnpm typecheck`、`pnpm build`。

## NovaSheet 接入边界

正式包依赖原型 `vendor/` 已有 core 0.17.3、canvas2d 0.6.6、react 0.4.14，pnpm overrides 保证包内 core/render backend 使用同一快照。没有修改 NovaSheet 源仓库或原型代码。

本快照 React 适配器没有转发 `isCellEditable`，因此用 `cellTypes` 注册只读类型保护模型字段与已生效值，只有草稿列可编辑。生产包的 vendor JSX 开发接口通过 Vite 的 importer 定向兼容模块转换；该转换只在 NovaSheet 引用中生效。页面加载错误通过边界展示，避免空白窗口。当前只验证一个点的桌面显示、编辑与应用；填充柄、自动滚动、Excel 多格粘贴、中文输入和大表性能需后续完整验收。

## libmodbus 三包边界（2026-10-15）

`crates/libmodbus/{libmodbus-src,libmodbus-sys,libmodbus-rs}` 归同一个根 Cargo workspace；src 是构建辅助包，sys 是唯一原生链接所有者，rs 是安全 API。基础设施只依赖 rs，C 编译和原始指针已迁出；基础设施 build.rs 仅保留 headless/测试的动态开发 rpath，不编译 C。

默认 vendored 静态；动态模式显式使用自行编译前缀 `LIBMODBUS_DIR`，没有宿主库探测。Tauri/基础设施入口均转发 feature。SOC 策略、帧门禁、内部传输、worker 与日志仍在 ESSim。动态应用通过打包脚本携带 dylib 并改用包内相对路径。详见 [三包构建说明](../../crates/libmodbus/README.md)。

## 同一端点多单元

统一客户端支持 addUnit(unitId) 与 updateSoc(raw, unitId)，Tauri 适配器转发到应用服务，完整快照包含 units 列表。core 的 UnitId 校验本阶段范围 1–247；infrastructure 的 worker 为每个 ID 创建独立 Native/context/mapping，收到请求后按 MBAP Unit ID 选择映射，未知单元关闭连接。新增成功确认后才发布新单元；旧 update 与快照 soc 保持默认单元 1 兼容。

前端 UnitSocSheets 按 Unit ID 保存独立数据源及生效值，切换不共用草稿；订阅中的日志更新不覆盖未提交编辑。单元添加、切换仍位于紧凑通信配置栏，保留 Dockview 工作台。没有增加删除、磁盘保存、多端点或多客户端。

## RTU 通信适配

core 用 Communication::Tcp(Endpoint) / Rtu(RtuEndpoint) 区分通信配置，RTU 配置和 SOC 独立保留；旧 TCP endpoint、units 和默认单点接口保留兼容。configure 只在停止后整组校验并发布；start_rtu 返回成功时真实串口已打开。Tauri DTO 使用带 transport 标记的配置和完整快照，RTU 活跃单元列表仅包含配置站号；前端经统一 EssimClient 调用，不直接引用 Tauri API。

infrastructure 按通信方式启动对应 worker；TCP 仍使用外部有界传输和内部 socketpair，RTU worker 独占 RtuContext 与 Mapping，直接调用库串口收发。RTU 只读 SOC 在进入会修改映射的正常 reply 前拒绝写入；广播不应答也不修改 SOC。错误状态仍可停止、释放并重配。串口 TX 只记录返回字节数与结果，未将构造字节伪装成抓包。

libmodbus-rs 中的 RtuConfig 是不可变的已校验配置，RtuRequest 独立于 TCP Request；RtuContext 无 Send/Sync，C 串口资源在 worker 中 close 后 free。首版限制配置范围和接收超时，不开启自定义 RTS、自动重连或链路重试。发送采用上游 Unix O_NONBLOCK 路径；真实驱动的停止行为仍需硬件验证。

工作台的 UnitSocSheets 以 transport + Unit ID 管理草稿，模式切换不串值；运行中锁定通信方式及参数，RTU 隐藏 TCP 多 Unit ID 与连接数控件。首版手填串口路径，不增加自动枚举或系统配置操作。

### 通用 Modbus 主站库补充（2026-10-15）

`libmodbus-rs::ModbusClient` 独立提供同步 TCP/RTU 主站：TCP 使用 Rust 主动连接并拥有 socket，C 仅借用；RTU context 独占串口，按 close → free 释放。主站复用官方八个读写 API，发送前校验地址和数量，不依赖 Mapping、ESSim 核心或 Tauri。目标切换、错误与超时由库暴露，无自动重试/重连；轮询、设备语义和未来工作台入口属于上层。

合法 Modbus 异常保留通道；I/O、超时和上游检测出的无效响应关闭通道，后续须重建。上游 TCP 响应 Unit ID/MBAP 长度校验及非墙钟截止时间的等待边界见 [主站验收记录](../test-records/2026-10-15-modbus-master.md)。当前独立 PyModbus TCP/PTY 互通已验，工作台主站 UI、真实设备及 RS-485 硬件未验。
