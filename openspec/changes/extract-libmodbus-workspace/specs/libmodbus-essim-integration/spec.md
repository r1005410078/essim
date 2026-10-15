## ADDED Requirements

### Requirement: 根 workspace 中归类管理三个包

三个 crate SHALL 位于 crates/libmodbus/ 并归现有根 workspace 管理，共享一份锁文件，不创建嵌套 workspace。ESSim 基础设施 MUST 只通过 rs 安全 API 接入，不直接调用 sys 或保留重复 C 构建。

#### Scenario: 检查成员和依赖图
- **WHEN** 检查 Cargo 元数据及依赖关系
- **THEN** 三个包都是根成员，rs/sys/src 不依赖 ESSim 或 Tauri，基础设施没有直接 FFI 与 C 构建脚本

### Requirement: 业务策略留在 ESSim 且闭环不变

ESSim 的既有 TCP 闭环 SHALL 保留 SOC 地址100、初值650、0–1000范围、稳定ID、默认 Unit ID1、外部只读策略、帧门禁、真实报文捕获、worker/订阅/停机逻辑。静态/动态两种模式 MUST 保持既有协议异常、更新确认后读回700、断连恢复与停机<1s要求。

#### Scenario: 两种模式独立互操作
- **WHEN** 分别以 vendored 静态和自行编译动态模式运行从站
- **THEN** 固定 PyModbus 客户端均读到650，确认更新后读到700，写入拒绝和异常响应与旧基线一致

#### Scenario: 前端和无界面入口
- **WHEN** 从 Tauri/NovaSheet 或无界面示例更新并停止同一应用服务
- **THEN** 修改仍需确认生效、停止释放端口，核心无窗口依赖，原型代码不被迁移

### Requirement: 应用传播模式并携带正确动态库

基础设施和 Tauri SHALL 显式传播默认 vendored 并允许入口关闭默认 feature。动态应用 MUST 随包携带自行编译库并配置相对加载路径；静态应用 MUST 不动态依赖 libmodbus。不得依赖全局环境变量或宿主机安装路径才能运行最终包。

#### Scenario: macOS 动态包
- **WHEN** 运行经过打包验证的动态模式 app，未设置全局动态库搜索变量
- **THEN** 实际加载应用携带的库，加载路径不指向 Homebrew 或临时构建目录

#### Scenario: macOS 静态包
- **WHEN** 检查并运行默认 vendored 模式 app
- **THEN** 未动态加载 libmodbus，原有真实 TCP 闭环仍成立，系统库依赖不被误称为完全静态

### Requirement: 如实记录平台与许可交付边界

文档 SHALL 区分提案/实现、静态/动态、实际执行/未执行的平台，保留源码和 LGPL 许可，并记录两种分发方式所需材料及尚未完成的发布事项。MUST 不因上游声称跨平台而宣布本项目所有平台完成。

#### Scenario: 更新验收记录
- **WHEN** macOS 两模式验收完成但 Linux/Windows 未执行
- **THEN** 文档只将 macOS 标为已验证，并记录版本、target、工具链、报文、加载位置和其他平台待验证项

#### Scenario: 准备后续发布
- **WHEN** 准备分发静态或动态安装包
- **THEN** 根据所选模式核实源码/修改、许可和适用的替换或重新链接安排，不把开发构建成功写成分发要求已完成

### Requirement: 同一端点可动态添加单元

ESSim 的 TCP 端点 SHALL 支持在停止或监听状态新增 1–247 的 Unit ID，单元 1 默认存在。每个单元 SHALL 拥有独立 SOC 运行值与协议映射；新增映射完成后才确认成功，非法/重复 ID 不得部分应用。所有 C 对象 MUST 留在协议 worker，配置和值在本应用会话停启间保留。旧单点接口 SHALL 默认使用单元 1。

#### Scenario: 运行中新增并独立读取
- **WHEN** 从界面添加 Unit ID 2 并更新其 SOC
- **THEN** 当前 TCP 端点无须重启便可按 Unit ID 2 读回新值，Unit ID 1 保持原值

#### Scenario: 单元切换与草稿
- **WHEN** 切换两个生效值相同但草稿不同的单元
- **THEN** 每个单元保留自己的草稿，应用操作只更新选中单元，日志快照不覆盖草稿

#### Scenario: 非法或未知单元
- **WHEN** 新增非法/重复 ID，或网络请求未配置 ID
- **THEN** 新增被拒绝且集合不变；未知 ID 请求关闭连接，不访问默认单元映射

### Requirement: 工作台选择 RTU 串口从站

ESSim SHALL 通过统一业务接口支持 TCP/RTU 通信配置，RTU 首阶段为单串口、单从站；提供设备路径、波特率、校验位、数据位、停止位、站号和超时配置，停止后才允许整体应用。TCP 的动态多单元能力 MUST 保留且不自动应用于 RTU。核心无 Tauri 依赖，无界面入口复用同一应用服务。

#### Scenario: 配置与参数生效
- **WHEN** 用户停止实例后选择 RTU 并提交合法串口配置
- **THEN** 整体校验后应用配置，启动结果来自真实串口打开；非法配置不部分替换，运行期间不热改通信参数

#### Scenario: TCP 与 RTU 切换
- **WHEN** 停止后在两种通信方式间切换
- **THEN** TCP 保留 IP/端口和多 Unit ID 配置，RTU 使用独立串口与单站号配置，不混用草稿、运行值或连接数语义

### Requirement: RTU SOC 策略与可停止生命周期

RTU SHALL 复用教学 SOC 的零基地址 100、初值 650、范围 0–1000 和确认后更新语义；外部写入拒绝，广播不回复且不得修改只读 SOC。所有 C 对象及串口 I/O MUST 归协议 worker 所有，停机先结束 I/O 再释放并 join，不允许前端跨线程关闭串口。

#### Scenario: 独立客户端读取与更新
- **WHEN** 独立 RTU 客户端读取本站 SOC，应用服务确认更新后再次读取
- **THEN** 依次读回 650 和更新值，写入拒绝，其他站号无响应；保存实际原始收发及配置作为证据

#### Scenario: 异常与停机
- **WHEN** 在空闲、半帧、持续错误流量、串口断开或发送受阻时停止
- **THEN** 验证配置及驱动条件下的有界退出和资源释放；目标为已验参数/平台停机小于 1s，无法满足时明确记录并调整实现，不虚报通过

### Requirement: 虚拟串口与真实硬件分开验收

RTU 验证 SHALL 包含参数/资源测试、PTY 或虚拟串口测试、独立客户端互操作，并分别验证静态/动态模式和 TCP 回归；真实 RS-485 电气、方向控制与时序 MUST 另用硬件验证，缺硬件时保留待验状态。日志 MUST 区分实际收到的字节、发送结果和真实捕获的发送字节，不构造假 TX 报文。

#### Scenario: 仅有虚拟环境
- **WHEN** PTY 及独立客户端通过，但真实串口或 RS-485 未测
- **THEN** 仅记录软件互操作范围，硬件验收保持未完成，不宣称 Linux/Windows、总线时序或规模支持完成
