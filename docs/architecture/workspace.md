# ESSim 正式工程

正式客户端使用TypeScript／React／Vite、pnpm workspace、Turborepo、shadcn/ui与Tailwind CSS；桌面壳为Tauri。工作台采用浅色紧凑Dockview、`@pierre/trees`资源与模拟器树、NovaSheet点表。

## 依赖与职责

- `crates/core/domain/foundation.rs`：稳定身份、采集模型版本、端点聚合、可选设备关联及纯值迁移。
- `crates/core/application/workspace.rs`：配置命令、权威目录校验、只读投影和最小仓储端口。保存不自动应用，应用不启动。
- `crates/core/application/runtime.rs`：运行状态机、执行器／可靠录制／历史读取／时钟窄端口及隔离回放契约。
- `crates/infrastructure/workspace.rs`：会话内内存仓储与Mutex保护的配置服务。没有生产ProtocolExecutor或可靠Recorder。
- `workbench/src-tauri/commands/`：DTO、命令／查询、订阅生命周期；`lib.rs`组装同一个WorkspaceEngine并注入State。
- 前端业务组件依赖统一客户端契约；Tauri导入只在应用适配层，后续可新增HTTP／WebSocket适配器。

core依赖serde及num-bigint／num-rational／num-traits精确数值库，不依赖Tauri、数据库、libmodbus或窗口。内存目录提交是整体替换，模型引用与版本检查在同一短临界区完成，不持锁等待协议I/O。

## 配置与状态

模型保存产生不可变版本；单元明确引用ModelId/version，旧版本不被新保存覆盖。角色在端点统一确定，四遥和Modbus数据区独立。线上站号不作为UnitId，同端点站号唯一；首版产品站号范围1–247不是TCP标准上限。RTU从站配置最多一个单元，RTU主站可表示多个目标。

前端EndpointConfig草稿与已保存、已应用副本分离。保存／应用携带expectedRevision，冲突不覆盖。应用预先迁移全部单元，失败不半应用；原始值按稳定点ID和同类型保留，新点用初值，删除移除；倍率变化不重算原始值。M02已增加精确编码、显式字节／字序及值兼容迁移规划。

版本／事件序号以十进制字符串跨IPC。配置事件投影幂等处理重复序号、拒绝缺口和未知schema；界面订阅发送可替换的完整快照，可以合并配置通知，不承担历史持久化。

## 运行与录制

一次显式启动对应RunId，覆盖端点全部单元；重连／暂停单元不创建新运行。状态与录制状态分别表示，停止请求返回后保持Stopping，资源释放确认后才能Stopped。录制创建成功前不打开通道；外部效果遵循可靠意图→执行一次→可靠结果，结果记录失败保留未确认范围并停止，不重发。

RunCoordinator通过测试适配器验证上述规则，生产工作台尚未装配执行器／可靠Recorder，启动、远端读写和回放命令返回CAPABILITY_UNAVAILABLE。回放类型不接收执行器，只重建独立值，固定模型版本，拒绝历史缺口／未知版本。不能将此契约测试宣称为可靠落盘或可用播放引擎。

## 界面组件边界

设备／资源与模拟器导航统一使用`@pierre/trees@1.0.0-beta.6`的React入口。树模型在组件生命周期内复用；目录变化用resetPaths更新。显式路径→业务ID映射使设备快捷入口与协议树定位同一端点Tab；不将路径或显示名称当作业务身份。模型重命名／复制／删除已接入；设备与端点的完整CRUD仍由后续阶段交付。

正式工作台安装NovaSheet发布包：core 0.17.3、canvas2d 0.6.6、react 0.4.14；pnpm overrides固定同一份core与后端，不再引用`prototype/vendor/`快照。原型自身保留原有依赖。React适配器未转发isCellEditable，使用cellTypes只读类型保护模型及确认列；只允许本地值草稿编辑，草稿不执行通信。已访问单元／分类保留挂载的Grid以保留选区与滚动，数据源按稳定身份／分类／模型版本隔离。M01未提供点表结构编辑器，不将空表展示算作填充柄、Excel粘贴、中文输入或大表性能验收。

