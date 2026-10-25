# 协议接入规范与 Modbus 接口审查

日期：2026-10-25，已按104／61850场景完成第二轮设计审查。状态：接入规范草案；“建议接口”尚未实现。本规范本身不调整Modbus执行器、mio／Tokio或工程文件格式；后续G0受限验证状态见下文。后续接口提取、共享设备状态和具体协议接入分别明确提案范围与验收条件。详见[深度审查：问题、反例、依据与验收](../reviews/2026-10-25-protocol-integration.md)。

现已汇总为[协议接入与通信管理重构提案](../../openspec/changes/refactor-protocol-integration-and-communications/proposal.md)，包含[技术设计](../../openspec/changes/refactor-protocol-integration-and-communications/design.md)、五组规范、[实施任务](../../openspec/changes/refactor-protocol-integration-and-communications/tasks.md)及[验收矩阵](../../openspec/changes/refactor-protocol-integration-and-communications/validation.md)。接口仍未实现；具体重构范围以该提案为准，104／61850完整产品另案。

二次审查六项修订、后续四项及第二轮两项G0复核遗漏已同步到上述提案：[G0](../test-records/2026-10-25-protocol-g0.md)以30项受限契约／故障和真实104队列用例重新确认通过，冻结候选语义／形状。补充实际STARTDT重置条件、可查询会话失效、完整changes批次顺序及每轮Fence身份，并保留Gap后的状态高水位、修复记录受理前拒绝后的序号恢复。以下公共生产接口仍未实现；G_A通信兼容、G_B共享状态、G_C持久化／展示尚未推进。

目标：后续协议能沿同一条配置、应用、运行、查询、录制与界面链路接入，同时保留各协议的地址、数据类型、交互及确认语义。当前产品只装配 Modbus TCP／RTU 主从站；lib60870 三包与 Examples 不代表工作台支持104。

## 1. 审查结论

现有 Modbus 已建立可复用的分层、稳定身份、保存／应用分离、整端点停机、唯一运行值所有者、有界资源与可靠录制机制。应保留这些规则。

当前尚无已装配的通用协议接口。`EndpointConfig` 没有协议标识，`PointDefinition`、`UnitConfig`、点表校验、运行命令、查询和观测都包含 Modbus 假设。新增协议不能只增加枚举选项，也不能把104信息对象塞进保持寄存器模型。

推荐在冻结接口前用Modbus反例、104真实TCP队列验证和61850模型／服务反例确认公共边界，再迁移Modbus通信管理；共享状态集成验证及格式升级分别推进。正式104服务端另案。目标拆分为设备状态／控制、协议原生模型与绑定、运行资源与会话；轮询、总召、报告、控制和发布订阅保留各自语义，不提前建设全部协议平台。

## 2. 当前真实调用路径

```text
React 业务组件
  → WorkspaceClient（TypeScript 契约）
  → workspace.ts（Tauri 通信适配，携带工程会话）
  → commands/*（解析窗口所属工程，后台执行）
  → WorkspaceEngine
      ├─ 配置：Workspace → 领域校验 → ConfigRepository → 查询投影
      ├─ 从站：TcpSlaveRuntime → 固定 mio 所有者线程 → libmodbus-rs
      └─ 主站：TcpMasterRuntime → 端点所有者线程／通信桥 → libmodbus-rs
                      ↓
           运行值／查询缓存、Journal → RecordingPool
                                      ├─ 可靠协议记录文件
                                      └─ 工程 SQLite 投影
```

`tcp_slave`、`tcp_master` 和 `tcp-runtime` 名称有历史原因，实际也承载 RTU。库包装与产品运行适配器是两层：前者管理 C API 安全性，后者实现 ESSim 的配置、权限、状态、录制和生命周期规则。

### 现有接口清单

