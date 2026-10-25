## ADDED Requirements

### Requirement: 实例服务与通道分别定义
系统 SHALL 分别表达协议实例、服务profile、通信角色、通道和原生模型身份，并按合法组合校验。已有EndpointId和UnitId MUST 保留稳定身份，不将104公共地址或61850逻辑设备强制转换成Modbus站号。

#### Scenario: 同一传输承载不同协议
- **WHEN** 计划分别声明Modbus TCP从站和104服务端
- **THEN** 系统以显式profile区分适配器并校验各自参数，不能只由TCP或slave推断协议

#### Scenario: 发布订阅不要求主从连接
- **WHEN** 契约fixture描述MMS服务端与GOOSE发布／订阅的服务组合
- **THEN** 模型能分别表达角色、通道及共享原生模型依赖，且生产能力仍因未装配而不可用

### Requirement: 公共状态与协议原生模型通过绑定关联
系统 SHALL 使用有类型Binding关联状态／控制目标与协议引用，保留协议原生层次、结构、FC及有序集合。系统 MUST 拒绝失效引用、错误类型和未支持的转换，不以通用地址字符串或任意JSON代替协议校验。

#### Scenario: 原生模型不全部展平成点
- **WHEN** 61850契约fixture包含结构DA、数组、相同名称的不同FC和DataSet成员
- **THEN** 编译计划保留引用上下文及成员顺序，未绑定业务Signal的原生节点仍可按身份查询

#### Scenario: 无效绑定在应用前拒绝
- **WHEN** Binding引用不存在Signal、越界协议地址、类型不匹配或不能表达的数值转换
- **THEN** 返回定位到实例、绑定和字段的错误，不停止旧运行、不提交部分计划

### Requirement: 纯规划与原生资源创建分离
系统 SHALL 先解析并校验定义、模型资产、依赖和资源估计，生成自包含PreparedPlan及内容摘要；纯规划 MUST 不打开通信或分配C句柄。原生资源 SHALL 在准入成功后由适配器安全所有者创建。

#### Scenario: 预览无外部副作用
- **WHEN** 用户预览应用计划或无界面调用方执行模型校验
- **THEN** 不监听端口、不打开串口、不启动库线程，不改配置或当前值

#### Scenario: 应用候选准备后依赖变化
- **WHEN** 提交应用前发现SavedDefinition或其候选模型资产、绑定与预览计划不符
- **THEN** 拒绝该候选应用并重新预览，不从编辑草稿补齐，不因此作废原已应用快照

### Requirement: 启动以自包含已应用快照为依据
系统 SHALL 区分SavedDefinition、AppliedSnapshot和RuntimeSnapshot，由管理器以内部AppliedPlanToken解析当前已应用计划及自包含资产后启动。普通保存／草稿变更 MUST 不使已应用计划失效；只有显式应用可替换该计划。调用方 MUST 不提交任意PreparedPlan绕过校验；内部代际／摘要 MUST 不提供历史选版或expectedRevision。

#### Scenario: 已保存新内容尚未应用
- **WHEN** 已应用模型为A，用户保存模型B但尚未显式应用，然后启动实例
- **THEN** 使用已应用A及其资产快照，既不拒绝合法启动，也不混入B或重读已被替换的源资产

#### Scenario: 显式应用替换计划
- **WHEN** B已成功显式应用，旧调用仍携带A的AppliedPlanToken
- **THEN** 拒绝旧凭证并要求读取当前应用，不能把A或调用方自造计划激活为当前运行

### Requirement: 旧Modbus配置与独立值兼容
系统 SHALL 将旧模型、端点、Unit和单层覆盖适配到新内部边界，保持原始值、倍率、字节／字序、权限、站号、默认参数和保存／应用语义。系统 MUST 不因DeviceId相同或复用模型自动共享运行值。

#### Scenario: 旧工程打开后保存
- **WHEN** 格式1工程只使用原有Modbus功能且被新版本打开、编辑、保存和重开
- **THEN** 保持格式1、稳定身份、配置与已应用快照，并且不自动启动通信

#### Scenario: 复用模型的两个Unit
- **WHEN** 两个旧Unit引用同一模型或关联同一DeviceId，调用方只修改其中一个运行值
- **THEN** 另一个Unit保持独立，除非用户显式创建经过校验的共享绑定

### Requirement: 新型元数据显式升级并防止旧客户端丢失
系统 SHALL 在显式保存新共享作用域／Binding／原生模型资源时以可靠事务升级至工程格式2，展示兼容性影响并保留一次迁移副本。打开／查询 MUST 不自动改写；不支持新格式的客户端 MUST 明确拒绝，不能静默忽略。格式号 MUST 不提供配置历史选版或expectedRevision。

#### Scenario: 保存新能力
- **WHEN** 用户确认保存包含新共享绑定的工程定义
- **THEN** 当前资源、引用和完整清单在同一可靠事务发布，旧格式副本保留，新工程仅包含当前定义

#### Scenario: 升级过程故障
- **WHEN** 格式升级暂存、提交决策或安装阶段故障
- **THEN** 按原工程事务规则回滚或进入恢复门禁，不发布混合格式或应用成功

#### Scenario: UI尚不能编辑原生模型
- **WHEN** 新格式包含前端尚未提供编辑器的合法原生元数据
- **THEN** UI明确只读且普通保存不丢失内容；未知或非法类型不能被静默转换成Modbus点
