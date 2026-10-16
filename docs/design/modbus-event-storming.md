# Modbus 模拟器事件风暴与运行流程

更新日期：2026-10-15。状态：讨论稿，尚未形成实现或验收结论。

关联：[M01 基础模型与架构边界](../plans/modbus-simulator.md#m01-基础模型与架构边界)。

本图分为业务配置与启动、执行与可靠录制、离线回放三个视角。第一张展示业务过程；后两张补充执行与存储协作，不将所有技术组件都当作业务聚合。具体事件格式、事务边界、可靠落盘、失败恢复与线程方案仍需在提案中确定。

图例：蓝色为命令，黄色为聚合，橙色为事件，紫色为自动策略，绿色为查询／展示，灰色为应用或技术组件，红色为故障。

## 1. 配置与启动的业务过程

保存、应用、启动均由用户分别触发，不自动串联。采集模型是一张点表，主从角色使用对应字段；设备关联可选，具体关系见 M01。

```mermaid
flowchart TD
    USER["用户"]

    USER --> CM["保存采集模型"]:::command
    CM --> MODEL["采集模型聚合<br/>点定义、校验、版本"]:::aggregate
    MODEL --> ME["模型版本已保存"]:::event
    ME --> MV["可用模型版本"]:::view

    USER --> CC["保存通信配置<br/>角色、参数、单元及模型引用"]:::command
    MV -.选择版本.-> CC
    CC --> CONFIG["端点／通道配置聚合<br/>TCP 从站端点、主站连接<br/>或 RTU 通道"]:::aggregate
    CONFIG --> CE["通信配置已保存"]:::event
    CE --> CV["已保存配置"]:::view

    USER --> CA["应用指定配置版本"]:::command
    CV -.选择配置.-> CA
    CA --> APPLY["应用服务<br/>检查运行状态、整组校验<br/>迁移运行值"]:::component
    APPLY --> AE["配置版本已应用"]:::event
    APPLY --> REJECT["应用被拒绝<br/>保留原配置"]:::failure
    AE --> AV["当前已应用版本"]:::view

    USER --> CS["启动仿真"]:::command
    AV -.提供启动依据.-> CS
    CS --> START["启动流程<br/>建立运行记录和起始状态"]:::component
    START --> READY["运行记录已建立"]:::event
    READY --> POLICY["记录建立成功后<br/>才允许打开通信资源"]:::policy
    POLICY --> OPEN["启动协议运行任务"]:::command
    OPEN --> TASK["协议执行器"]:::component
    TASK --> RUNNING["监听／连接已建立"]:::event
    TASK --> FAILED["启动失败"]:::failure

    RUNNING --> SESSION["运行会话<br/>维护生命周期、已应用版本<br/>和录制状态"]:::component
    SESSION --> STATUS["实例状态查询"]:::view

    classDef command fill:#2563EB,stroke:#1E40AF,color:#FFFFFF,stroke-width:2px;
    classDef aggregate fill:#FACC15,stroke:#A16207,color:#111827,stroke-width:2px;
    classDef event fill:#F97316,stroke:#C2410C,color:#111827,stroke-width:2px;
    classDef policy fill:#9333EA,stroke:#6B21A8,color:#FFFFFF,stroke-width:2px;
    classDef view fill:#16A34A,stroke:#166534,color:#FFFFFF,stroke-width:2px;
    classDef component fill:#475569,stroke:#334155,color:#FFFFFF,stroke-width:2px;
    classDef failure fill:#DC2626,stroke:#991B1B,color:#FFFFFF,stroke-width:2px;
```

## 2. 运行操作、可靠录制与停止

以下以用户置数／主站读写为例。原始报文与执行结果分别记录；请求已发送不等于远端执行成功。

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
    COMMITTED --> PROJECTION["查询投影更新"]:::component
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

## 3. 离线回放

回放消费已保存的事实，不重新执行历史命令。

```mermaid
flowchart TD
    USER["用户"] --> SELECT["选择运行记录"]:::command
    SELECT --> HISTORY["历史查询<br/>起始配置、模型版本<br/>事件、报文、检查点"]:::component
    HISTORY --> VALIDATE["检查完整性与版本兼容"]:::component

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