现有NovaSheet JSX开发接口的生产构建兼容模块仍仅对该包生效。全局错误边界保留。共享shadcn组件通过`@essim/ui/components/...`使用；`pnpm ui:add <组件名>`维护共享组件，不复制其他项目UI。

## 订阅与回收

Tauri配置订阅上限8个，每个订阅仅在序号改变时发送最新完整目录，100ms检查取消。发送失败结束线程；取消幂等，在blocking任务等待线程回收；Subscriptions销毁时停止并回收剩余线程。当前是低频配置订阅，不是未来高频点值推送设计，M07还需批量订阅／背压及性能验证。

## 通用协议库与教学退役

`crates/libmodbus/`三包、官方源码／许可、通用示例和测试保留。src负责源码构建，sys是原生链接所有者，rs提供RAII安全API及TCP／RTU主从子集；句柄不Send／Sync，Rust拥有TCP socket、C借用；RTU资源按close→free释放。

M01删除固定SOC worker、旧Native、旧IPC和教学示例，不为维持旧链接断言保留无用协议依赖。当前桌面应用不链接libmodbus，静态／显式动态能力由保留库与独立client_demo验证。打包工具按实际链接依赖处理。真实多实例执行器、调度及可靠记录均是后续提案，不将历史SOC互操作当作新产品验收。

[实施与迁移记录](../test-records/2026-10-15-m01-foundation.md) · [通用库说明](../../crates/libmodbus/README.md)。

## v15 样式对齐（2026-10-15）

工作台已按确认稿调整冷白配色、蓝线Tab、纯图标工具栏和右侧Details双列属性；导航及属性使用Dockview公开的header.hidden接口消除重复标题，底部事件／报文Tab保持可停靠布局，按后续确认移除运行历史Tab和独立历史面板。树节点以通信地址显示，但仍映射稳定业务ID。新增观测区仅为空态，不是历史录制交付，详见[样式验证](../test-records/2026-10-15-m01-v15-style.md)。

## M02 点表管理

`domain/point_table.rs`负责有理数十进制换算、精确RawValue、寄存器编码和整表占用校验。`infrastructure/point_exchange.rs`负责文本行适配和受限JSON／CSV／XLSX解析，不修改目录、不执行公式。Tauri文件命令在后台任务中处理原生文件选择、取消及导出；前端业务组件只调用统一客户端。

ModelDraft按ModelId持有单一文本草稿，PointEditor及Details共享状态。DraftSource只管理可见行和空白扩容，按稳定点ID合并一次粘贴／填充，草稿统一负责撤销。数据源订阅遍历快照，避免监听器同步重订阅造成循环。结构编辑使用同一NovaSheet公开Grid及canvas2d入口，提供isCellEditable和原生singleSelect编辑器，不改vendor。WKWebView系统粘贴使用隐藏textarea承接可信剪贴板事件，再调用组件公开TSV工具；普通单元格编辑仍由组件处理。

模型保存生成新版本，expectedRevision防止过期覆盖。删除权威检查配置、已应用及保留记录引用；后者仅有窄端口与测试。应用预览为纯规划，提交先停止完整端点，再重新核对权威配置并整体替换已应用副本。停机后失败保留原应用与值，报告已停止；当前只有StoppedBoundary，未接入真实I/O。主站映射变化使相应缓存失效，不把从站初值或本地值伪装成远端读取结果。

配置序号未变化时订阅不克隆完整目录。配置全量快照仍不适用于未来高频值流。macOS自定义Quit菜单与窗口关闭共同保护会话；退出权限仅在处理器就绪后启用，用户明确确认后才允许退出。会话数据尚未落盘。

详见[使用及交换说明](../point-table-format.md)与[M02验证记录](../test-records/2026-10-16-m02-progress.md)。旧M01章节的验收范围保持历史边界，不能据此宣称M02全部完成。

2026-10-16选中态修复：选区是Canvas上方的DOM浮层，`selectionBg`必须半透明；此前不透明浅蓝色遮住表头及范围内文字，现改为`rgba(8, 117, 255, 0.09)`。已核对本地NovaSheet源码及其注册源最新版本并安装发布包，没有复制源码或新增tgz文件。
