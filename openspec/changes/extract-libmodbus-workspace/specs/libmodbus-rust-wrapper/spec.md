## ADDED Requirements

### Requirement: 通用安全 API 与 feature 转发

rs SHALL 提供 TCP/RTU 主站和从站所需的安全 API 子集并向 sys 转发 vendored，默认启用 vendored。MUST 不依赖 ESSim core/infrastructure、Tauri、SOC 或 UI；静态/动态 SHALL 共享相同业务调用 API。

#### Scenario: 独立 Rust 使用者
- **WHEN** 无 ESSim/Tauri 的最小示例只依赖 rs 默认配置
- **THEN** 可通过内置源码静态构建使用通用寄存器 API

#### Scenario: 关闭默认 feature
- **WHEN** 使用者关闭 rs 默认 feature 并显式选择有效动态前缀
- **THEN** sys/vendored 不被隐式重新开启，调用代码无需按链接模式改写

### Requirement: 安全封装资源和请求边界

rs MUST 以 RAII 保证 context/mapping 初始化失败与正常退出均正确释放，socket 所有权唯一且借用不能悬空。首轮 context/mapping MUST 不未经证明实现 Send/Sync；无 C 回调。安全应答接口 MUST 不将任意未校验的请求切片直接传给 C。

#### Scenario: 构造或销毁
- **WHEN** 构造在获取部分资源后失败，或对象正常 Drop
- **THEN** 已取得的资源逆序释放一次，不二次关闭 Rust 所有的 socket，不丢失原始错误

#### Scenario: 截断或伪造请求
- **WHEN** 使用者尝试向安全 reply 接口提交未校验字节
- **THEN** 类型/API 或运行校验拒绝该输入，不能导致 C 越界访问

#### Scenario: socket 与线程约束
- **WHEN** 尝试令借用 socket 早于 context 销毁，或把 context 发送至其他线程
- **THEN** 安全 API 的类型约束拒绝该用法，合法 worker 内的正常使用不受影响

### Requirement: 寄存器映射与业务语义分离

rs MUST 校验映射区间与访问边界，允许合法 UInt16 值和通用写入应答；MUST 不固定地址 100、初值 650、SOC 范围或强制全局只读。

#### Scenario: 非教学映射
- **WHEN** 使用者创建不同地址区间并设置合法 UInt16 值
- **THEN** 在无 SOC 范围约束下正常处理，越界访问返回错误

#### Scenario: 通用写入策略由上层决定
- **WHEN** 通用示例选择对合法写请求调用库正常应答
- **THEN** 库允许映射按请求更新；ESSim 可以独立选择拒绝同类写入

### Requirement: 四类数据区独立映射

安全封装 SHALL 支持线圈、离散输入、保持寄存器、输入寄存器，分别校验零基区间；位值为 bool，寄存器为 u16。输入区 SHALL 允许服务端本地更新，网络侧只读；保留 HoldingRegisters 兼容入口。

#### Scenario: 同地址与边界
- **WHEN** 四区使用相同地址并设置不同值
- **THEN** 数据互不污染，未分配或越界访问返回错误，空区间和超范围配置被拒绝

#### Scenario: 协议读写
- **WHEN** 提交合法 FC01/02/03/04/05/06/0F/10 请求
- **THEN** 读取和写入对应数据区，写入线圈与保持寄存器不改变输入区，响应字节符合预期

四区命名统一为 `holding`（保持寄存器）、`input`（输入寄存器）、`coil`（线圈）、`discrete`（离散输入）；Mapping 的本地访问方法使用这些名称及 `set_` 前缀。构造参数顺序沿用 C 映射顺序 coil、discrete、holding、input，不因显示顺序调整。上游 C API 和 `HoldingRegisters` 兼容入口保留原名。

### Requirement: RTU 串口配置与资源安全

rs SHALL 提供 RtuConfig 和 RtuContext，校验设备路径、串口参数、1–247 站号及有限超时；复用四区 Mapping。C context MUST 独占其打开的串口，正常退出及失败路径按获取状态 close 后 free 一次，保留原始错误；MUST 不复用 Rust socket 借用释放逻辑，不实现未经证明的 Send/Sync。

#### Scenario: 非法配置或打开失败
- **WHEN** 配置非法、设备不存在、权限不足或目标系统拒绝参数
- **THEN** 返回明确错误，释放已获得资源，不回退其他串口或留下可运行的半初始化对象

#### Scenario: 正常释放与部分失败
- **WHEN** 打开后初始化失败或 context 正常销毁
- **THEN** 串口及 context 各释放一次，不重复关闭，后续合法打开不受残留对象影响

### Requirement: RTU 请求与广播边界

RTU receive/reply SHALL 使用区别于 TCP 的请求类型与 ADU 长度约束，CRC/接收处理复用 libmodbus。MUST 区分被忽略、超时、无效和成功接收；safe reply 不接受任意切片。RTU 广播 MUST 不发送正常或异常响应，合法广播写入的映射修改与上层只读策略分开处理。

#### Scenario: 跨协议或截断请求
- **WHEN** 尝试将 TCP 请求交给 RTU reply，或接收到截断/CRC 无效帧
- **THEN** 类型约束或接收校验拒绝输入，不发生 C 越界访问，也不作为正常请求执行

#### Scenario: 非本站与恢复
- **WHEN** 收到非本站帧或错误帧后再收到合法本站请求
- **THEN** 非本站或错误请求不应答，合法请求可恢复处理，恢复范围由测试记录说明

#### Scenario: 广播写入与异常
- **WHEN** 通用映射接收合法广播写入，或上层拒绝广播请求
- **THEN** 前者按协议更新对应可写区，后者不修改被拒绝的数据；两者均不发送正常或异常响应


### Requirement: 同步 TCP / RTU 主站

rs SHALL 提供独立于 ESSim 的同步主站，支持主动 TCP 连接、RTU 打开、站号切换、FC01/02/03/04 四区读取和 FC05/06/0F/10 写入。MUST 校验零基地址范围及每功能数量上限，独占通道且不实现未经证明的 Send/Sync，不自动重试控制写入。

#### Scenario: 主动读写独立从站
- **WHEN** 主站连接配置一致的独立 TCP 或 PTY RTU 从站
- **THEN** 读取四区原始值并确认单/多写，输入区保持独立，合法异常保留 errno 并允许后续请求

#### Scenario: 超时或错误确认
- **WHEN** 收发超时、断连或上游检测到事务号/功能/CRC 等无效响应
- **THEN** 返回原始错误并释放通道，后续请求返回 NotConnected；调用方须明确重新连接，不将写入错误解释为未执行

#### Scenario: 非法范围
- **WHEN** 数量为空、超过功能上限或末地址大于 65535
- **THEN** 发送前返回 InvalidInput，既有连接仍可使用

#### Scenario: 验收与能力边界
- **WHEN** 报告主站支持状态
- **THEN** 区分库/工作台、PTY/硬件，声明上游 TCP Unit ID/MBAP 长度校验不足及非墙钟截止时间的等待语义，不将普通互操作称为完整兼容性证明
