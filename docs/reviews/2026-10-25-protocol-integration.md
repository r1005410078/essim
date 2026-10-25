# 面向 IEC 104／61850 的协议接入深度审查

日期：2026-10-25。对象：[首轮接入规范](../design/protocol-integration.md)及当前Modbus／lib60870源码。性质：架构审查和规范修订，未实现接口、修改线程模型或接入61850。P1表示冻结公共接口前必须解决的设计阻碍，P2表示对应服务交付前必须明确；不将未来协议缺口说成当前Modbus已经发生的故障。

后续讨论已将统一协议通信管理补入[完整重构提案](../../openspec/changes/refactor-protocol-integration-and-communications/proposal.md)。[验收矩阵](../../openspec/changes/refactor-protocol-integration-and-communications/validation.md)逐项关联本报告13项问题；提案完成不代表这些实现缺口已关闭。

## 结论

首轮规范可以作为Modbus现状清单，尚不足以冻结跨协议接口。最主要的问题不只是缺少ProtocolId，而是仍把“端点→Unit→标量点→主从读写”作为公共领域骨架。

建议采用“设备状态／控制＋协议原生模型与绑定＋运行资源与会话”三部分。共享规则保持窄小；IEC 61850的IED模型、DataSet、RCB和控制对象不压扁为寄存器表，GOOSE／SV独立声明服务能力和时序范围。先明确边界再提取Modbus，避免抽出一个仍需为61850重做的通用执行器。

## 核实依据与范围