| 边界 | 现有接口与源码 | 使用规则／现状 |
| --- | --- | --- |
| 配置命令与查询 | [WorkspaceEngine](../../crates/infrastructure/src/workspace.rs)、[Workspace 服务](../../crates/core/src/application/service.rs) | `execute`／`execute_with_event`、`query`、`preview_apply`；保存不启动通信 |
| 存储 | [ConfigRepository](../../crates/core/src/application/ports.rs) | `ensure_available`、`reserve_staging`、`load`、`commit`；提交失败不得发布成功 |
| 应用／删除停机 | [ApplyBoundary](../../crates/core/src/application/ports.rs)、[ReleaseDriver](../../crates/core/src/application/resource_release.rs) | `stop`／`state`；请求停止与 `wait_released` 分开；真实生产路径另有直接停止主从运行服务的编排 |
| 从站运行意图 | [SlaveCommand／SlaveAction](../../crates/core/src/application/commands/slave.rs) | `Start`、`Stop`、`SetValues`、`ResetValues`；通过 `execute_slave` 进入实际运行 |
| 从站提交／完成 | [TcpSlaveRuntime／SlaveCompletion](../../crates/infrastructure/src/tcp_slave/mod.rs) | `execute_pending`／`execute_pending_permitted` 返回完成句柄；`wait_timeout` 超时可继续等待；同步 `execute` 有10秒等待上限 |
| 主站运行意图 | [MasterCommand／MasterAction](../../crates/core/src/application/commands/master.rs) | 连接、断开、启停轮询、暂停Unit、读、写及 Modbus 串行诊断；`execute_master` 返回投影，最终结果另查 |
| 运行查询 | [SlaveQuery／SlaveView](../../crates/core/src/application/queries/slave.rs)、[MasterQuery／MasterView](../../crates/core/src/application/queries/master.rs) | 一个Unit按稳定点ID查询，最多1024点；不等待协议网络；主站保留最近成功值及质量 |
| 应用值与迁移 | [UnitValues／migrate](../../crates/core/src/domain/values.rs)、[ModelLayout](../../crates/core/src/domain/slave.rs) | 应用快照与保存内容分离；运行后由执行器拥有实际值；ModelLayout 的四区索引是 Modbus 专属 |
| 操作互斥与准入 | [OperationGates](../../crates/infrastructure/src/operation_gates.rs)、[ResourceManager／Reservation](../../crates/infrastructure/src/resources.rs) | 应用／删除／运行操作共享端点门；额度随实际完成释放，接口超时不能提前归还 |
| 可靠录制 | [RecordingPool／RecordStream／RecordCompletion](../../crates/infrastructure/src/recording.rs) | 字节队列有界，完成确认经过真实文件同步；协议 Journal 负责编码快照和事实 |
| 运行事实 | [SlaveFact](../../crates/core/src/application/events/slave.rs)、[MasterFact](../../crates/core/src/application/events/master.rs) | 区分接收、准备、写出及操作结果；和配置提交事实分离 |
| 前端接口 | [WorkspaceClient](../../workbench/src/shared/api/contracts.ts)、[通信适配器](../../workbench/src/shared/api/workspace.ts) | 业务组件不导入 Tauri；目前运行API仍分主从，尚未按协议分派 |
| 原生库 | [libmodbus 三包](../../crates/libmodbus/README.md)、[lib60870 三包](../../crates/lib60870/README.md) | src 管来源／构建，sys 管 FFI／链接，rs 管 RAII／安全API；不依赖 core／Tauri |

### 容易误用的已有抽象

[application/runtime.rs](../../crates/core/src/application/runtime.rs) 已有 `ProtocolExecutor`、`RecorderPort`、`RunCoordinator`、`HistoryReader` 和 `Clock`。这些用于核心契约测试，当前基础设施／桌面没有装配 `RunCoordinator`，实际运行走上表的主从适配器。

其中 `ProtocolExecutor::execute(run, operation: &str)` 与 `ExecutionResult { confirmed, detail }` 不足以表达批量读写、异步完成、自发事件、取消及多阶段控制。不能把“文件里已有 trait”视为公共接入层已经完成，也不应继续以字符串操作承载新协议。提取生产接口时，应明确这套旧契约的迁移或保留用途，避免两套生产入口。

## 3. 接入104／61850前的主要缺口

以下是跨协议扩展的阻碍，不表示已有 Modbus 行为本身错误。

