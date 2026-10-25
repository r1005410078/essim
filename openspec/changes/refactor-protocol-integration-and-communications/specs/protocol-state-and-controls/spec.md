## ADDED Requirements

### Requirement: 状态观测与协议投影分别拥有
系统 SHALL 由项目StateRegistry拥有StateScope及其状态，独立于协议RunId和连接生命周期；远端采集默认进入独立观测缓存，协议映射维护编码投影。共享 MUST 通过显式Binding建立，适配器 MUST 不各自权威保存同一设备状态；最后一个协议停止 MUST 不销毁或复位共享状态。

#### Scenario: 同一设备多个协议投影
- **WHEN** 无界面fixture显式把Modbus和104绑定到同一Signal并提交一次新值
- **THEN** 两适配器观察同一提交身份并生成各自投影，停止任意一个不清空权威状态

#### Scenario: 观测不能隐式覆盖仿真值
- **WHEN** 客户端收到远端采样而没有声明状态来源绑定
- **THEN** 只更新观测缓存，不能覆盖本地仿真设备状态

#### Scenario: 无协议运行时修改状态
- **WHEN** 所有协议都已停止，用户提交合法共享状态补丁
- **THEN** 项目状态所有者仍按共享提交规则处理，不能要求借用已结束协议的RunId或Journal；重新启动协议读取该状态

### Requirement: 状态提交整批校验且精度明确
系统 SHALL 对状态批次的身份、类型、权限和转换完整校验后原子提交，保留值／质量／更新时间和来源。Signal类型及Binding转换 MUST 明确；不支持精度或协议类型时 SHALL 拒绝，不能通过前端Number或默默舍入改变语义。

#### Scenario: 批次中一个值非法
- **WHEN** 本地补丁包含未知Signal、重复身份、不可表达值或权限冲突
- **THEN** 全批不提交，返回可定位错误，不发布部分变化

#### Scenario: 权威值已提交但某投影失败
- **WHEN** 显式共享批次提交成功而某协议投影拒绝或失步
- **THEN** 权威提交保留，故障实例明确标记失败／滞后，不生成虚假协议成功，不宣称跨协议交付原子完成

### Requirement: 异步提交具有唯一可靠决策与发布点
系统 SHALL 以submit_patch返回可查询CommitOperationId，区分Accepted、Pending、Committed、Rejected及Unknown。DeviceSignalScope SHALL 在唯一ScopeJournal可靠确认意图与完整提交决策后，原子发布预备值和游标，才返回StateCommit及变化；协议Journal仅记录本协议阶段并关联CommitId，不重复决定业务提交。确认前 MUST 不投影或发送新值。LegacyRawScope SHALL 保留原所有者、Modbus可靠屏障及结果未知语义，不叠加共享提交Journal。

#### Scenario: 意图或决策确定失败
- **WHEN** 共享补丁整批校验通过，但意图或提交决策确定未可靠写入
- **THEN** 保留旧可见值且不投影，报告失败；记录故障阻止该scope后续写入，不能将Accepted当作已提交

#### Scenario: 决策确认丢失
- **WHEN** 提交决策可能已落盘但确认丢失
- **THEN** 操作保持Unknown、冻结冲突写入和迁移，保留最后确认视图，不自动重发或发布新投影；查明可靠决策后才能恢复

#### Scenario: 意图记录在受理前因队列额度不足被拒绝
- **WHEN** RecordingPool因临时队列额度不足明确未受理CommitIntent，之后额度释放
- **THEN** 返回JournalRejected，不创建已受理提交、不改变值／游标／投影，也不消耗连续记录序号；调用方后续显式新提交可正常接续意图与决策记录，不自动重发被拒请求。此规则不适用于已受理后的写入／同步故障或未知确认，后者仍按故障隔离规则处理

#### Scenario: 决策同步报错但数据可能已落盘
- **WHEN** 提交决策的sync_data或完成确认报错，不能证明决策尚未写入
- **THEN** 按Unknown隔离，不直接判定Rejected或重试；只有明确在接纳／写入之前拒绝才能判未提交

#### Scenario: 决策可靠后发布前进程中断
- **WHEN** 可靠提交决策已写入，但内存发布前进程中断
- **THEN** 恢复证据保留该决策与未完成阶段，不能误报为未发生或网络已投影，不能自动重放控制／恢复通信