- 本仓库实际实现见下文逐项代码链接；`Device`目前只有id／name／kind，生产运行值仍在协议所有者内，不存在已经实现的共享设备状态服务。
- 104核对仓内固定lib60870-C v2.4.1及[官方同标签指南](https://github.com/mz-automation/lib60870/blob/v2.4.1/user_guide.adoc)。首批安全层仅总召、无时标单点／短浮点和无线程服务端，没有产品控制或延迟连接回复能力。
- 61850核对官方文档和libiec61850 **v1.6.2**的[模型头文件](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_model.h)、[服务端头文件](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_server.h)、[客户端头文件](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_client.h)。它是本轮审查参照，不表示已经选择本项目后续依赖版本、编译或完成封装。
- 官方在线API的 `latest` 页面仍标1.6.0，某些回调锁说明与v1.6.2头文件有差异。后续封装必须以选定源码版本和具体回调为准；没有用旧网页推断全部回调行为。

## P1-01：设备状态不能由多个协议执行器分别权威持有

**证据：** [Device](../../crates/core/src/domain/aggregates/device.rs)只提供关联信息；[UnitValues](../../crates/core/src/domain/values.rs)在Unit维度保存值和模型；[从站运行所有者](../../crates/infrastructure/src/tcp_slave/worker.rs)持有C Mapping。首轮规范把“运行值所有权归执行器”写成通用规则，又提出跨协议共享设备状态，两者没有闭合。

**触发与后果：** 同一PCS同时有Modbus寄存器、104遥测和61850属性。若每个执行器各有权威值，改一个协议的本地值不会可靠更新另两个；停止或重应用其中一个协议还可能错误复位设备状态。

**修订：** 区分权威设备状态、远端观测缓存和协议投影。共享设备状态由设备／仿真状态所有者提交；协议适配器只能维护映射、编码和会话缓存。无Device关联的现有Modbus实例可继续使用独立状态作用域，不强迫用户为兼容旧工程创建设备。

**验收：** 同一逻辑状态映射三种协议，本地修改或合法控制后都投影同一状态提交；停掉一个协议不清空设备值、不停另两个。来源标记不能取代写入仲裁，也不能自动把任意远端观测覆盖为仿真权威值。

## P1-02：通信实例、网络资源与协议原生对象不是同一种身份

**证据：** [EndpointConfig](../../crates/core/src/domain/aggregates/endpoint/aggregate.rs)将角色、传输和Unit集合绑定；[UnitConfig](../../crates/core/src/domain/aggregates/endpoint/unit.rs)必须有Modbus站号。首轮 `describe(protocol, role, transport)` 仍假定每个实例只有单一主从角色和一个通道。

**触发与后果：** 一个104连接可承载多个CA；61850 IED有LD／LN和多个服务；同一应用可同时作为MMS服务端和GOOSE发布者／订阅者。强制把LD当Unit、把IED当一个TCP socket，无法表达实际依赖或停止范围。官方将[MMS客户端／服务端与GOOSE／SV发布订阅](https://libiec61850.com/documentation/)列为不同API。

**修订：** 配置层区分 `ProtocolInstance`、服务profile／角色、`ChannelBinding` 和原生对象身份。现有EndpointId保留兼容，不能仅重命名就宣称分层完成。角色使用协议服务内的有限集合，不要求所有实例都选master／slave。资源停止以实际依赖闭包为准，允许多个服务共享一个模型所有者。

**验收：** 104两CA、61850一IED多LD及MMS／GOOSE组合均能在设计模型中表达；停MMS连接不被自动解释为停止所有GOOSE服务。L2网卡是共享通道，不能沿用RTU“整块网卡独占”的规则。

## P1-03：标量点＋地址不足以承载61850原生模型

**证据：** [PointDefinition](../../crates/core/src/domain/aggregates/model/point.rs)是四区标量；[CollectionModel校验及比较](../../crates/core/src/domain/aggregates/model/aggregate.rs)、[Unit覆盖](../../crates/core/src/domain/aggregates/model/overrides.rs)和[交换预览](../../crates/infrastructure/src/point_exchange.rs)都围绕该结构。

**触发与后果：** 61850对象有LD／LN／DO／DA、FC、结构与数组，数据集是有序引用集合，RCB与控制对象有独立模型。平铺成带字符串地址的点会丢失结构、FC和引用关系；反过来完全禁止字符串引用也不合理，61850对象引用本来就以规范化字符串表示。官方模型定义明确包含这些[层次、FC和集合](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_model.h)。

**修订：** 公共Signal只表达需要联动的业务状态；协议原生模型单独保存，Binding通过有类型、经过解析的引用连接它们。支持一份状态映射多个协议字段，或一组业务值映射结构对象。SCL／ICD导入、程序生成模型或受限原生模型编辑分阶段选择；不得把“支持61850”简化为增加几个点列。

**验收：** 同名引用不同FC不混淆；数组元素、数据集成员顺序、q／t及控制对象引用不丢失；未知CDC／DA类型明确报错，不能静默转浮点。协议原生模型中没有对应业务Signal的属性仍可保留并查询。

## P1-04：值修改与过程控制必须有两条业务入口

**证据：** 当前[SlaveAction](../../crates/core/src/application/commands/slave.rs)提供本地置数，[MasterAction::Write](../../crates/core/src/application/commands/master.rs)写远端值并回验；[104 Request](../../crates/lib60870/lib60870-rs/src/server.rs)只提供回调内确认／发送／终止。没有已实现的共享设备动作或控制对象状态机。

**触发与后果：** 把61850 `Operate` 当成设置 `stVal` 会绕过选择、检查、执行及实际反馈；把104控制ACT_CON当作动作完成也会提前更新设备状态。一个控制可能影响多个状态，不一定有同地址同类型的反馈点。

**修订：** `StateCommit`用于直接置数／采样提交；`DeviceCommand`用于动作请求。后者包含控制目标、来源会话、发起者、选择／执行上下文、测试标志、期限及协议关联信息。协议库处理原生控制序列，ESSim决定权限、过程检查、业务执行与状态反馈；不重写库已有的SBO状态机。61850的[官方控制教程](https://libiec61850.com/documentation/control-tutorial/)明确区分检查和执行回调。

**验收：** 覆盖直接控制、选择后执行、选择超时、跨客户端执行拒绝、取消、测试不产生实际动作、执行失败和反馈延迟；未声明支持的模型明确拒绝。两个协议同时控制同一对象时按设备控制目标仲裁，不能只依赖EndpointId操作门。

## P1-05：不能把所有上送都抽成ESSim生成一条发送事件

**证据：** 104上送由[Server::enqueue](../../crates/lib60870/lib60870-rs/src/server.rs)接受应用生成的ASDU。61850则通过更新模型API检查报告／GOOSE触发，直接修改底层MmsValue反而会漏报，见[官方数据更新API](https://support.mz-automation.de/doc/libiec61850/c/latest/group__IEC61850__SERVER__DATA__MODEL__ACCESS.html)。首轮只描述了104式的变化→组包→发送流程。

**修订：** 公共边界输出“已提交的状态批次”，适配器决定如何投影。104负责其已约定的事件策略与ASDU；61850调用模型更新API并由库根据DataSet／RCB／触发条件生成报告。ESSim不复制RCB调度、协议分段及重发引擎。

**验收：** 相同值再次采样可以更新观测时间而不必触发值变化；质量变化、重复更新和数值变化保留不同含义。数据集成员的更新原因按协议保留，不能被一个公共changed布尔值吃掉。具体dchg／qchg／dupd及合批规则由服务profile和模型确定。

队列和交付进度还必须有接收范围：104按所选冗余组／活动连接策略，61850按RCB／订阅管理。当前104封装固定单冗余组，并非10条连接各自独立消费事件；不能用一个“最后已发送序号”代表所有主站。多订阅缓冲、慢接收者和掉线策略分别计费、验收。

## P1-06：总召／报告的快照和变化流需要一致性边界

**触发与后果：** 104总召期间值变化，如果从可变表逐点读取，会发送混合时刻的快照；随后再次发送较旧的自发值会使主站回退。61850报告按数据集成员包含位和原因更新，不是每次替换整份表；分段报告、数据集变化也不能混成同一批。

**修订：** 公共状态端口提供带内部提交序号的自有快照和有序变化批次，声明订阅起点／续接、缺口与重同步。104应用明确总召切点及与自发事件的排序；61850适配器使用库的模型／报告机制，客户端映射保留RCB、成员位置、可用的confRev／分段信息。相关字段及借用生命周期见[客户端API](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_client.h)。内部序号不是用户配置版本。

**验收：** 总召中修改点，最终不会被旧值覆盖；报告未包含成员不被清零；缺段／溢出明确导致不完整或重同步，不能伪造连续历史。传输编号可回绕，不能当成全工程单调序号。

## P1-07：可靠业务审计与连续通信记录不能共享一种发送屏障

**证据：** [Modbus从站](../../crates/infrastructure/src/tcp_slave/worker.rs)可在响应准备与实际socket写出之间等待记录；首轮把可靠意图规则推广到全部外部效果。[GOOSE发布实现](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/goose/goose_publisher.c)有状态／重传序号；[SV发布实现](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/sampled_values/sv_publisher.c)有采样计数和同步信息，不能按UI点事件逐条写SQLite再发。

**修订：** 每个服务profile声明记录保证：业务控制意图可设可靠执行门禁；自发状态更新的持久化顺序单独规定；库维护报文／GOOSE重复发送／SV采样流使用独立、有界捕获策略。若要求完整无损捕获，预算不足应明确拒绝启动或停止受影响服务并标记缺口，不把采样展示当完整记录。不能为了保持实时性，擅自放宽已经承诺的可靠控制审计。

**验收：** 慢盘不悄悄拉长报文定时并仍宣称时序正确；控制审计失败不执行动作；捕获不完整有范围／计数。没有实际发出／协议确认钩子时，只报告库入队，不生成虚假Sent／Delivered。GOOSE／SV没有可通用套用的逐接收者应用确认。

## P1-08：共享模型应用需要依赖范围，不能只停当前Endpoint

**证据：** [prepare_apply](../../crates/core/src/application/queries/apply_preview.rs)按端点遍历Unit；[WorkspaceEngine](../../crates/infrastructure/src/workspace.rs)也按单端点组织真实释放及重配。当前行为适合Modbus独立实例，不能自动扩展为共享设备模型事务。

**修订：** 区分协议映射修改和共享状态schema／IED模型修改。前者尽量只影响该协议；后者预览完整受影响实例集合，按稳定顺序取得生命周期门，停止必要资源，复核依赖后发布同一有效计划。失败时保留旧应用并报告实际已停止范围；不承诺撤销已经发生的外部效果。先在提案里限定支持的原子范围，不提前建设跨进程事务。

**验收：** 删掉被三个协议绑定的Signal，预览能指出全部引用；部分停机失败不留下新旧映射混用；只改104 IOA不停止无关MMS服务。停止一个通道与重置设备状态是两种操作。

## P1-09：104与61850原生库共存的符号边界尚未验证

**证据：** lib60870官方明确说明与libiec61850共享common／HAL，提供[关闭重复公共部分的构建方式](https://support.mz-automation.de/doc/lib60870/latest/index.html)。仓内[CMake](../../crates/lib60870/lib60870-src/vendor/lib60870/lib60870-C/CMakeLists.txt)的 `BUILD_COMMON`／`BUILD_HAL` 默认开启，[当前src构建器](../../crates/lib60870/lib60870-src/src/lib.rs)未选择其他公共符号提供方式。本轮两个库尚未同链验证。

**风险与修订：** Rust不同的 `links` 名字不能隔离C全局符号。后续同进程链接前，应核对两版公共API／ABI／编译选项，选择经过验证的唯一common／HAL提供者，或明确的符号隔离方案；不能只靠链接顺序让某一库的实现意外满足另一库。共享层必须有构建／链接所有者，模式进入构建清单和缓存身份。方案另行提案，不在当前三包中直接改为依赖尚不存在的61850包。

**验收：** 同一二进制同时调用104和61850，分别覆盖静态、显式动态的符号／加载检查和并发通信；释放其中一方不破坏另一方。目标平台分别验证。此处是明确的集成风险，尚无实测证明当前构建已经发生冲突。

## P2-10：有效性、协议质量、时间和新鲜度需要分别保存

**证据：** [MasterPoint](../../crates/core/src/application/queries/master.rs)只有value／unix_ms／quality字符串；`UnitValues.stale`为整个Unit标记。[RawValue](../../crates/core/src/domain/point_table.rs)只含布尔和有限数值；首轮PointSample只给一个可选源时标。

**修订：** 样本保存本地观测时间、可选源时间及其精度／时钟质量、独立接入有效性、协议原生质量。源时间未知不补成本机时间；链路在线不代表报告已激活或已完成初次同步。标量Signal可继续复用RawValue；61850结构／枚举／字符串／位串按实际支持的原生类型保留，不强迫所有内容进入标量Signal表。时标与质量类型见[官方通用类型](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_common.h)。

**验收：** 质量变坏而值不变仍可传播；断线保留最后成功值；重复有效更新可刷新采样时间；源时钟无效、无源时标、协议无质量字段分别显示。四遥分类是视图，不决定FC、CDC或控制权限。

对时还须区分进程单调计时、宿主墙钟、设备仿真时钟和报文源时标。未来104／61850对时按声明的设备作用域处理，不能因一台模拟设备收到对时就修改宿主系统时间或其他工程时钟；操作超时和队列期限使用单调计时。当前尚未接入这些对时服务。

## P2-11：回调线程、锁和延迟回复必须逐API声明

**证据：** 本项目104 `Request<'a>`只在同步回调内借用，当前没有可保存的连接令牌；无法把整个Request送到异步业务线程稍后 `finish`。libiec61850在线1.6.0数据模型API提示回调已持锁，而v1.6.2服务端头文件为特定控制回调注明事件工作线程、未持模型锁，并禁止回调内stop；不能合成一句“所有回调都在所有者线程”或“全部回调都已持锁”。

**修订：** 封装为每种回调记录调用线程、已有锁、重入限制、返回期限、可否等待、借用寿命、延迟完成方法和停止顺序。用有界自有消息转发业务请求；同步回调不可等待只有它返回后才能推进的所有者／磁盘完成。库允许WAITING的控制阶段可分阶段推进，但不将WAITING假设推广到所有回调。以[固定v1.6.2声明](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_server.h)为本轮参照。

**验收：** 回调异常被隔离，停机后没有悬空模型／连接；延迟业务完成、客户端断开、超时与取消不重复执行。104延迟控制若纳入范围，先增加有寿命／失效检查的窄回复接口；不能暴露裸指针绕过Rust借用。

IED原生模型、节点、DataSet及值的C对象图要由安全封装明确拥有／借用关系，生命周期覆盖所有引用它的服务和回调；先停止相关使用者再释放模型。查询只返回自有快照／稳定引用标识，不能序列化或克隆C指针来恢复工程。

## P2-12：协议运行配置与工程保存配置不是同一个平面

**触发与后果：** 61850客户端可预留／启用RCB，可能创建关联级DataSet；这些会话状态若直接覆盖工程JSON，破坏“保存／显式应用”；反过来全面禁止协议revision又会丢掉标准的ConfRev。原生模型与客户端API均包含这些[控制块与协议标记](https://github.com/mz-automation/libiec61850/blob/v1.6.2/src/iec61850/inc/iec61850_client.h)。

**修订：** 工程保存设计意图、原生模型资产和允许远端配置的策略；运行层保存RCB所有者／订阅、动态集合、选择态和协议标记。需要写回工程时提供显式捕获，不自动保存。ConfRev、EntryID、stNum／sqNum是协议兼容／传输状态，允许保留，不能用“取消配置版本管理”删除；它们不提供模型历史版本或expectedRevision。

**验收：** 重连或重启对预留／动态集合／缓冲的保留范围明确；从未落盘的BRCB缓冲不能宣称进程重启后继续交付。数据集变化按ConfRev和成员表重新解析，不按旧索引错误赋值。

## P2-13：前端和资源预算仍需要模型／服务维度

**证据：** [WorkspaceClient](../../workbench/src/shared/api/contracts.ts)只分主从API；[ObservationPanel](../../workbench/src/features/workspace/ObservationPanel.tsx)按role选解析；[ResourceUsage](../../crates/core/src/domain/resources.rs)只有内存、端点、连接、队列；[工程运行投影](../../crates/infrastructure/src/project/runtime.rs)用事件字符串判断报文和结束。

**修订：** 查询按协议实例／模型节点／订阅分页，点表作为可展平投影；能力按服务profile、对象类型、实际平台及部署条件报告。资源还需计算DataSet／RCB实例、每订阅缓冲和帧率／带宽，CPU／时序必须测量，不能仅按点数推导。MMS、GOOSE、SV各自声明记录与停止状态；MMS断连不等于整个IED运行结束。

**验收：** 未封装的服务不显示可操作入口；原生模型树／属性与点表稳定身份一致；多客户端RCB、无MMS连接的发布者及跨窗口共享网卡不被误算。实际GOOSE／SV平台、权限和网卡接入需单独验证，不因TCP测试通过就标记支持。

## 修订后的最小接口方向

以下仍为设计名，不是本轮新增trait；只按阶段实现需要消费的部分。

| 边界 | 建议数据／操作 | 关键限制 |
| --- | --- | --- |
| 状态所有者 | `snapshot(scope) → StateSnapshot(cursor)`；`commit(StatePatch) → StateCommit`；`changes(scope, after) → ChangeBatch / Gap` | 设备状态与远端观测分开；提交是原子批次；有界游标缺口可重同步 |
| 设备动作 | `submit(DeviceCommand, InvocationContext) → CommandHandle`；查询／推进结果 | 控制目标与反馈Signal可不同；本地与远端来源同样保留因果身份；不让RPC取消撤销已执行动作 |
| 原生模型与映射 | `compile(profile, modelAssets, bindings) → PreparedPlan`；`impact(old, next) → ApplyImpact` | schema／服务范围、引用、结构、模型摘要及依赖齐全；不在core分配C句柄 |
| 协议运行 | `activate(plan, services) → RuntimeHandle`；`submit(ProtocolAction)`；`status`／`request_stop`／`wait_released` | 资源、会话、订阅、库回调由适配器拥有；数据传输就绪与进程运行分开 |
| 状态投影 | `project(StateCommit / Snapshot) → ProjectionReceipt` | 状态已提交不等于所有协议已发送；回执标识实际到达的阶段，失步实例明确标记 |
| 观察与记录 | 通用身份／类别／阶段信封＋协议有类型载荷；审计策略／流捕获策略分开 | 控制因果与多帧／多目标事件可关联；不要求每一帧对应一个用户Operation |

`InvocationContext`至少能携带来源协议实例／会话、方向、本地或远端发起者和关联ID；协议的选择／执行序列号保留在专属载荷，不能伪造为全局唯一用户commandId。远端控制入口不要求存在前端命令。

### 边界示例（仅示意，非已接入点表）

```text
PCS状态作用域：有功功率Signal、开关位置Signal、独立的分合闸控制目标
    ├─ Modbus绑定：area／address／编码／比例 → 寄存器投影
    ├─ 104绑定：CA／IOA／TypeID → 总召／自发ASDU
    └─ 61850原生模型绑定：对象引用＋FC → DA更新、控制DO和库报告机制
```

映射比例是各协议绑定的解释，不把一个协议的寄存器raw值直接当作所有协议的物理量。权威状态的数据类型／单位和转换失败行为必须明确，避免双重倍率和精度丢失。

## 下一步收敛与验证

1. 在公共接口提案中先确定状态作用域、协议原生模型／绑定、控制目标和服务profile；先做三种协议的纸面映射，不急于写万能RuntimeAction。
2. 用Modbus现有fixture验证原值、倍率、Unit独立性和工程格式兼容。新增共享设备状态是单独范围，不在接口命名重构中暗中改变现有值语义。
3. 104首批仍以服务端监控为可选起点，明确总召切点、变化策略、流失步和录制可观测阶段。延迟控制另列封装任务与验收，不能用当前回环Example证明。
4. 61850首阶段建议评估MMS服务端＋受限IED模型、读／报告能力；控制根据真实EMS需求单列。GOOSE、SV、Setting Groups、文件服务、安全机制分别声明范围，不因底层库有代码而默认启用。此为建议，未替用户确认最终交付范围。
5. 进入实现前加入上述每项反例作为验收条件。需要源码级试验时单建有界Example，实测回调／锁、报告及帧时序；本轮未编译libiec61850、未执行真实EMS或硬件验证。

本轮只核对代码、官方API和设计反例，更新规范并检查文档链接／差异空白；没有重跑上轮17项核心测试，也没有将这些既有测试当成104／61850架构已验证的证据。