| 缺口 | 当前证据 | 需要形成的边界 |
| --- | --- | --- |
| 协议身份缺失 | [EndpointConfig](../../crates/core/src/domain/aggregates/endpoint/aggregate.rs) 只有 Role／Transport；TCP本身无法区分 Modbus TCP 与104 | 显式协议种类；协议、通信角色、传输分别表达并校验合法组合 |
| 点定义带寄存器假设 | [PointDefinition](../../crates/core/src/domain/aggregates/model/point.rs) 固定 Area、`address: u16`、字节／字序；[ValueType::width](../../crates/core/src/domain/point_table.rs) 返回寄存器数量 | 公共点语义与协议映射分开；寄存器占用仅留在 Modbus 编码器 |
| Unit线上地址固定 | [UnitConfig](../../crates/core/src/domain/aggregates/endpoint/unit.rs) 使用 `station: u8`，聚合校验1–247 | UnitId继续作为内部身份；Modbus站号和104公共地址使用各自有类型的绑定，不通过扩大同一个station字段硬兼容 |
| 校验及差异判断绑定四区 | `CollectionModel::validate`、`protocol_model_matches`、Unit覆盖、点表预览／交换都遍历现有 Modbus 字段 | 按协议校验映射、比较生效字段和预览编码；新增字段须覆盖应用差异、导入导出与覆盖合成 |
| 运行装配直接依赖主从服务 | `WorkspaceEngine` 持有两个具体 runtime，应用／删除／复位／关工程显式调用它们；主站操作门还借用从站 runtime | 由应用层按协议选择适配器，公共操作门独立共享；所有生命周期入口进入同一分派边界 |
| 完成与能力语义不同 | 从站完成句柄和主站异步投影不同；[Capabilities](../../crates/core/src/application/queries/capabilities.rs) 只有全局布尔值 | 统一提交／查询结果的含义，按协议×角色×传输声明实际操作能力，不从库是否编译推断 |
| 样本与事件尚非通用 | 从站值无质量字段；主站质量为字符串；事实分为 SlaveFact／MasterFact，观测内层为JSON字符串 | 公共样本保留有效性、时间、来源；协议质量位和业务事件使用有类型的扩展 |
| 观察与存储识别具体事件名 | [ObservationPanel](../../workbench/src/features/workspace/ObservationPanel.tsx) 根据role选API；[工程运行投影](../../crates/infrastructure/src/project/runtime.rs) 根据 Modbus 事实名筛选报文、识别结束 | 观察记录显式带协议／记录类别／阶段；结束由生命周期事实表达，不能猜字符串 |
| 自发上送与可靠屏障未接入 | lib60870安全层提供 `enqueue`／`tick`，当前无业务变化检测、发送字节或协议确认的安全观测接口 | ESSim生成变化事件；适配器提交发送。库接收入队不能冒充可靠落盘、实际写出或主站确认 |
| 权威设备值尚未建立 | Device仅是关联实体，UnitValues和C Mapping由具体协议运行持有 | 权威设备状态、远端观测缓存、协议投影分别拥有；多协议通过显式Binding联动 |
| 控制缺少设备动作边界 | 当前主站Write和从站SetValues围绕点值，104安全API只支持回调内回复 | 设备动作与直接置数分开；协议原生控制模型、过程检查、会话仲裁和实际状态反馈分别处理 |
| 61850模型不能用点列表达 | 当前模型没有LD／LN／DO／DA、FC、DataSet或RCB | 独立原生模型资产与有类型引用；点表仅作为可展平业务投影 |
| 单端点／主从假设不适用于服务组合 | 当前生命周期以EndpointId操作门和TCP／RTU角色编排 | 服务profile、通道和原生模型实例分开；依赖范围决定应用／停机，GOOSE／SV不强制TCP连接或逐帧确认 |
| 原生库公共符号可能重叠 | lib60870构建默认携带与libiec61850共享的common／HAL，目前尚无双库同链证据 | 明确公共符号唯一提供者或隔离方案，验证ABI、静态／动态共存，不能靠Rust包名区分C符号 |

## 4. 必须保持的接入规则

### 4.1 身份、配置与映射

1. EndpointId、UnitId、PointId是现有稳定业务身份，线上地址、名称、目录路径和表格行号都不能替代。现有Modbus点实例以端点、Unit、Point的组合定位，复用同一模型的不同Unit具有独立值；跨协议共享状态另有StateScope／Signal身份，经Binding连接，不能复用一个PointId就认定值相同。
2. 设备类型与协议实例分开。当前Unit可关联DeviceId，不代表已经有可跨协议共享的设备值引擎。后续同一设备暴露多协议时，权威状态由设备／仿真状态所有者提交，协议持有投影和会话缓存。无设备关联的旧Modbus实例保留独立状态作用域，不强制生成设备或改变已有值语义。
3. 每个模型只保存当前内容，Unit引用稳定模型ID，保留单层覆盖；禁止引入历史选版、升级或expectedRevision。协议映射拆分不等于引入配置版本。
4. 新协议配置使用有类型的枚举／结构，不使用万能地址字段、任意JSON字段包或为所有点堆积可空协议字段。61850对象引用允许以经过解析、带类型／FC上下文的字符串表达，禁止无校验的万能字符串。首批按具体服务profile静态分派，无需插件注册框架；master／slave不是所有协议服务的必选角色。
5. Modbus映射保留数据区、零基地址、字节／字序、访问权限及采集参数；104映射单独定义公共地址、IOA、信息对象类型及适用的总召／上送／控制参数。地址范围和支持组合由该协议实现校验，不套用Modbus限制。
6. 数值精度、倍率转换可以复用纯逻辑；寄存器编码与长度计算不能成为公共值类型的定义。前端仍不能用 JavaScript Number 代替精确整数／十进制转换。
7. 61850保留独立的原生模型资产、层次、结构／数组、FC、DataSet有序成员及控制块。业务Signal仅映射需联动的数据，不要求每个DA都是标量点；控制目标与反馈Signal可不同。各绑定声明类型／单位／倍率转换，避免把Modbus寄存器raw值直接复制给其他协议。

### 4.2 保存、应用与启动

