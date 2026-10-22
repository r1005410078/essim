# Modbus 模拟器事件风暴与运行流程

更新日期：2026-10-18。原图始于2026-10-15；本次将当前代码流程与目标业务流程分开标明。第0节与0B节对应现有实现，第1–3节是业务设计视角，不能整体当作已接入能力。

关联：[M01 基础模型与架构边界](../plans/modbus-simulator.md#m01-基础模型与架构边界)。

第0节逐步对应服务、命令、提交事件、查询和前端订阅的实际调用。后续三张业务图保留配置与启动、执行与可靠录制、离线回放三个设计视角；事件后的连线表达业务先后或目标协作，不表示已存在事件总线、自动策略监听器或数据库。TCP从站已增加独立运行命令、固定线程执行器和可靠文件记录，见0B节；RTU、通用业务事件分发及可用回放仍由后续提案实施。

图例：蓝色为命令，黄色为聚合，橙色为事件，紫色为自动策略，绿色为查询／展示，灰色为应用或技术组件，红色为故障。

## 0. 当前代码：新建模拟器的命令、查询与通知

此图是当前成功路径的调用时序。前置条件：用户已选择保存过的模型，命令携带完整端点及首个Unit。保存只写本次会话的MemoryRepository，不启动通信。

```mermaid
sequenceDiagram
    participant UI as React WorkspaceStore
    participant API as Tauri commands
    participant E as WorkspaceEngine
    participant S as Workspace 应用服务
    participant H as endpoint 命令处理器
    participant R as MemoryRepository
    participant L as WorkspaceProjectionListener
    participant P as Projection 查询投影
    participant N as 配置订阅线程

    UI->>API: execute_workspace(Command / CreateEndpoint)
    API->>E: execute(command)
    E->>S: execute(command)
    S->>R: load() 读取权威目录副本
    S->>H: create_endpoint(catalog, endpoint)
    Note over H: 领域校验 + 模型引用检查<br/>准备 EndpointCreated 事实
    H-->>S: Handled(object_id, fact)
    S->>R: commit(catalog) 原子替换内存目录
    R-->>S: Ok
    Note over S: 提交成功后构造 ConfigEvent<br/>包含 fact、sequence 和 catalog 快照
    S->>L: handle(projection, event) 同步调用
    Note over L: 检查schema及通知顺序<br/>重复忽略，缺口拒绝
    L->>P: 更新 sequence 与 catalog 快照
    Note over L,P: 消费完整信封，没有按fact路由
    L-->>S: Ok
    S-->>E: Receipt + ConfigEvent
    Note over E: execute_with_event 可返回二者<br/>生产 execute 只保留 Receipt，没有 publish
    E-->>API: Receipt
    API-->>UI: 命令成功

    UI->>API: query_workspace() 主动刷新
    API->>E: query()
    E->>S: query()
    S->>P: query()
    P-->>S: WorkspaceView
    S-->>E: WorkspaceView + 当前资源状态
    E-->>API: WorkspaceView
    API-->>UI: 更新目录与界面

    Note over UI,N: connect 时已另行建立快照订阅<br/>下面不是 EndpointCreated 业务事件监听
    loop 每100ms检查一次
        N->>E: sequence()
        E-->>N: 当前配置通知序号
        opt 序号与该订阅上次发送位置不同
            N->>E: query()
            E-->>N: 最新 WorkspaceView
            N-->>UI: Tauri Channel 推送完整快照
            Note over UI: receive(view) 忽略旧快照<br/>通知界面重新渲染
        end
    end
```

校验或commit失败时，不返回成功ConfigEvent，不推进配置通知序号。应用／删除另有资源释放确认边界；释放已确认但提交失败时，停止事实保留，不能由本图的新建路径推断其失败状态。

### 图中组件对应哪份代码

| 图中职责 | 实际代码 | 当前行为 |
| --- | --- | --- |
| 前端命令入口和主动刷新 | [WorkspaceStore.execute／refresh](../../workbench/src/features/workspace/model.ts) | 构造Command，收到Receipt后主动查询，不依赖后台订阅才能刷新 |
| Tauri命令与查询适配 | [execute_workspace／query_workspace](../../workbench/src-tauri/src/commands/simulator.rs) | 写命令在后台任务执行；读取返回WorkspaceView |
| 线程安全应用入口 | [WorkspaceEngine](../../crates/infrastructure/src/workspace.rs) | execute内部调用execute_with_event，但生产入口只返回Receipt |
| 应用服务 | [Workspace](../../crates/core/src/application/service.rs) | 权威加载、命令分派、原子提交、事件信封和同步投影更新 |
| 命令定义及处理 | [Command／Action](../../crates/core/src/application/commands/mod.rs)、[create_endpoint](../../crates/core/src/application/commands/endpoint.rs) | 命令是写入意图；处理器调用领域规则，准备事务内结果 |
| 已提交配置事件 | [ConfigurationFact／ConfigEvent](../../crates/core/src/application/events/mod.rs) | EndpointCreated表达成功业务事实；没有发布到业务监听器 |
| 权威存储 | [MemoryRepository](../../crates/infrastructure/src/workspace.rs) | 内存整体替换，没有SQLite或其他本地数据库 |
| 查询投影 | [Projection／WorkspaceView](../../crates/core/src/application/queries/mod.rs) | 构造只读查询快照；accept兼容入口委托给同一监听器 |
| 同步事件消费者 | [WorkspaceProjectionListener::handle](../../crates/core/src/application/event_handlers/workspace_projection.rs) | 服务提交后显式调用，按序号复制catalog快照；不按fact路由 |
| 后台快照通知 | [subscribe_workspace](../../workbench/src-tauri/src/commands/subscriptions.rs) | 每100ms检查序号，变化后查询并通过Channel发送最新快照，可合并中间变化 |
| 前端实际监听点 | [channel.onmessage](../../workbench/src/shared/api/workspace.ts)、[WorkspaceStore.connect／receive](../../workbench/src/features/workspace/model.ts) | 监听WorkspaceView消息及本地Store变化，不监听EndpointCreated |

### 当前缺口：具体业务事件尚未分发

目前不存在 `on(EndpointCreated, handler)`、事件分发器或按业务事件注册的监听器。ConfigurationFact已经生成；现有WorkspaceProjectionListener只同步消费信封中的快照，生产入口没有继续分发给其他业务监听器。不能把“已有具体事件类型”写成“已经完成事件发布与监听闭环”。

| 需要区分的动作 | 当前状态 |
| --- | --- |
| 提交成功后生成具体配置事实 | 已接入 |
| 服务同步把事件交给查询投影 | 已接入，通过event_handlers中的同步投影监听器 |
| 前端监听查询快照Channel | 已接入，序号检查后推送 |
| 按EndpointCreated等业务事件注册、发布和调用监听器 | 未接入；监听器职责、失败处理和通知顺序仍待设计 |
| 事件可靠存储、重试和重放 | 未接入；配置事实不能替代可靠运行历史 |

后续如果要求事件监听驱动投影或其他服务，需要先明确哪些处理必须同步完成、哪些允许失败后补做、如何避免重复处理，以及查询何时能看到提交结果，再形成实施任务。本次目录细分已将现有同步消费者独立为event_handlers模块，图同步反映该调用；按具体fact注册及分发的缺口仍保留，不引入消息平台。

## 0B. 当前代码：TCP从站运行（2026-10-18）

本路径使用独立SlaveCommand／SlaveView及SlaveFact，不把运行操作发布成ConfigEvent。既有RunCoordinator仍是通用契约测试；正式TCP路径没有假装装配该泛型协调器，也没有通用事件总线。Journal是SlaveFact的显式同步消费者，文件记录可靠确认后才继续。

```mermaid
sequenceDiagram
    participant UI as 工作台运行栏
    participant API as commands/slave.rs
    participant E as WorkspaceEngine
    participant M as TcpSlaveRuntime
    participant W as 固定工作线程Owner
    participant J as Journal文件记录
    participant C as libmodbus响应器及Unit映射
    participant EMS as 外部主站
    UI->>API: execute_slave(SlaveCommand/start)
    API->>E: 后台执行
    E->>E: 单端点操作门、读取已应用快照
    E->>M: execute(command, snapshot)
    M->>W: 有界命令队列 + Waker
    W->>W: 共享ModelLayout、独立Unit Mapping
    W->>J: 建立自包含快照 + sync_data及目录同步
    W->>W: 绑定监听、注册mio
    W->>J: Listening事实可靠确认
    W-->>UI: SlaveView实际运行状态
    EMS->>W: TCP请求（可能分段或连续）
    W->>W: 有界MBAP缓冲、完整帧校验
    W->>J: Request事实可靠确认
    W->>C: 检查完整范围及写权限，调用libmodbus
    C-->>W: 已准备响应、实际映射值
    W->>J: ResponsePrepared事实可靠确认
    W->>EMS: 非阻塞发送队列写出
    W->>J: ResponseSent事实可靠确认
    Note over W,J: 记录失败停止该端点、保留不确定范围<br/>本地发送成功不等于EMS业务确认
    loop 可见端点面板，每500ms串行查询
        UI->>API: query_slave(当前Unit可见行，最多1024点)
        API->>M: query，不复制整个配置目录
        M->>W: 查询命令
        W-->>UI: 实际状态、选定点值、最近100条事件／帧
    end
```

本地置数先经ModelLayout.plan_values整批校验，然后记录ValuesIntent、修改映射、记录ValuesCommitted；同一所有者也处理外部写入。恢复指定数据区地址范围／全部点使用已应用初值。前端刷新与提交均不依赖选区：刷新由RuntimeGrid公开视口解析稳定身份；提交由RuntimeDataSource待修改集合归集当前Unit各分类草稿，成功后只清除提交时版本，期间的新编辑继续保留。停止命令在所有者完成当前同步处理后关闭该端点全部客户端及监听；尚未写出的响应明确保留不确定标记。apply／delete先停止并接回实际值，再沿配置分段释放流程执行；成功后清理旧静止映射。

| 职责 | 实际文件 |
| --- | --- |
| 运行命令／查询／事件定义 | [commands/slave.rs](../../crates/core/src/application/commands/slave.rs)、[queries/slave.rs](../../crates/core/src/application/queries/slave.rs)、[events/slave.rs](../../crates/core/src/application/events/slave.rs) |
| 模型地址、权限及批量规划 | [domain/slave.rs](../../crates/core/src/domain/slave.rs) |
| 应用边界及配置协调 | [WorkspaceEngine](../../crates/infrastructure/src/workspace.rs) |
| 运行服务／所有者 | [tcp_slave/mod.rs](../../crates/infrastructure/src/tcp_slave/mod.rs)、[worker.rs](../../crates/infrastructure/src/tcp_slave/worker.rs) |
| Unit映射／可靠事实消费者 | [mapping.rs](../../crates/infrastructure/src/tcp_slave/mapping.rs)、[journal.rs](../../crates/infrastructure/src/tcp_slave/journal.rs) |
| 安全完整帧响应入口 | [responder.rs](../../crates/libmodbus/libmodbus-rs/src/responder.rs) |
| IPC／前端运行入口 | [commands/slave.rs](../../workbench/src-tauri/src/commands/slave.rs)、[SlaveRuntimeControls.tsx](../../workbench/src/features/workspace/SlaveRuntimeControls.tsx) |

规模探测与原生待验边界见[M04记录](../test-records/2026-10-18-m04-tcp-slave.md)。文件记录不等于SQLite配置持久化或已交付历史查看／回放。

## 1. 配置与启动的业务过程（目标视角）

保存、应用、启动均由用户分别触发，不自动串联。采集模型是一张点表，主从角色使用对应字段；设备关联可选。当前配置保存／应用已接入，TCP从站实际启动和文件录制已接入0B节，RTU及其他运行路径仍待实施；本节事件连线是业务关系，实际调用以第0节为准。配置版本管理已于2026-10-16取消，以下改用当前保存内容和独立已应用快照。

```mermaid
flowchart TD
    USER["用户"]

    USER --> CM["保存采集模型"]:::command
    CM --> MODEL["采集模型聚合<br/>点定义及校验"]:::aggregate
    MODEL --> ME["当前模型已保存"]:::event
    ME --> MV["当前保存模型"]:::view

    USER --> CC["保存通信配置<br/>角色、参数、单元及模型引用"]:::command
    MV -.引用稳定模型ID.-> CC
    CC --> CONFIG["端点／通道配置聚合<br/>TCP 从站端点、主站连接<br/>或 RTU 通道"]:::aggregate
    CONFIG --> CE["通信配置已保存"]:::event
    CE --> CV["已保存配置"]:::view

    USER --> CA["应用当前保存配置"]:::command
    CV -.选择配置.-> CA
    CA --> APPLY["应用服务<br/>检查运行状态、整组校验<br/>迁移运行值"]:::component
    APPLY --> AE["保存配置已应用"]:::event
    APPLY --> REJECT["应用被拒绝<br/>保留原配置"]:::failure
    AE --> AV["独立已应用快照"]:::view

    USER --> CS["启动仿真"]:::command
    AV -.提供启动依据.-> CS
    CS --> START["启动流程<br/>建立运行记录和起始状态"]:::component
    START --> READY["运行记录已建立"]:::event
    READY --> POLICY["记录建立成功后<br/>才允许打开通信资源"]:::policy
    POLICY --> OPEN["启动协议运行任务"]:::command
    OPEN --> TASK["协议执行器"]:::component
    TASK --> RUNNING["监听／连接已建立"]:::event
    TASK --> FAILED["启动失败"]:::failure

    RUNNING --> SESSION["运行会话<br/>维护生命周期、已应用快照<br/>和录制状态"]:::component
    SESSION --> STATUS["实例状态查询"]:::view

    classDef command fill:#2563EB,stroke:#1E40AF,color:#FFFFFF,stroke-width:2px;
    classDef aggregate fill:#FACC15,stroke:#A16207,color:#111827,stroke-width:2px;
    classDef event fill:#F97316,stroke:#C2410C,color:#111827,stroke-width:2px;
    classDef policy fill:#9333EA,stroke:#6B21A8,color:#FFFFFF,stroke-width:2px;
    classDef view fill:#16A34A,stroke:#166534,color:#FFFFFF,stroke-width:2px;
    classDef component fill:#475569,stroke:#334155,color:#FFFFFF,stroke-width:2px;
    classDef failure fill:#DC2626,stroke:#991B1B,color:#FFFFFF,stroke-width:2px;
```

## 2. 运行操作、可靠录制与停止（核心契约与目标流程）

以下以用户置数／主站读写为例。RunCoordinator目前只有受控端口测试，工作台未装配生产执行器和可靠Recorder。图中的录制、实际收发及运行查询展示属于目标流程；事件节点之间的箭头不能理解为已注册监听器。原始报文与执行结果分别记录，请求已发送不等于远端执行成功。

```mermaid
flowchart TD
    USER["用户／采集调度"]

    USER --> CMD["置数、读取或写入"]:::command
    CMD --> CHECK["命令处理器<br/>校验参数和运行状态"]:::component
    CHECK --> INTENT["录制器<br/>保存操作意图"]:::component
    INTENT --> SAVED["操作意图已可靠记录"]:::event

    SAVED --> EXEC["协议运行任务<br/>更新本地映射或执行通信"]:::component
    EXEC --> WIRE["实际收发报文记录"]:::event
    EXEC --> RESULT["操作结果<br/>成功、失败或未确认"]:::event

    WIRE --> STORE["录制器<br/>按运行记录保存事实"]:::component
    RESULT --> STORE
    STORE --> COMMITTED["结果已可靠记录"]:::event
    COMMITTED --> PROJECTION["目标：运行查询投影更新<br/>尚未接入生产流程"]:::component
    PROJECTION --> VIEW["当前点值、有效状态<br/>操作结果、报文"]:::view

    INTENT --> ERROR["录制失败"]:::failure
    STORE --> ERROR
    ERROR --> STOPRULE["完整性优先<br/>拒绝新操作并停止该运行"]:::policy

    USER --> STOPCMD["停止仿真"]:::command
    STOPRULE --> STOPCMD
    STOPCMD --> STOPPING["停止流程<br/>停止采集和调度<br/>释放该端点／通道资源"]:::component
    STOPPING --> STOPPED["运行已停止"]:::event
    STOPPING --> STOPFAIL["停止或资源释放失败"]:::failure

    STOPPED --> NOTICE["直接更新运行状态展示"]:::view
    STOPFAIL --> NOTICE
    ERROR --> NOTICE
    ERROR --> INCOMPLETE["记录不完整<br/>展示最后可靠位置<br/>已发出请求可能结果未知"]:::failure

    classDef command fill:#2563EB,stroke:#1E40AF,color:#FFFFFF,stroke-width:2px;
    classDef event fill:#F97316,stroke:#C2410C,color:#111827,stroke-width:2px;
    classDef policy fill:#9333EA,stroke:#6B21A8,color:#FFFFFF,stroke-width:2px;
    classDef view fill:#16A34A,stroke:#166534,color:#FFFFFF,stroke-width:2px;
    classDef component fill:#475569,stroke:#334155,color:#FFFFFF,stroke-width:2px;
    classDef failure fill:#DC2626,stroke:#991B1B,color:#FFFFFF,stroke-width:2px;
```

正常停止也要记录停止结果；故障停止不能等待录制恢复才执行，也不能因为最后一条停止事件写不进去而反复触发停机。故障提示必须能绕开已失败的持久化路径显示。

一条运行记录覆盖一个端点／通道及其下属单元，临时断线和重连不另开记录。外部主站请求也纳入录制，但不能在收到请求前预先记录其意图，具体收发与落盘顺序需单独细化。

## 3. 离线回放（目标流程）

ReplaySession目前仅有隔离状态重建契约与测试；生产历史读取、可用播放界面、持久化检查点和拓扑／3D联动尚未接入。目标是消费已保存的事实，不重新执行历史命令。

```mermaid
flowchart TD
    USER["用户"] --> SELECT["选择运行记录"]:::command
    SELECT --> HISTORY["历史查询<br/>起始配置、完整模型快照<br/>事件、报文、检查点"]:::component
    HISTORY --> VALIDATE["检查完整性与记录格式兼容"]:::component

    VALIDATE --> LOAD["载入起始状态<br/>或目标时间之前的检查点"]:::component
    VALIDATE --> WARN["记录不完整或不兼容<br/>明确可回放范围或拒绝打开"]:::failure

    LOAD --> REPLAY["独立回放会话"]:::component
    USER --> CONTROL["播放、暂停、倍速、跳转"]:::command
    CONTROL --> REPLAY
    REPLAY --> APPLY["按记录顺序应用状态事件<br/>不执行原始命令"]:::component
    APPLY --> STATE["指定时刻的历史状态<br/>及对应操作、报文"]:::view
    STATE --> UI["回放点表<br/>拓扑／3D按交付阶段联动"]:::view

    REPLAY -.始终遵守.-> RULE["不打开真实连接<br/>不重发控制请求<br/>不覆盖实时实例"]:::policy

    classDef command fill:#2563EB,stroke:#1E40AF,color:#FFFFFF,stroke-width:2px;
    classDef view fill:#16A34A,stroke:#166534,color:#FFFFFF,stroke-width:2px;
    classDef component fill:#475569,stroke:#334155,color:#FFFFFF,stroke-width:2px;
    classDef failure fill:#DC2626,stroke:#991B1B,color:#FFFFFF,stroke-width:2px;
    classDef policy fill:#9333EA,stroke:#6B21A8,color:#FFFFFF,stroke-width:2px;
```

## 图与实现的维护约定

新增或调整服务、命令、查询、事件消费和存储路径时，同时核对第0节时序及代码映射；行为尚未接入的部分继续标为目标／契约，不能仅因目录拆分或类型存在就改为已实现。业务范围变化还需同步实现计划和对应OpenSpec提案。历史验收记录保留原证据边界，本次不关闭M02／M03未验任务。


## M05 TCP主站实际流程（2026-10-19）

```mermaid
sequenceDiagram
    participant UI as Dockview主站面板
    participant API as Tauri适配
    participant Service as WorkspaceEngine
    participant Owner as 主站所有者
    participant Wire as 透传观测与libmodbus
    participant Remote as 外部从站
    UI->>API: MasterCommand（因果ID）
    API->>Service: execute_master
    Service->>Owner: 已应用快照／有界命令
    Owner-->>UI: busy操作投影（仅接收）
    Owner->>Wire: 单在途读写
    Wire->>Wire: 实际帧事实可靠记录
    Wire->>Remote: TX请求
    Remote-->>Wire: RX响应
    Wire-->>Owner: libmodbus确认／异常／I/O错误
    Owner->>Owner: 成功值与质量、逐点结果；写后回验
    UI->>API: MasterQuery（可见点）
    API->>Owner: 查询独立投影
    Owner-->>UI: 时间／质量／实际值／最近观测
```

事件来自真实所有者与传输路径，经同步投影和可靠文件记录消费；没有引入通用消息总线或配置事件溯源。连接命令接收、协议写入确认、回验一致与设备动作完成是不同语义。应用配置走同端点操作门，先断开并确认释放后原子替换，保持断开，不自动重发控制。


### RTU实际路径（2026-10-19）

沿用上文MasterCommand → WorkspaceEngine → 所有者 → 传输适配 → 实际从站 → MasterFact／查询。RTU适配为`rtu_bridge::RtuConnection`，独占串口由透传线程RAII持有；TX生成的实际帧先可靠记录再准备写出，RX记录实际接收、CRC及短帧，迟到窗口数据明确discarded，不进入下一次成功结果。单Unit协议错误经有界排空继续其他目标，驱动故障才关闭通道并在采集意图存在时重开。诊断走Diagnose手动命令及相同有界队列，拒绝改变通信状态的子功能。


RTU从站的实际执行链：SlaveCommand → 统一从站所有者 → 独占SerialPort → 接收与CRC／站号过滤 → 共享PDU业务校验 → libmodbus RtuResponder → RtuFrame(prepared) → 非阻塞驱动写出 → RtuFrame(driverWritten)。运行查询返回所有者真实点值，UI不直接修改映射；本地置数继续走ValuesIntent／ValuesCommitted。静默广播写也须经过权限与完整点校验；无效帧不进入映射。命令、应用／删除与串口释放使用既有端点操作门。

### M06 已接入编排与实际完成（2026-10-20）

```mermaid
sequenceDiagram
    participant UI as Dockview任务页
    participant API as 统一客户端／Tauri
    participant C as BatchCoordinator
    participant R as 固定BatchRunner
    participant S as BatchService／Task聚合
    participant P as BatchProjectionListener
    participant W as WorkspaceEngine／端点门
    participant O as 固定协议所有者
    participant D as 固定录制池
    UI->>API: 固定范围预览／明确草稿处置
    API->>C: prepare_batch
    UI->>API: 确认开始
    API->>C: start
    C->>R: submit_reserved
    R->>S: Start／Claim
    S-->>P: Started／ItemStarted
    R->>W: 申请凭证／复核本项
    W->>O: 应用释放／启动／停止
    O->>D: 提交本阶段事实
    Note over O,D: 等待期间可查询；从站继续其他端点
    D-->>O: 对应文件sync_data完成
    O->>O: 依次执行动作、结果与响应屏障
    O-->>R: 真实完成
    R->>S: Complete
    S-->>P: ItemCompleted
    UI->>API: query_batch
    API->>R: 有界只读结果
    R-->>UI: 计数／原因／耗时
    UI->>API: cancel_batch
    API->>R: 只取消未领取项
    R->>S: Cancel
    S-->>P: Cancelled
```

查询推进观察超时会生成ItemUncertain，但完成句柄、执行槽和端点凭证仍保留；不能由页面直接提交Complete。真实动作完成才结算，成功项不因别项失败／取消回滚。关闭任务Tab只停止其轮询；主站待写草稿保留则跳过，明确丢弃才处理，配置草稿不自动保存。准确状态与容量边界见[交付报告](../test-records/2026-10-20-m06-delivery.md)。
