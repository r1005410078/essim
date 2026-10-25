## ADDED Requirements

### Requirement: 通用样本保留来源质量及时间
系统 SHALL 分别保留值、接入有效性、本地观测时间、可选源时标及精度／质量、来源和协议原生质量。系统 MUST 不以零值替代失败结果，不以本机时间冒充未知源时标，不将设备对时应用到宿主系统时钟。

#### Scenario: 断连后的最后成功值
- **WHEN** 通信失败或订阅失步
- **THEN** 最近成功值及时间保留，有效性单独变化，链路在线不自动表示数据新鲜

#### Scenario: 源时钟未知
- **WHEN** 原生样本不含源时标或声明源时钟无效
- **THEN** 保存真实缺失／质量信息，与本机接收时间分别展示

### Requirement: 事件信封与阶段证据明确
系统 SHALL 使用带工程、所属记录流身份、顺序、时间、类别、阶段及可选因果／会话身份的信封，协议专属信息使用有类型载荷。协议流 SHALL 关联实例／运行，ScopeJournal SHALL 关联scope／scope session且不要求协议RunId。系统 MUST 仅发布实际可观测阶段，不把入队、写出、协议确认、命令终止和设备反馈合成一个成功。

#### Scenario: 只有库接纳证据
- **WHEN** 当前104安全封装成功接受ASDU但无实际写出／确认钩子
- **THEN** 只记录库接纳阶段，Sent和Delivered不能被推测生成

#### Scenario: 多对多通信关联
- **WHEN** 一次状态提交生成多帧或一个报告包含多个Signal
- **THEN** 信封保留真实关联，允许不存在用户Operation，不为每帧伪造控制操作

#### Scenario: 状态已提交但源协议尚未完成
- **WHEN** ScopeJournal确认业务提交并发布StateCommit，源协议投影或回复仍待处理
- **THEN** 状态提交与各Binding的ProjectionReceipt分别记录，不能以StateCommitted生成该协议操作成功、Sent或Acked

#### Scenario: 会话重置造成投影缺口
- **WHEN** 已有库接纳的ASDU和未发应用积压，而会话代际变化
- **THEN** 分别记录应用积压被替代及协议阶段未知，不能把应用丢弃数当作C缓冲损失数；快照恢复不补造Sent／Acked或历史连续交付

### Requirement: 记录保证按服务声明
系统 SHALL 分别声明Modbus兼容可靠记录、控制审计、状态提交记录和连续流捕获策略，保持现有Modbus保证。需要可靠控制审计的服务 MUST 在可靠意图失败时阻止动作；连续流 MUST 不被强制逐帧sync_data／SQLite后才发送。

#### Scenario: 可靠控制结果未知
- **WHEN** 外部动作发生后结果记录失败
- **THEN** 保留最后可靠位置和未知范围，停止受影响可靠运行，不自动重发或降级为无审计控制

#### Scenario: 流捕获预算不足
- **WHEN** 声明完整捕获的服务无法满足队列／磁盘预算
- **THEN** 按其承诺拒绝启动或停止受影响服务并标记缺口，不用采样视图冒充完整记录

#### Scenario: 仅查询投影故障
- **WHEN** SQLite投影或可丢诊断日志故障而可靠文件仍正常
- **THEN** 按现有独立故障规则报告，不由新管理器擅自改变Modbus通信保证

### Requirement: 工程记录按语义类别消费
系统 SHALL 按事件类别识别报文、按真实运行释放事实结束记录，并保留旧记录读取兼容。系统 MUST 不将MMS断连或陌生事件字符串当整个实例结束，也不能批量重写既有历史文件。

#### Scenario: 新协议报文进入工程
- **WHEN** 统一信封中出现104或契约原生模型的通信／状态事件
- **THEN** 工程投影按类别处理并保留来源，不能按Modbus事实名误分类

### Requirement: 共享审计归作用域且不重复业务决策
DeviceSignalScope SHALL 使用独立于协议RunId的ScopeJournal，记录自包含应用schema、scope会话、初值、意图和完整提交决策；各协议Journal SHALL 只关联CommitId并记录本协议事实。可靠ScopeJournal故障或决策未知 MUST 隔离受影响scope及其依赖写入，不扩散为整个工程已失败，也不以事后日志补造成功。LegacyRawScope MUST 保持原Modbus可靠前缀及保证，不额外叠加一次业务决策。

#### Scenario: 两个投影只决定一次提交
- **WHEN** 无协议运行或Modbus和104同时运行时提交同一共享补丁
- **THEN** 都由同一scope提交链确认一次，协议运行时各自关联该CommitId；停止其中一方不关闭仍被状态所有者使用的Journal

#### Scenario: 单一作用域可靠记录故障
- **WHEN** 一个共享scope的可靠记录失效而另一无依赖scope正常
- **THEN** 保留故障scope最后可靠位置与未知范围，阻止相关写入／新投影，另一scope不被误停；既有Modbus兼容运行仍遵守原独立故障规则

### Requirement: 前端统一客户端与能力门禁
业务组件 SHALL 通过统一通信客户端读取运行、会话、操作和观测，Tauri调用 MUST 限制在适配层。前端 SHALL 按实际能力呈现操作，复用既有公共工作台组件，原生模型节点与Signal点表分别按稳定身份有界读取。

#### Scenario: 可见范围运行查询
- **WHEN** 用户滚动运行点表或查看原生模型节点页
- **THEN** 只请求有界所需数据，不复制整个Catalog，不等待真实网络读取，保留现有值草稿

#### Scenario: 生产未启用测试profile
- **WHEN** 测试构建验证过104／61850契约但正式客户端未装配它们
- **THEN** 工作台不出现可成功启动入口，禁用原因准确，图标不作为支持证据