### Requirement: 快照与变化流有可验证切点
系统 SHALL 提供同作用域的一致快照、带所有者epoch的内部单调游标和有界变化批次。每个完整ChangeBatch SHALL 声明after／through范围，一次业务提交产生的多帧 SHALL 在同一批次内准入；after MUST 等于最后接纳位置，有进展时through大于after；当前epoch／位置的空轮询（through等于after且无帧）SHALL 作为无变化处理，不触发Gap。epoch不符、范围重叠／乱序／断档或快照切点落后于已接纳位置 MUST 返回Gap并阻止继续投影，不能只比较总召切点。内部游标 MUST 不提供用户配置版本或expectedRevision。

#### Scenario: 总召期间存在发送积压
- **WHEN** 104监控测试profile在发送窗口堵塞且已有旧自发帧时受理总召，切点后又有更新
- **THEN** 经G0验证的每会话单一应用FIFO／同一原生发送路径使已接纳旧帧排在总召批次前、切点后变化排在总召批次后；切点前迟到投影按序归入前段或明确标为被快照替代，不能后插；C队列满时保留队首，不绕用低优先级enqueue，不以StateCursor单独宣称发送有序

#### Scenario: 断线后的当前状态重同步
- **WHEN** 当前状态监控存在应用／原生积压时断线、STOPDT后重启、重复STARTDT或主站接管
- **THEN** 在每次tick后通过会话有效性查询发现代际变化，废弃旧应用积压并记录缺口，捕获当前权威快照后再接续新变化；旧会话的异步结果拒绝，不推断C缓冲已清或精确损失数，不宣称SOE／离线无损

#### Scenario: 总召切点之后仍有旧变化迟到
- **WHEN** 切点为10，接纳完整批次10到12后，旧批次10到11到达
- **THEN** 返回Gap且不接纳旧值，暂停投影并重新取得当前快照，不能把接收端从cursor12回退到11

#### Scenario: 旧快照晚于新变化到达
- **WHEN** 已接纳cursor11后，收到cut10的异步快照
- **THEN** 拒绝旧快照并进入失步，不能将其排在新值之后；重新同步须重新捕获权威快照，不能复用旧回复

#### Scenario: 一个提交生成多帧或批次超限
- **WHEN** 同一游标批次包含多条ASDU，而剩余应用预算不足以接纳整批
- **THEN** 全批拒绝且接纳游标不推进，不能先收部分帧再把其他同游标帧当作重复更新丢弃

#### Scenario: Gap后旧快照迟到且连续重同步失败
- **WHEN** 同一状态所有者epoch已接纳到cursor13（包含尚未发送的帧），进入Gap后收到cut10／cut12旧快照，期间可能重置传输会话
- **THEN** 高水位独立于待发队列保留，失败重试不能清除；旧cut及其他所有者epoch的回复继续返回ProjectionGap，暂停接纳变化／发送，直到重新捕获同epoch且cut不小于13的一致快照后才接续变化。更换传输会话不重置状态进度，切换状态所有者须显式重新绑定，不能靠任意快照切换epoch

#### Scenario: 变化日志超过保留范围
- **WHEN** 消费者使用过旧游标继续读取
- **THEN** 明确返回Gap并要求重同步，不能静默跳过后宣称连续交付

#### Scenario: 工程重开后的旧游标
- **WHEN** 状态所有者重建后消费者使用上一epoch的游标
- **THEN** 明确拒绝续接并要求新快照，不能因数值序号碰巧相同混合两次状态流

### Requirement: 保留更新原因与接收范围
系统 SHALL 分别表达值变化、质量变化和同值再更新，并按冗余组／RCB／订阅维护接纳及恢复位置。公共层 MUST 不复制原生库已有报告引擎，也不能把单个发送序号当所有接收者的进度。

#### Scenario: 仅质量变化
- **WHEN** 数值不变而质量变坏
- **THEN** 状态提交保留质量变化原因，由协议profile决定对应上送／报告行为，不能被数值去重丢弃