```text
编辑草稿 → 纯校验／应用预览 → 保存当前配置
显式应用 → 整组预校验／预算预留／生命周期凭证 → ScopeFence关闭新写入
         → 必要协议停止＋已受理提交／动作排空 → 最终快照
         → 重新核对候选定义 → 可靠配置提交 → 原子发布schema／迁移值／已应用快照／epoch
显式启动 → 解析当前已应用计划凭证及自包含资产 → 校验实际能力 → 可靠记录／通信准入
```

保存不自动应用，应用后保持停止。应用／删除以实际资源依赖范围处理，不能只停止选中Unit。现有Modbus边界是完整端点／串口；共享状态schema或IED模型变化时须预览所有受影响实例，纯协议映射修改尽量只影响该协议。预校验失败不先停机；停机后若配置核对或提交失败，保持旧应用和值并如实显示已停止范围，不伪装回运行态或发布半份新映射。

SavedDefinition是候选应用依据，AppliedSnapshot保存已应用定义及自包含资产，RuntimeSnapshot固定本次运行内容。保存B未应用时仍能启动应用A；只有显式应用成功才替换内部AppliedPlanToken。不能因最新保存／源文件变化拒绝合法A或混入B，不能接受调用方任意PreparedPlan。

共享schema迁移必须由项目StateRegistry封闭本地、远端、绑定采样和DeviceCommand全部写入口；排空后取最终值，不能从预览旧值迁移。旧epoch迟到写拒绝，未知提交／配置决策阻塞迁移并进入受控恢复，不能强行解锁。确定失败可恢复旧schema准入，协议不自动重启。G_A仍保持原Modbus应用路径，上述共享机制在G_B引入。

配置锁只用于短时读取／提交，不在锁内等待网络、磁盘同步或C回调。停止必须确认监听、连接、句柄及回调均结束后才发布Stopped。失败状态不自动代表资源已经释放。

### 4.3 运行所有权与调度

每个权威状态、观测缓存和C句柄有明确所有者。当前Modbus运行值仍在协议执行器中，不能把这一实现冻结为跨协议规则。目标由状态所有者提交批次，适配器投影到原生库模型；远端观测是否可改变仿真设备状态需显式绑定和仲裁。前端编辑产生草稿，经命令提交；查询投影只读，不触发网络采集。

公共接入接口不规定 mio 或 Tokio。Modbus保留现有实现；104可由固定线程驱动 `tick`，具体调度随其产品提案决定。现有线程绑定句柄在所有者线程创建、使用和释放，不为统一接口添加unsafe Send。后续库若启用内部线程，必须逐API核实回调线程、已持锁、重入及停止契约，不能假定所有回调都在Rust所有者线程；跨线程传输自有消息，不泄露借用句柄。

命令、点查询、事件队列、连接和录制均有条数／字节／时间边界；满载需明确拒绝或按已声明规则降级。停止／查询不能被无限业务队列饿死。等待超时不释放仍在使用的额度，不自动重复发起同一控制操作。

停止使用可信LifecycleLease并先锁存，不重新申请被在途业务占用的DataPermit；应用内部复用已有生命周期上下文。即使超时操作占门且普通队列满，停止仍可送达所有者，但不夺取原许可或强制打断C／磁盘操作。ResourcesReleased和操作／记录Finalized分别查询。

工程状态按窗口所属工程隔离。当前各WorkspaceEngine拥有各自资源账本，不能把单工程上限当作整个进程所有窗口的合计保证；新增协议复用所属工程账本，跨工程总预算另行评估。

原生库层还需验证进程级共存。lib60870／libiec61850共享common／HAL，链接所有权和ABI兼容必须明确；不同Rust `links` 不等于C符号命名空间隔离。模型对象图和回调状态的寿命覆盖全部使用者，不能通过序列化C指针保存工程。

### 4.4 操作结果

通用接口必须区分三类信息，不用一个 `success` 混在一起：

| 类别 | 必须表达的状态／证据 |
| --- | --- |
| 提交与执行 | 未受理、已受理、执行中、已完成、已取消、结果未知；返回投影或入队成功不能当作完成 |
| 协议交互阶段 | 本地提交、库入队、实际写出、协议确认／拒绝等；协议不支持或封装不可观测的阶段明确标为未知／不可观测 |
| 业务判断 | 回验一致／不一致、执行反馈、实际设备状态；协议确认不等于设备动作完成 |

操作以稳定command／operation身份关联结果。单次提交可继续查询／等待真实结果；接口超时保持待确认，不能假定动作没发生。身份关联本身不是跨进程exactly-once或去重保证。

直接置数和设备动作分开。动作入口保留来源协议／会话、发起者、控制目标、测试／检查要求和期限，接受远端请求时不要求存在前端commandId。原生选择／执行状态机尽量交给成熟库；ESSim负责过程检查和业务执行，协议确认、命令终止及设备反馈分别呈现。共享设备的控制冲突按控制目标仲裁，端点操作门只解决生命周期冲突。

