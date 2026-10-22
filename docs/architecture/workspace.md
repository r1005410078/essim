# ESSim 正式工程

正式客户端使用TypeScript／React／Vite、pnpm workspace、Turborepo、shadcn/ui与Tailwind CSS；桌面壳为Tauri。工作台采用浅色紧凑Dockview、`@pierre/trees`资源与模拟器树、NovaSheet点表。

当前调用顺序见[图与代码对应：命令、服务、查询及前端通知](../design/modbus-event-storming.md#0-当前代码新建模拟器的命令查询与通知)。该图明确区分服务同步调用投影监听器、前端快照订阅与尚未接入的业务事件监听。

## 文件分层（2026-10-18）

```text
crates/core/src/
├── application/
│   ├── service.rs                       # 应用服务：事务与用例编排
│   ├── commands/                        # 配置处理器及slave.rs运行命令
│   ├── events/
│   │   ├── configuration.rs             # 成功配置业务事实
│   │   ├── config_event.rs              # 配置事件信封
│   │   ├── history.rs                   # 运行历史事件契约
│   │   └── slave.rs                     # 实际TCP从站运行事实
│   ├── event_handlers/
│   │   └── workspace_projection.rs      # 同步查询投影监听器
│   ├── queries/
│   │   ├── workspace_view.rs            # 查询结果
│   │   ├── capabilities.rs              # 实际能力
│   │   ├── projection.rs                # 投影状态与快照构造
│   │   ├── apply_preview.rs             # 只读应用预览
│   │   ├── inspection.rs                # 保存／应用差异查询
│   │   └── slave.rs                     # 独立运行摘要和点值查询
│   ├── ports.rs                         # 外部能力接口
│   └── catalog.rs                       # 跨聚合目录及引用检查
└── domain/
    ├── aggregates/
    │   ├── endpoint/
    │   │   ├── aggregate.rs             # EndpointConfig聚合根
    │   │   ├── unit.rs                  # 所属Unit实体
    │   │   └── transport.rs             # 角色、传输参数值对象
    │   ├── model/
    │   │   ├── aggregate.rs             # CollectionModel聚合根
    │   │   ├── point.rs                 # 所属点定义
    │   │   └── reference.rs             # 模型身份引用
    │   ├── device.rs                    # 可选设备实体
    │   └── navigation.rs                # 独立导航及其规则
    ├── identity.rs                      # 稳定身份
    ├── values.rs                        # 运行值与迁移
    ├── point_table.rs                   # 点表编码、地址校验
    └── slave.rs                         # 共享运行模型索引与纯置数规划
```

各目录的`mod.rs`负责声明及重导出。原`domain::endpoint`、`domain::model`等导入路径继续有效，具体实现统一在`aggregates/`，没有两套模型。事件监听器是现有同步投影消费者，不是通用事件总线或动态订阅框架。

## 依赖与职责

- [`domain/`](../../crates/core/src/domain/mod.rs)：身份、模型、端点、导航及值迁移按领域拆分；`foundation.rs`仅为旧导入门面。
- [`application/commands/`](../../crates/core/src/application/commands/mod.rs)：Command／Action及按模型、端点、导航、设备、生命周期组织的处理器。
- [`application/queries/mod.rs`](../../crates/core/src/application/queries/mod.rs)：WorkspaceView、Projection、纯应用预览及inspection，不依赖仓储提交或资源操作。
- [`application/events/mod.rs`](../../crates/core/src/application/events/mod.rs)：具体ConfigurationFact与ConfigEvent提交信封。
- [`application/event_handlers/`](../../crates/core/src/application/event_handlers/mod.rs)：提交后的同步投影监听器；事件定义、查询状态和事件处理逻辑分别存放。
- [`application/ports.rs`](../../crates/core/src/application/ports.rs)：ConfigRepository及ApplyBoundary；[`service.rs`](../../crates/core/src/application/service.rs)负责原子提交、分段释放及投影更新；[`catalog.rs`](../../crates/core/src/application/catalog.rs)负责跨聚合会话目录和引用检查。
- [`application/runtime.rs`](../../crates/core/src/application/runtime.rs)：运行状态机、执行器／可靠录制／历史读取／时钟窄端口及隔离回放契约，仍未装配为生产执行器。
- [`infrastructure/workspace.rs`](../../crates/infrastructure/src/workspace.rs)：内存仓储与线程安全应用入口；`execute_with_event`可返回已提交事件，原execute只返回Receipt。
- `workbench/src-tauri/src/commands/`：DTO、命令／查询与订阅生命周期，`lib.rs`组装同一个WorkspaceEngine并注入State。
- 前端业务组件依赖统一客户端契约；Tauri导入只在应用适配层，后续可新增HTTP／WebSocket适配器。

core依赖serde及num-bigint／num-rational／num-traits精确数值库，不依赖Tauri、数据库、libmodbus或窗口。内存目录提交是整体替换，模型引用与整组校验在同一短临界区完成，不持锁等待协议I/O。

## 配置与状态

模型按ModelId保存单份当前配置，保存覆盖同一对象；单元只引用ModelId。已应用模型保存在UnitValues的完整快照中，编辑／保存模型不覆盖该快照。角色在端点统一确定，四遥和Modbus数据区独立。线上站号不作为UnitId，同端点站号唯一；首版产品站号范围1–247不是TCP标准上限。RTU从站配置最多一个单元，RTU主站可表示多个目标。

前端EndpointConfig草稿与已保存、已应用副本分离。保存／应用不携带配置版本；命令在权威目录内串行校验并原子提交。应用状态通过当前内容与已应用快照比较。应用预先迁移全部单元，失败不半应用；原始值按稳定点ID和同类型保留，新点用初值，删除移除；倍率变化不重算原始值。M02已增加精确编码、显式字节／字序及值兼容迁移规划。

事件通知序号以十进制字符串跨IPC，仅用于通知排序。配置事件投影幂等处理重复序号、拒绝缺口和未知schema；界面订阅发送可替换的完整快照，可以合并配置通知，不承担历史持久化。

## 运行与录制

一次显式启动对应RunId，覆盖端点全部单元；重连／暂停单元不创建新运行。状态与录制状态分别表示，停止请求返回后保持Stopping，资源释放确认后才能Stopped。录制创建成功前不打开通道；外部效果遵循可靠意图→执行一次→可靠结果，结果记录失败保留未确认范围并停止，不重发。

RunCoordinator通过测试适配器验证通用契约；2026-10-18正式TCP从站使用独立SlaveCommand、TcpSlaveRuntime和Journal实现运行与文件记录，没有将该泛型协调器宣称为已装配。旧配置Action::Start保留能力门禁，工作台通过独立execute_slave进入真实TCP路径；主站远端读写、RTU及回放仍不可用。回放类型不接收执行器，只重建独立值，固定模型快照，拒绝历史缺口／未知版本。不能将此契约测试宣称为可靠落盘或可用播放引擎。

## 界面组件边界

设备／资源与模拟器导航统一使用`@pierre/trees@1.0.0-beta.6`的React入口。树模型在组件生命周期内复用；目录变化用resetPaths更新。显式路径→业务ID映射使设备快捷入口与协议树定位同一端点Tab；不将路径或显示名称当作业务身份。模型重命名／复制／删除及M03端点配置CRUD已接入；设备更多管理仍由后续阶段交付，M03剩余原生验收独立保留。

正式工作台安装NovaSheet发布包：core 0.17.3、canvas2d 0.6.6、react 0.4.14；pnpm overrides固定同一份core与后端，不再引用`prototype/vendor/`快照。原型自身保留原有依赖。React适配器未转发isCellEditable，使用cellTypes只读类型保护模型及确认列；只允许本地值草稿编辑，草稿不执行通信。已访问单元／分类保留挂载的Grid以保留选区与滚动，数据源按稳定身份／分类／模型快照隔离。M01未提供点表结构编辑器，不将空表展示算作填充柄、Excel粘贴、中文输入或大表性能验收。

现有NovaSheet JSX开发接口的生产构建兼容模块仍仅对该包生效。全局错误边界保留。共享shadcn组件通过`@essim/ui/components/...`使用；`pnpm ui:add <组件名>`维护共享组件，不复制其他项目UI。

## 订阅与回收

Tauri配置订阅上限8个，每个订阅仅在序号改变时发送最新完整目录，100ms检查取消。发送失败结束线程；取消幂等，在blocking任务等待线程回收；Subscriptions销毁时停止并回收剩余线程。当前是低频配置订阅，不是未来高频点值推送设计，M07还需批量订阅／背压及性能验证。

## 通用协议库与教学退役

`crates/libmodbus/`三包、官方源码／许可、通用示例和测试保留。src负责源码构建，sys是原生链接所有者，rs提供RAII安全API及TCP／RTU主从子集；句柄不Send／Sync，Rust拥有TCP socket、C借用；RTU资源按close→free释放。

M01删除固定SOC worker、旧Native、旧IPC和教学示例，不为维持旧链接断言保留无用协议依赖。M01阶段桌面应用曾不链接libmodbus；2026-10-18 TCP从站重新通过infrastructure的tcp-runtime feature接入，桌面默认vendored静态构建；关闭桌面默认feature并提供LIBMODBUS_DIR保留显式动态构建路径。打包工具按实际链接依赖处理。TCP从站的固定线程执行器及文件记录见下节和独立实施记录，不将历史SOC互操作当作新产品验收。

[实施与迁移记录](../test-records/2026-10-15-m01-foundation.md) · [通用库说明](../../crates/libmodbus/README.md)。

## v15 样式对齐（2026-10-15）

工作台已按确认稿调整冷白配色、蓝线Tab、纯图标工具栏和右侧Details双列属性；导航及属性使用Dockview公开的header.hidden接口消除重复标题，底部事件／报文Tab保持可停靠布局，按后续确认移除运行历史Tab和独立历史面板。树节点以通信地址显示，但仍映射稳定业务ID。新增观测区仅为空态，不是历史录制交付，详见[样式验证](../test-records/2026-10-15-m01-v15-style.md)。

## M02 点表管理

`domain/point_table.rs`负责有理数十进制换算、精确RawValue、寄存器编码和整表占用校验。`infrastructure/point_exchange.rs`负责文本行适配和受限JSON／CSV／XLSX解析，不修改目录、不执行公式。Tauri文件命令在后台任务中处理原生文件选择、取消及导出；前端业务组件只调用统一客户端。

ModelDraft按ModelId持有单一文本草稿，PointEditor及Details共享状态。DraftSource只管理可见行和空白扩容，按稳定点ID合并一次粘贴／填充，草稿统一负责撤销。数据源订阅遍历快照，避免监听器同步重订阅造成循环。结构编辑使用同一NovaSheet公开Grid及canvas2d入口，提供isCellEditable和shadcn Select编辑器，不改vendor。WKWebView系统粘贴使用隐藏textarea承接可信剪贴板事件，再调用组件公开TSV工具；普通单元格编辑仍由组件处理。

模型保存覆盖当前内容，不保留版本列表或期望版本字段。删除权威检查配置与已应用引用；未来历史记录使用自包含快照，不阻止当前模型删除。应用预览为纯规划，提交先停止完整端点，再重新核对权威配置并整体替换已应用副本。停机后失败保留原应用与值，报告已停止；当前已通过真实TCP／RTU运行适配器等待资源释放；StoppedBoundary仅用于无I/O测试。主站映射变化使相应缓存失效，不把从站初值或本地值伪装成远端读取结果。

配置序号未变化时订阅不克隆完整目录。配置全量快照仍不适用于未来高频值流。macOS自定义Quit菜单与窗口关闭共同保护会话；退出权限仅在处理器就绪后启用，用户明确确认后才允许退出。会话数据尚未落盘。

详见[使用及交换说明](../point-table-format.md)与[M02验证记录](../test-records/2026-10-16-m02-progress.md)。旧M01章节的验收范围保持历史边界，不能据此宣称M02全部完成。

2026-10-16选中态修复：选区是Canvas上方的DOM浮层，`selectionBg`必须半透明；此前不透明浅蓝色遮住表头及范围内文字，现改为`rgba(8, 117, 255, 0.09)`。已核对本地NovaSheet源码及其注册源最新版本并安装发布包，没有复制源码或新增tgz文件。

## 配置简化（2026-10-16）

2026-10-16 用户要求移除配置版本管理：每个模型只保存一份当前配置，单元只引用稳定模型ID；保存覆盖当前内容，不保留历史版本，不提供选版、升级或expectedRevision。保存与应用仍分开，已应用模型按完整快照保存，待下次显式应用才改变；运行记录以后使用自包含配置快照，不依赖模型历史版本。通知序号与文件格式标记仅用于内部传输／解析，不是用户配置版本功能。此前版本管理约定由本条取代，历史验收记录保持原证据边界。


## M03配置与资源释放边界（2026-10-17）

创建／保存端点拆成独立命令：创建原子写入首个Unit，保存不允许目标复活或改变角色／传输种类。复制只复制当前保存定义，备注为展示字段。导航Folder与成员关系独立于EndpointConfig，改名／移动不改变协议身份。树主名称现使用自定义名称，通信参数辅助展示，取代上述v15阶段的地址主显示。

核心`inspect_endpoint`是唯一协议差异比较规则，既用于查询的`applicationMatches`，也通过只读IPC比较未保存草稿。前端不再比较完整JSON来判断应用关系；异步结果受草稿对象与通知sequence约束。点表按Unit／分类及稳定点集合复用数据源，更新展示列而不覆盖用户值草稿。保存仍覆盖当前内容，不引入配置版本。

基础设施先在短锁内创建ReleasePlan，放锁等待ApplyBoundary，再重新锁定并校验目标、依赖、已应用快照和值，原子提交且保留无关并发修改。真实停止状态独立于提交结果。ConfirmedBoundary将ReleaseDriver的请求和有界释放确认适配为整组边界；生产只装配StoppedBoundary，没有真实资源，不据此提供启动成功。Tauri配置执行在后台任务中运行。M04／M05接入实际所有者前，须先明确M06并发及资源约束。

禁用Unit仍保留值，移除Unit在成功应用后清理；主站经应用停机后缓存标记过期，从站保持兼容原始值。复位只针对已应用的停止Unit；运行态必须经未来执行器值所有者处理。导航和全部配置仅本次会话有效。自动化及原生已验范围、两个待验任务与完整workspace的autoconf限制见[M03记录](../test-records/2026-10-17-m03-configuration.md)。


## 配置CQRS与领域职责入口（2026-10-17）

配置写入路径为：Tauri适配器 → WorkspaceEngine → service::Workspace → 对应用例处理器 → 领域校验／跨聚合引用检查 → 仓储原子提交 → 返回具体ConfigEvent并更新Projection。查询读取WorkspaceView或调用无副作用预览，不通过写处理器。生命周期处理器发现权威内容变化时交由服务刷新投影，不直接操纵查询模型。

领域层维护端点类型不变、站号、模型及导航局部规则；应用层协调模型／设备引用、资源释放和存储事务。目录分层不等于所有类型已经严格封装为富领域聚合；当前采用轻量DDD，不建设通用框架。

ConfigurationFact包含具体的创建、保存、复制、删除、应用、移动和复位事实。ConfigEvent的fact表达成功业务动作，catalog提供查询恢复快照；前端订阅仍发送WorkspaceView。核心execute及基础设施execute_with_event明确返回成功提交后的事件；生产Tauri路径使用execute，只保留Receipt，没有继续发布ConfigurationFact。服务同步调用WorkspaceProjectionListener::handle并复制catalog快照；Projection::accept保留为兼容委托入口，没有按fact注册或分派业务监听器。前端还会在命令成功后主动refresh，并通过独立快照Channel接收后台更新。没有事件总线或可靠投递。此机制不是持久化事件溯源；运行HistoryEvent及可靠录制是另一条后续链路。

[重构提案](../../openspec/changes/refactor-workspace-cqrs-boundaries/proposal.md) · [设计与目标目录](../../openspec/changes/refactor-workspace-cqrs-boundaries/design.md) · [实施与71项测试记录](../test-records/2026-10-17-cqrs-boundaries.md)。M02／M03未完成验收不因此关闭。

## M04 TCP从站实际接入（2026-10-18）

```text
core/src/application/{commands,events,queries}/slave.rs  # 运行意图、实际事实、只读结果
core/src/domain/slave.rs                              # 共享模型索引及纯校验
infrastructure/src/tcp_slave/
  mod.rs            # 有界命令入口、单端点操作门、线程回收
  worker.rs         # 所属端点／Unit／连接的唯一所有者，mio轮询
  mapping.rs        # C四区映射、完整地址及权限校验
  journal.rs        # 类型化事实的可靠同步文件消费者
libmodbus-rs/src/responder.rs                         # 安全完整帧、私有响应socket
workbench/src-tauri/src/commands/slave.rs              # 窗口无阻塞运行IPC
workbench/src/features/workspace/SlaveRuntimeControls.tsx # 可见行查询、当前Unit待修改提交、地址范围恢复
workbench/src/features/workspace/RuntimeGrid.tsx          # NovaSheet公开Grid／可见行适配
workbench/src/features/workspace/RuntimeDataSource.ts     # 草稿基线及待修改身份
```

实际线程数为最多4个运行工作线程；每端点默认10连接，运行启动参数可改为1～100，全局连接上限10000，控制队列每工作线程256条，单次运行命令最大8MiB。单连接收发缓冲各16KiB，半帧／发送停滞5秒超时，接口等待确认10秒；接口超时不能当作写入未发生。模型按内容在工作线程内共享，Unit映射独立；配置Catalog仍会深复制模型和值，亿点规模尚未证明。

运行事实按自包含JSONL文件可靠确认，默认目录为Tauri应用数据目录下runs。高频值不写配置数据库；配置仍仅本次会话保存。请求事实先落盘，再执行C映射并记录准备响应，随后实际写出并记录发送事实；文件失败停止该端点且保留最后可靠位置。当前同步落盘会影响同工作线程尾延迟，实测如实记录，尚未提供历史浏览及可用回放。

工作台仅为可见端点面板每500ms查询当前Unit当前分类的真实可见行，单次最多1024点；通过NovaSheet公开后端工厂读取视口区域和组合数据源稳定ID，不读取选区。运行修订号独立于配置sequence。未修改的草稿跟随实际值；已修改草稿保留并计数，提交覆盖当前Unit各分类待修改点，成功只清除本次提交版本。恢复初值明确填写数据区及地址闭区间，整Unit复位单独确认。原生滚动、跨分类提交及独立PyModbus联合验证见[M04记录](../test-records/2026-10-18-m04-tcp-slave.md)。


## M05 TCP主站职责（2026-10-19）

- `core/domain/master.rs`：完整点请求规划、地址／数量上限、设置校验，无socket／Tauri。
- `core/application/commands/master.rs`：连接／采集／读取／写入意图；`events/master.rs`：状态、真实帧、失败与操作完成事实；`queries/master.rs`：按需最近值、质量、更新时间及逐点下发结果。
- `infrastructure/tcp_master/worker.rs`：端点独立所有者，调度同步libmodbus客户端；`bridge.rs`：私有socket pair透传及真实帧观测，不改vendored源、不生成模拟响应；`journal.rs`：连接前快照与sync_data可靠记录。
- `infrastructure/workspace.rs`：配置应用门、断开等待及快照迁移；前端经统一WorkspaceClient和Tauri适配调用，不直接访问运行线程。
- 首版32槽，每槽一个所有者、连接后另一个透传线程，命令队列容量1。查询读取投影不等待远端网络。该上限是资源门禁，非已验持续性能数字。


## M05 RTU主站（2026-10-19）

保留`infrastructure/tcp_master`历史目录名，现为TCP／RTU主站共享所有者与调度；`bridge::Connection`按Transport选择TCP或`rtu_bridge`，不复制采集／写入状态机。core的MasterAction增加Diagnose，查询增加实际诊断响应，MasterFact区分RtuFrame、短帧及隔离数据。

`libmodbus-rs::SerialPort`在所属透传线程打开真实串口，以设备身份／macOS tty-cu归一化做进程占用登记，并用TIOCEXCL阻止后续非特权打开；Drop先恢复串口参数、关闭原始fd，再释放dup独占和登记。TIOCEXCL不驱逐已打开的第三方句柄，未知驱动别名不作全局排他承诺。

私有RTU上下文只借用Rust Unix socket，基础功能由libmodbus处理；原始诊断发送由库补CRC，白名单长度／站号／功能／CRC／回送验证在安全封装内。串口参数变化经完整应用停止边界后清空缓存；临时驱动故障的自动重开不另建运行文件。


### RTU从站首批运行

沿用`tcp_slave`历史目录，`worker`是TCP与RTU从站共同的运行值所有者；`serial`持有独占SerialPort、接收缓冲和有界待发响应，C上下文不跨线程。共享UnitMapping的PDU权限／完整点校验，RtuResponder直接使用RTU上下文生成响应，C补CRC；不通过TCP ADU伪装RTU。串口非阻塞读写、短帧隔离、准备／驱动写出事实与独立查询均在同一所有者串行处理。UI仅通过统一运行接口访问。停止、应用、删除先释放串口，再复用已有运行值迁移。20ms软件短帧清理边界不是物理字符时序证明，方向控制与RS485电气仍待验。

## M06 首阶段的文件与调用对应（2026-10-19）

```text
core/src/application/
├── service.rs                         # 分组准备、一次目录事务与PartitionCreated
├── commands/partition.rs              # 分组输入、不可变计划、原子创建处理器
├── queries/partition.rs               # 分组数量／地址范围摘要
├── batch_service.rs                   # 批量任务命令协调，不执行协议IO
├── commands/batch.rs                  # 建立、领取、取消、未确认、真实完成
├── events/batch.rs                    # Started／ItemStarted／ItemUncertain等事实
├── event_handlers/batch_projection.rs # 同步事件监听与旧通知拒绝
└── queries/batch.rs                   # 有界计数、逐项结果与耗时
core/src/domain/
├── partition.rs                      # 完整业务点排序规则
├── aggregates/batch.rs               # 状态机、固定目标及未确认占位
└── resources.rs                      # 额度维度和溢出／上限校验
infrastructure/src/
├── partition.rs                      # 本机IP复查、会话预览、暂存预留
├── local_interfaces.rs               # getifaddrs RAII只读枚举
└── resources.rs                      # 原子预留账本及唯一释放令牌
```

分组调用：统一客户端→Tauri适配→`WorkspaceEngine::preview_partition`，先流式估算源模型字节并预留，再准备分组和校验本机地址；确认后`create_partition`→`Workspace::commit_partition`→单次仓储提交→`PartitionCreated`→已有`WorkspaceProjectionListener`。失败不发布成功事实，前端刷新读取已提交目录；配置仍为会话内当前内容，没有历史选版。

任务状态调用：`BatchService::execute(BatchCommand)`→`BatchTask`合法状态转换→`BatchEvent`具体事实→`BatchProjectionListener::handle`→有界任务查询。早期只做纯状态闭环；当前生产装配与查询边界见下节。

### M06 固定范围及线程组件（2026-10-20）

- `application/commands/batch_targets.rs`解析范围并建立不可变`BatchPlan`，内部按模型实际内容共享比较副本；`queries/batch_plan.rs`只发送固定目标摘要。
- `Workspace::prepare_batch`和`revalidate_batch`直接借用投影。目标移动／新增不改变清单；相关定义变化在每项执行前复核，不使用expectedRevision。
- `infrastructure/batch_runner.rs`是会话所有者持有的固定8线程组件，`submit`→原有BatchService／聚合／事件／监听投影→线程领取→BatchExecutor端口→实际返回后Complete。查询不等待执行端口，超时不释放槽，取消不打断已开始项。
- 已通过`batch_lifecycle.rs::BatchCoordinator`装配真实WorkspaceEngine、共享端点门、逐项应用／启停与安全重试。Tauri只适配预览／开始／取消／查询；Dockview页面销毁不销毁后台执行器。早期组件证据见[调度记录](../test-records/2026-10-20-m06-batch-runner.md)，当前交付见[报告](../test-records/2026-10-20-m06-delivery.md)。

端点操作门使用不可序列化的能力凭证，固定批量线程与手动生命周期入口共享同一会话的门。调用等待超时不释放实际所有者持有的子凭证；从站完成接收器可继续等待同一次提交，主站释放和配置迁移由原有所有者归还子凭证。查询不申请操作门，普通保存继续允许，配置应用提交仍校验目标变化。详见[操作完成验证](../test-records/2026-10-20-m06-operation-completion.md)。

### M06 实际分层与持久化等待（2026-10-20）

```text
core/application/
  commands/batch_targets.rs        固定范围、内容复核与按项观察阈值
  commands/batch.rs                Start／Claim／Cancel／MarkUncertain／Complete
  batch_service.rs                聚合协调与具体事件派发
  events/batch.rs                  具体任务事实
  event_handlers/batch_projection.rs 事件监听与有界投影
  queries/batch.rs                 计数／条目／排队及执行耗时
core/domain/
  aggregates/batch.rs              状态、占位、取消和数量守恒
  resources.rs                    多维额度校验
  values.rs                       Arc已应用定义与独立Unit原始值
infrastructure/
  batch_lifecycle.rs               会话预览、真实执行器、安全重试
  batch_runner.rs                  固定8线程、单活动任务、真实完成回收
  operation_gates.rs               可验证能力凭证与回调占位
  resources.rs                    唯一RAII令牌、配置暂存与运行准入
  recording.rs                    固定4线程、每文件FIFO、字节上限、完成句柄
  tcp_slave/worker.rs              固定协议线程内本地协作任务、最近可靠查询缓存
  tcp_slave/journal.rs             自包含快照与逻辑可靠序号
  tcp_master/mod.rs                每端点录制串行门，查询不持有磁盘等待锁
workbench/src-tauri/commands/
  batch.rs                        批量／资源IPC适配，不提供伪造Complete入口
workbench/src/features/workspace/
  BatchPanel.tsx                  范围、预览、结果、取消与重试
  ResourcePanel.tsx               估计／预留／上限／RSS／录制积压
```

应用在同一目录锁内复核固定计划并准备释放；等待不占目录锁，提交时仍复核目标变化。普通保存可继续。配置仓储在复制暂存前预留，提交新目录时旧新占用并存计费。运行启动先预留，真实失败／停止后归还相应活动额度；停止保留值映射内存。任务比较暂存在真实全部结束或全未开始取消后归还，未确认项不提前归还。

从站的每端点本地Future留在所属固定协议线程，不具有Send迁移能力；C句柄不会因await换线程。等待录制时同一线程继续其他端点；查询读取独立的最近可靠缓存，不访问借用中的C映射。对应文件可靠确认后才更新已确认值和序号，ResponsePrepared确认前不发响应。停机路径可以释放资源，不依赖往已满数据录制队列追加成功事实。

主站所有者与wire观测使用每端点publish门序列化；实际磁盘等待时释放查询投影锁，避免并发事实跳号或无记录成功。连接与采集仍分开，控制不自动重发。只读导航接口不含点表和Unit值，完整模型／应用内容另行按需获取；这不是配置版本查询。

## 模型到 Unit 单层覆盖（2026-10-22）

`UnitConfig.overrides` 由 `pointOverrides`（稳定 PointId→字段补丁）、`addedPoints` 和 `removedPointIds` 组成。缺省覆盖等价空覆盖；字段缺省／null 继承，空字符串与 false 是显式值。默认初值只覆盖 `initialEngineering`，合成时复用精确编码计算 `initial`，不接受独立原始值补丁。没有模型父链、配置版本或 expectedRevision。

`effective_model` 是核心有效定义入口。基础模型保存校验所有保存引用，包括禁用 Unit；删除被隐藏点同步清理隐藏标记，仍被字段补丁引用则拒绝。创建模型＋端点、回写模型＋清理源差异、提取独立模型＋重绑都通过工作区暂存与仓储提交，不允许半提交。`PromotionPlan` 保存一次操作所需的基础内容、源和引用配置基线，提交重新比较和计算，失效需重新预览；这不是可选历史版本。

应用预览对每个 Unit 合成，再按完整有效内容相等性共享 `Arc<CollectionModel>`；相同 ModelId 不能作为映射共享条件。值始终按 Unit 独立。已应用 `UnitValues.model` 及运行 Snapshot 自包含，后续基础保存不修改既有快照。TCP／RTU 主从执行器沿用已应用定义；空有效点表可保存但不能运行。

配置暂存计入命令内容，应用暂存按每个 Unit 的基础定义＋覆盖计数，保存目录按实际共享定义指针去重、独立值计费；预算与 RSS 分开报告。批量应用进入相同准入及端点停止门禁，固定计划保存覆盖内容用于复核。新增查询 `resolve_unit`、`preview_unit_promotion`、`capture_initials` 经统一适配层调用。捕获确认值限制选区、质量、映射一致性及取样期间配置竞态，主站不发送远端请求。

前端 `UnitPointDraft` 复用 NovaSheet 和既有点表校验，按 Unit 保存历史与恢复标记；异步保存只确认提交时内容，新输入保持脏状态。基础变化需要显式重继承；回写前保护源和目标草稿。配置／运行分离，后者只读取已应用定义。有效表导出复用原模型编解码，不改变交换格式；M08 未来项目文件须保存引用和差异，旧读者不得静默忽略覆盖。

操作说明与验证边界见[使用说明](../unit-model-overrides.md)、[实施记录](../test-records/2026-10-22-unit-model-overrides.md)。
