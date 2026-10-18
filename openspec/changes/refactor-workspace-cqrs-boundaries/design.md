## Context

配置服务当前同时定义Catalog、命令、事件、查询投影、仓储及停止端口，并在单个match中处理全部操作。已有M03分段释放规划和并发测试，不能为拆文件而退回持锁等待。CQRS只要求读写职责分离，不要求独立数据库；DDD要求局部不变量归领域，不要求每个类型各建一个crate。

## Goals / Non-Goals

**Goals:** 可从目录定位命令、处理器、查询、提交事实及领域规则；保持现有业务行为、IPC、原子性与测试证据；增加可验证的具体提交事实。

**Non-Goals:** 事件溯源、通用消息总线／分发框架、插件、中间件、持久化、配置版本、真实I/O、前端功能重构、关闭M02／M03遗留验收。

## Decisions

### 1. 应用层按职责和用例族拆分

目标结构：

```text
application/
  commands/
    mod.rs             # Command、Action、Receipt
    model.rs           # 模型保存／复制／删除处理
    endpoint.rs        # 端点创建／保存／复制及引用协调
    navigation.rs      # 导航命令处理
    lifecycle.rs       # 应用／删除／复位，保留释放规划
  queries/             # view、capabilities、projection、apply_preview、inspection
  events/              # configuration事实、config_event信封、history契约
  event_handlers/      # workspace_projection同步消费者
  ports.rs             # ConfigRepository、ApplyBoundary、状态及StoppedBoundary
  catalog.rs           # 跨聚合会话目录及引用索引
  workspace.rs         # 薄兼容门面，只重导出
  service.rs           # 权威加载、分派、提交、序号、投影及释放协调
  runtime.rs           # 现有运行契约，不冒充生产执行器
  resource_release.rs  # 现有请求／确认适配
```

保留已有Command+Action序列化结构，不引入通用CommandBus、类型擦除或每个CRUD一个trait。处理器按用例族分组，返回操作结果与具体事实；服务只负责事务边界，避免只把大match搬到另一个大文件。查询读Catalog／快照，不依赖写处理器或调用commit。原子应用预览是无副作用应用规划，可由查询与命令共用，不重复算法。

### 2. 领域局部不变量与跨聚合协调分开

`domain/`拆出identity、error、model、endpoint、values、navigation，保留point_table编码模块。`foundation`仅保留重导出门面，正式代码使用明确模块路径。端点类型固定由EndpointConfig的方法检查；文件夹目标、循环、非空删除由Navigation方法检查；模型引用和设备存在等跨聚合规则由应用处理器协调。Catalog仍是当前会话配置存储边界，不谎称所有对象构成一个DDD聚合。

### 3. 具体配置事实与查询快照分开

`ConfigurationFact`枚举表达已成功提交的业务动作，至少涵盖模型保存／复制／删除、端点创建／保存／复制／删除／应用、Unit值复位、文件夹保存／删除、端点移动、设备创建。携带稳定身份；复制携带源与目标，复位携带端点与Unit，移动携带新父目录。

`ConfigEvent`是带schema、sequence、command_id的提交事件信封，新增fact，同时保留catalog快照供现有Projection恢复。事实在处理器校验通过后准备，只有仓储commit成功后才作为成功结果返回并推进投影；验证、停止、并发检查或commit失败均不返回成功事件，不推进配置通知序号。它是提交事实，不代表UI点击，不表示真实通信，也不能用于完整状态重放。

不建事件总线：核心execute显式返回Receipt和ConfigEvent；基础设施提供带事件的调用入口，原有execute继续返回Receipt，Tauri及前端订阅继续使用完整WorkspaceView。没有可靠投递或订阅者确认承诺；未来可靠历史由M07／M08细化。考虑过只拆文件保留笼统快照事件，但无法表达业务事实；也考虑过全面事件溯源，超出当前需求且增加复杂性。

### 4. 保持兼容并验证边界

仓内正式调用迁到commands／queries／ports／service及领域模块。旧Rust路径仅作为短重导出入口保留，避免无意义的大范围测试／示例破坏；不保留两套实现。前端Action JSON、Tauri命令名及点表交换格式不变。新增内部事件fact字段不改变用户配置，现有会话无持久化事件需要迁移。

测试先建立原基线，再验证事实对应、失败无成功事件及序号不推进，保留迁移精度、重复ID、引用保护、存储失败、释放等待并发和UI DTO测试。增加小型模块边界检查，禁止领域依赖应用／基础设施／Tauri；门面无处理逻辑。

## Risks / Trade-offs

- [拆分引入循环依赖或职责倒置] → 领域只依赖领域；Catalog引用领域；查询与处理器通过窄纯函数共享规划；服务依赖处理器和端口。
- [事实描述与实际提交不一致] → 测试创建／复制／删除／应用／复位等事实身份和失败无事件；事实只在commit成功后可观察。
- [把配置快照仍称为事件溯源] → 文档明示快照查询、配置事实、运行历史三种概念；不提供配置事件回放能力。
- [停止与提交边界回退] → 原M03并发和失败仓储测试原样保留，服务不在全局目录锁内等待资源。
- [为目录整齐增加抽象] → 只增加模块与具体处理函数，不增加依赖、框架或业务能力。

## Migration Plan

先记录测试基线和现有改动；完成领域模块及应用职责拆分，再增加事实与失败用例，迁移装配层导入并更新架构入口。原API门面只重导出；现有会话、协议库和点表格式不迁移。按实际改动运行核心／基础设施／文档测试、前端DTO测试和桌面构建；全workspace若仍缺autoconf如实记录。仅用户要求时提交。

## Open Questions

无阻塞的产品问题。代码级模块名可根据实际依赖调整，但不得以此扩大为事件溯源、生产执行器或持久化项目。

## 2026-10-18 目录细分

领域聚合移入`domain/aggregates/`，端点按aggregate／unit／transport、模型按aggregate／point／reference拆分；设备仍是可选实体，导航保持独立边界，不把Catalog整体称作聚合。保留原领域模块路径的重导出，维持调用及JSON兼容。

事件分为配置事实、配置信封及运行历史契约；查询按结果、能力、投影、预览、差异分文件。现有配置事件消费逻辑移入`event_handlers/workspace_projection.rs`，服务提交后显式同步调用；`Projection::accept`只作为兼容委托入口。没有新增动态注册、按fact路由、异步调度或可靠投递。运行协调器保留原HistoryEvent／Fact导出，生产能力边界不变。最新目录与时序见[架构说明](../../../docs/architecture/workspace.md)和[流程图](../../../docs/design/modbus-event-storming.md)。