批量本地点值修改先整批校验后提交；远端批量命令可拆成多请求，不承诺跨请求事务，必须报告已执行、拒绝、未知和未执行范围。取消只阻止可取消的后续动作，不撤销已经写出的控制。

### 4.5 变化事件与自发上送

```text
本地置数／显式绑定的远端采集／仿真计算
  → 项目StateRegistry整批校验 → 唯一ScopeJournal意图／提交决策可靠确认
  → 原子发布值／游标 → StateCommit（受理不等于提交）
      ├─ Modbus从站：更新映射，后续请求读取
      ├─ 104服务端：判断上送条件 → 生成自发ASDU → 排队并驱动发送
      └─ 61850服务端：更新原生模型 → 库按模型与RCB检查报告触发
```

上图是目标数据流，当前没有已装配的公共状态批次接口。值变化、质量变化、同值再次更新保留不同语义；不可统一去重后交给全部协议。实现可先采用所有者内部的有序批处理／直接调用，不要求通用事件总线。原生库回调、配置提交事件、值变化事实、可靠运行记录及前端快照通知是不同边界。

共享scope不归协议RunId拥有，没有协议运行时也能提交；协议Journal只关联CommitId及自身阶段。决策确认前不能投影；确认丢失或sync_data报错且可能已落盘时保持Unknown和最后确认视图，阻止冲突写入／迁移，不能当作确定未提交。发布后某投影失败不回滚权威值，也不生成源协议成功。LegacyRawScope保持旧所有者／Modbus可靠屏障及未知语义，不叠加共享业务决策。

连续记录序号在RecordingPool.append成功受理后才推进，受理不等于可靠确认。意图在受理前因临时队列额度不足明确拒绝时返回JournalRejected，不产生Pending、可见新值或投影，不消耗序号；额度释放后可显式发起新提交，不自动重发。操作ID允许跳号；已受理后的故障／未知及永久Journal故障仍隔离，不能借预算恢复绕过。