#### Scenario: 部分数据集报告
- **WHEN** 61850契约fixture报告仅包含部分成员且带有原因／模型标记
- **THEN** 仅更新包含成员，未包含成员不清零，数据集顺序／FC／版本标记不混用

### Requirement: 设备动作与直接置数分开
系统 SHALL 提供独立DeviceCommand及InvocationContext，保留控制目标、来源协议／会话、发起者、测试／检查、期限及关联身份。设备许可、过程检查和反馈 SHALL 与原生协议选择／执行状态机分工；未支持动作 MUST 拒绝。

#### Scenario: 远端控制没有前端命令
- **WHEN** 适配器收到合法远端控制上下文
- **THEN** 可关联服务端生成的操作身份和来源会话，不要求伪造用户commandId，不直接绕过动作处理器设置反馈点

#### Scenario: 测试或仅选择
- **WHEN** 契约动作处理器收到测试命令或选择阶段请求
- **THEN** 不产生实际设备动作或提前改变反馈状态，执行结果和实际反馈分别报告

#### Scenario: 跨协议控制冲突
- **WHEN** 两个协议会话同时操作同一控制目标
- **THEN** 按目标仲裁拒绝或有界排序，不能因EndpointId不同而绕过占用；超时／取消不重复执行

### Requirement: 应用影响覆盖共享依赖
系统 SHALL 计算ApplyImpact并在全部预校验、预算及生命周期门准备后封闭受影响StateScope全部写入口，再停止必要资源并排空已受理操作。系统 SHALL 在排空后取得最终快照／游标，复核候选定义，再原子发布新schema、迁移值、AppliedSnapshot与新epoch。系统 MUST 保留停机事实，不承诺撤销已发生外部效果。

#### Scenario: 共享schema删除被引用Signal
- **WHEN** 三个协议Binding引用的Signal被申请删除
- **THEN** 预览列出全部引用及需要处理的实例，未解决引用不能提交，不能只检查当前选中Unit

#### Scenario: 纯协议映射修改
- **WHEN** 只修改104 IOA且不改变共享状态或其他模型依赖
- **THEN** 停机范围限于所需104实例，不停止无关Modbus／MMS服务或重置设备状态

#### Scenario: 部分释放或存储失败
- **WHEN** 依赖集合中某实例释放失败或停机后持久提交失败
- **THEN** 不发布半份新计划，保留旧应用和实际已停止范围，不自动重启

### Requirement: ScopeFence覆盖全部写入且未知操作阻塞迁移
系统 SHALL 由StateRegistry提供begin_quiesce、quiesce_status、final_snapshot、publish_migration及abort_quiesce边界。ScopeFence SHALL 绑定scope、schema epoch及本轮独立quiesce身份，只允许当前活动fence查询、取消或发布；abort／发布后 MUST 失效，不能只靠schema epoch区分多轮应用。ScopeFence MUST 封闭本地补丁、远端写入、绑定采样及DeviceCommand带来的新写入；迁移须等待已受理提交／动作／投影达到确定安全边界，不能仅以协议已停作为排空依据。旧epoch句柄 MUST 拒绝迟到写入。

#### Scenario: 预览后仍有写入
- **WHEN** 应用预览后、ScopeFence生效前有已受理补丁或设备动作完成
- **THEN** 迁移基于排空后的最终快照包含其结果，不能使用预览时旧值覆盖；Fence之后的新写入明确返回ScopeQuiescing

#### Scenario: 未知提交不能强行解围
- **WHEN** 提交／动作或配置持久决策未知，无法确定迁移是否安全
- **THEN** scope保持Quiescing或RecoveryRequired，允许查询／停止与受控恢复但不迁移、不强行解锁写入；确定未提交失败才可恢复旧schema准入，协议保持真实停止状态

#### Scenario: 新epoch发布后迟到回调
- **WHEN** 新schema和值已原子发布，旧适配器或异步回调尝试使用旧epoch写入
- **THEN** 拒绝旧句柄且不修改新状态，重新获取有效绑定后才可提交

#### Scenario: 旧Fence不能操作下一轮quiesce
- **WHEN** f1被abort后，schema epoch未变且已创建f2，迟到调用仍携带f1
- **THEN** 最终快照、取消和发布均返回StaleFence，f2保持关闭写入；f2完成后其重复调用也不能重新生效