61850应使用原生库模型更新API，让库处理DataSet／RCB触发与报告行为，不在ESSim复制报告引擎；依据见[官方数据模型更新API](https://support.mz-automation.de/doc/libiec61850/c/latest/group__IEC61850__SERVER__DATA__MODEL__ACCESS.html)。GOOSE、SV属于独立服务阶段，不能一概当成“数值变化才发送”。

104的变化判定、质量变化、遥测死区、遥信防抖、总召与变化流顺序、断线积压、队列满和重连恢复策略，须在104提案中明确。lib60870不监视ESSim点表；`enqueue`成功只表示本地排队。当前封装上限64条，满时返回 `WouldBlock`；不能默默覆盖尚未处理事件。

“最后上送值”应按适配器声明的事件接纳规则推进，入队失败不得当作已上送；库入队、协议确认、持久化交付检查点分别记录。没有确认观测能力时，不得宣称已可靠交付。变化来源需保留，跨协议联动应防止把自身回写再生成同一操作的循环。

缓冲、最后接纳／发送位置及恢复范围按冗余组／RCB／订阅区分，不用一个端点级计数代表全部接收者。当前104安全层为单冗余组，不能从允许多个TCP连接推断多主站独立事件交付。

总召／快照与变化流须明确切点、顺序和缺口重同步。查询快照带内部提交游标，变化批次有范围；104总召期间不能被后来的旧事件回退。61850客户端按报告实际包含的成员更新，未包含成员不清零，按协议提供的分段、原因及模型标记核对映射；不假设任意跨协议发布都能原子完成。

当前104回调回复走高优先级队列，Server.enqueue走低优先级队列，StateCursor不能重排已入C队列的帧。G0验证了受限当前状态监控的每会话单应用FIFO、总召／自发同一高优先级路径；C队列满保留队首。入队前还须校验完整changes的(epoch, after, through)，after接续已接纳游标、有进展时through前进；当前epoch／位置的空轮询不触发Gap；同次提交的多帧整批准入。快照cut不能落后于接纳高水位，乱序／重叠／断档及旧快照返回Gap，重捕当前权威快照后恢复，不把单FIFO当作接纳前顺序的证明。

已接纳的状态来源epoch／高水位包含尚未发送的帧，与可丢弃待发队列分开维护；Gap、会话切换及连续重同步失败不能清除。重同步的旧cut或错误来源epoch仍须拒绝，持续停发并拒绝变化，直到同epoch且cut不落后的新一致快照通过，再接续变化。状态所有者改变须显式重新绑定，不把传输会话代际当状态代际；G0使用同步新捕获回调，真正异步快照还须绑定请求／来源身份，尚不视为已实现。

SessionToken／自有总召数据／session_is_current／try_send_to在默认关闭feature内验证。每tick后不发ASDU即可查询会话代际有效性；重置检测匹配固定上游实际接受并清队列的分支，包含其非标准输入且不得误判TESTFR。失效时替代旧应用积压、拒绝旧会话回复，先排入新当前快照再接续新变化；空FIFO同样触发重同步，不推测C缓冲损失、不宣称SOE／离线无损。vendor及生产控制范围未改，详见提案D8及验收矩阵。

### 4.6 可靠记录与观察

保留现有Modbus“可靠意图→执行一次→可靠结果”的规则。新服务profile分别声明业务控制审计、状态提交记录和连续流捕获保证；控制审计失败时不得退化为无记录继续执行。GOOSE重复报文／SV采样流不能强制逐帧sync_data后发送；需独立、有界捕获策略、预算及缺口说明，完整无损捕获无法满足时明确拒绝或停止受影响服务。不得以此擅自放宽现有Modbus保证。

C库回调必须防止panic跨ABI，并按其生命周期复制必要数据；不能把借用连接／ASDU带出回调。同步回调不可等待只有该回调返回后才能推进的任务。支持延迟处理的阶段使用库声明的机制；当前104封装只有回调内回复，不足以支持异步延迟业务完成，新增控制范围前需补安全接口。

对104，业务响应／上送与库内部链路维护报文要分别声明可拦截、可观察的边界。不得默认能复制 Modbus 的“响应准备后、实际socket发送前”屏障。若当前安全API无法满足阶段要求，先补窄接口和测试，或在提案中明确受限能力，不能用事后日志冒充发送前门禁。

建议统一事件信封：工程上下文、所属记录流身份、sequence、单调相对时间／墙钟、可选unit／point／operation、类别、阶段和有类型载荷。协议流关联protocol／endpoint／run，ScopeJournal关联scope／scope session，无协议运行时不伪造RunId。协议质量位、COT、功能码等保留在协议载荷中，不提升为所有协议必填字段。

界面最近窗口可合并／截断并说明边界；可靠记录不能跟随界面暂停而停止，不能无说明丢弃。完整协议记录、SQLite查询投影、可丢应用诊断日志分别处理。新事件类别和结束事实必须覆盖工程投影，格式schema标记仅用于解析兼容，不是配置版本功能。

协议会话平面与工程配置分开：RCB预留／启用、动态DataSet、控制选择和协议计数保存在运行层，不能自动覆盖工程。61850 ConfRev、EntryID和GOOSE stNum／sqNum是协议状态，保留它们不等于恢复用户配置版本管理；BRCB缓冲也不自动代表进程重启后的可靠交付。

## 5. 建议整理的公共接口

以下名称和签名为讨论草案，尚无对应生产trait。只在消费边界确实需要替换／注入时使用trait；纯规则可以是普通模块和函数。领域类型与应用端口放core，具体资源、队列、文件和库调用放infrastructure，前端契约留在通信适配层；原生句柄不进入core。

| 接口组 | 建议操作与输入／输出 | 必须保证 |
| --- | --- | --- |
| `ProtocolDescriptor` | `describe(serviceProfile, platform) → capabilities` | 声明具体MMS／报告／控制或104服务范围、库版本／标准edition、实际支持角色与通道；库存在不等于产品可运行 |
| 原生模型／绑定与应用规划 | `validate(definition, modelAssets, bindings) → issues`；`compile(appliedSnapshot) → PreparedPlan`；`impact(old, next) → ApplyImpact` | 纯解析规划与文件读取分离；保留结构、引用及依赖；规划只含不可变数据与资源估计，C句柄在安全所有者内创建 |
| 状态所有者 | `snapshot(scope) → StateSnapshot(cursor)`；`submit_patch(StatePatch) → CommitOperationId`；`commit_status(id) → CommitStatus`；`changes(scope, after) → ChangeBatch / Gap` | 项目StateRegistry独立拥有；唯一ScopeJournal确认后原子发布，受理不等于提交，协议只投影 |
| 状态迁移 | `begin_quiesce(scopes, lifecycleLease) → ScopeFence`；`quiesce_status`／`final_snapshot`／`publish_migration`／`abort_quiesce` | Fence绑定scope／schema epoch／本轮quiesce身份，结束即失效；关闭写入口、排空、最终快照、新epoch；未知阻塞，旧句柄／旧Fence拒绝 |
| 设备动作 | `submit(DeviceCommand, InvocationContext) → CommandHandle`；`command_status(id)` | 动作目标与反馈值分开；来源会话／测试／期限及仲裁完整；不复制库已有原生控制状态机 |
| 运行命令 | `submit(CommandEnvelope<RuntimeAction>) → OperationHandle`；`operation(id) → OperationStatus` | 唯一一次受理与可继续查询结果；封装已有主从不同完成机制；显式拒绝不支持的操作 |
| 停机边界 | `stop(instance, reason, lifecycleContext) → OperationHandle`；`wait_released(instance, timeout) → ReleaseStatus` | 生命周期凭证不受普通业务门阻塞，原许可不提前归还；资源释放与最终确认分开；停止协议不重置设备 |
| 查询与投影 | `query(RuntimeQuery) → RuntimeView`；`project(StateCommit / Snapshot) → ProjectionReceipt` | 按稳定身份／模型节点有界查询；支持原生模型非Signal对象；回执区分投影完成与实际协议交付 |
| 可靠记录与捕获 | `open(snapshot, policy) → stream`；`append(envelope) → completion` | 业务审计与连续流策略分开；可可靠写入部分复用RecordingPool；明确观察阶段、缺口和失败范围 |
| 前端协议适配 | 公共运行查询／操作客户端＋协议专属属性、点列和报文解析组件 | 业务组件只依赖客户端；按实际能力呈现操作，复用公共工作台组件 |

### 协议通信管理适配

每工程装配一个 `ProtocolCommunicationManager`，由应用服务把手动、批量、应用、删除、关闭工程和退出统一路由到已注册的协议适配器。进程级窄通道租约协调跨工程监听／串口冲突，业务状态和操作仍按工程隔离。

| 管理器接口 | 职责 |
| --- | --- |
| `capabilities(profile)` | 具体服务／平台的实际装配、限制和不可用原因 |
| `start(instance, appliedPlanToken, invocation)`／`stop(instance, reason, lifecycleContext)` | 管理器解析自包含已应用计划；可信停止先锁存且不申请普通业务许可，返回可跟踪操作 |
| `submit(instance, ProtocolAction)`／`cancel(operation)` | 有类型动作、有界调度和取消余项，不撤销已发出控制 |
| `operation(id)`／`status(instance)`／`sessions(instance, page)` | 查询结果、运行／连接／数据就绪／录制状态和有界会话页，不触发网络采集 |
| `wait_released(instance, deadline)` | 后台有限等待真实资源释放；超时不能声明Stopped或提前归还凭证 |

管理器监督生命周期、能力、预算和唯一重连归属；实际收发、分帧、协议定时、总召、报告与控制状态机留在适配器／成熟库。无需统一socket读写或切换线程模型。监听、TCP连接、104传输激活、MMS报告就绪分别表达；GOOSE／SV不要求TCP会话。

原有主从IPC保留兼容层并委托同一管理器；其生产运行不直接复用测试用RunCoordinator。TCP拨号目标不独占，串口真实身份独占，L2网卡按服务共享；进程租约预检不能代替OS打开／绑定。详细启动、停止、恢复、失败和迁移顺序见[提案设计D3–D6](../../openspec/changes/refactor-protocol-integration-and-communications/design.md)。

### 命令和查询的数据边界

`RuntimeAction`首批提取启动／停止等生命周期能力；本地置数／恢复路由到状态作用域，现有SlaveAction保留兼容适配。启动不得隐式令所有客户端开始轮询或启用报告。协议动作使用静态有类型分支，例如Modbus读／写／轮询／串行诊断、104总召、61850报告配置；设备控制另走动作边界。不能让104实现空的 `StartPolling` 来适配接口。

`RuntimeView`公共部分表达运行身份、生命周期、连接／录制状态、当前operation、不确定性、错误、最后可靠位置及样本；各协议统计和操作细节留在专属分支。不要让所有协议填 `requests`／`exceptions`／`polling` 以满足Modbus投影。

建议 `PointSample` 分别表达值（可空）、接入有效性、观测时间、可选源时标及其精度／质量、来源及协议原生质量。通讯失败保留最近成功值和时间，另外更新有效性；失联不是零值，源时标和本机接收时间不混用。当前RawValue可作为业务Signal精确标量基础；原生结构／数组／枚举等另按支持范围保留，不能全部强转数值，也不预建所有协议值系统。

设备对时不修改宿主系统时钟或其他工程；设备仿真时间、源时间、宿主墙钟与进程单调期限分别处理，具体对时服务另定。

### 前端与工程文件接入

新增协议至少核对创建入口、模型编辑／覆盖、属性面板、保存／应用差异、运行工具栏、值表、观测解析、导入导出、工程恢复及批量生命周期。后端支持但前端不显示的能力要明确标记，前端出现图标不能算协议交付。

属性面板统一用 [property-panel](property-panel.md)，业务弹框统一用 [workbench-dialog](workbench-dialog.md)，点表继续复用NovaSheet和稳定点ID。协议选择器显示中文含义并保留准确标识。支持的字段组合直接用现有React组件，不先建立动态schema表单系统。

工程文件增加协议种类／映射时必须有现有Modbus工程fixture的读取、覆盖保存及重新打开证据；迁移规则单独设计，不默认为所有旧文件补写，也不在读取时静默丢弃未知协议或映射。未知／不支持的内容明确失败或进入已设计的受限模式。

## 6. 提取与接入顺序

| 阶段 | 具体交付 | 验收条件 |
| --- | --- | --- |
| G0：冻结前验证 | 最小设施验证六项反例，104实际TCP队列及窄监控安全API、61850模型／服务反例前移 | 不成立则修订设计，不能先冻结接口再全部迁移 |
| G_A：通信管理兼容重构 | 四种Modbus profile、全部生命周期、旧IPC／UI统一管理，不换线程、值写路径／Journal／格式1 | 独立TCP／PTY、可靠记录故障、headless及macOS工作台回归通过后单独验收 |
| G_B：共享状态与原子应用 | StateRegistry、唯一提交、投影回执、ScopeFence／最终快照迁移、动作；真实Modbus＋104共享Example及61850反例 | 原生队列／提交／迁移故障复验，保持G_A通过；暂不保存新格式，不注册新协议生产能力 |
| G_C：格式与展示 | 显式格式2可靠升级／恢复及只读原生元数据／统一观测UI | 新旧工程与故障矩阵、G_A／G_B持续通过，验收范围明确 |
| 后续104产品 | 单独明确首批点型、CA／IOA、总召／变化／控制、记录和工作台接入 | 独立栈／EMS逐步互操作，不能把测试Example算完整产品 |
| 后续61850分服务 | 明确edition／模型／服务、库版本／共存／回调，MMS先评估、GOOSE／SV另阶段 | 复用原生机制，独立业务／部署／互操作证据后才启用 |

G_A独立交付通信管理兼容性，G_B独立交付共享状态；不把模块重命名当作接口提取完成。公共层应能不增加另一路应用／删除／退出分支地装配第二协议，协议特有操作仍可扩展。阶段前后关系及证据以提案任务／验收矩阵为准。

## 7. 新协议接入检查单

- [ ] 官方库版本、来源、许可、平台与已封装API范围核实；源码保持原样，unsafe所有权／线程／回调契约完整。
- [ ] 多库同进程静态／动态链接验证，common／HAL等公共C符号所有权和构建模式明确。
- [ ] 协议×角色×传输的能力矩阵、首批点型、地址语义与控制含义明确；不支持的组合明确拒绝。
- [ ] 配置校验、覆盖合成、应用差异、值迁移、导入导出和旧工程兼容覆盖；保存／应用仍分开。
- [ ] 启动、停止、应用、删除、批量、关工程和退出使用同一资源所有者；确认释放后才能重绑／重开。
- [ ] 运行命令有类型、有稳定因果身份，受理／完成／拒绝／取消／未知可区分；控制超时不自动重发。
- [ ] 值与质量／时间来自真实路径，变化事件和协议发送有明确界限；队列满、断线恢复和循环反馈策略可测。
- [ ] 多协议共享状态与控制仲裁明确；停止一个协议不复位设备，原生结构／FC／DataSet成员与协议计数不丢失。
- [ ] 适用协议覆盖总召／报告切点、质量变化／同值更新、部分报告、重同步、控制选择／超时／跨会话拒绝。
- [ ] 按声明的记录profile检查启动门禁；要求可靠记录的服务在建立失败时不打开业务通信；阶段屏障和录制失败有故障注入证据，SQLite与观测能识别新协议。
- [ ] 逐服务声明控制审计与流捕获保证；GOOSE／SV时序、网卡接入及缺口另验，不宣称逐接收者确认或自动重启续传。
- [ ] 命令／事件／连接／内存／录制均有界，慢客户端和单端点故障隔离；查询不等待网络。
- [ ] 提供无Tauri运行Example及独立客户端测试；自测不能替代独立成熟协议栈或真实设备互操作。
- [ ] 公共属性面板、弹框、Dockview与NovaSheet复用；原生焦点、窄窗口和实际协议流程验收。
- [ ] README、项目目标与协议实施记录如实更新；未验平台、硬件、规模和时序不写成已支持。

## 8. 本次审查与验证边界

审查依据为当前工作区源码及已有测试，不以架构文档中历史阶段描述代替当前实现。发现 `workspace.md` 中仍有M01／M03时期“尚无执行器”的历史段落，因此上面的接口清单优先指向实际代码；本轮不批量改写历史记录或Modbus计划。

首轮Modbus现状审查针对公共规则复跑以下四组既有测试，共17项通过、零失败、零忽略：

```sh
cargo test -p essim-core --test architecture_boundaries --test resource_release --test point_apply --test runtime_contracts --offline
```

这些验证的是核心分层、应用／迁移、停机和录制契约，不是新增公共接口已实现。第二轮结合官方API和设计反例审查104／61850，未重复运行这些测试、编译libiec61850或执行真实EMS、原生UI与硬件验证。文档相对文件链接和本轮差异空白检查通过；参照的官方来源及版本差异见[深度审查](../reviews/2026-10-25-protocol-integration.md)。
