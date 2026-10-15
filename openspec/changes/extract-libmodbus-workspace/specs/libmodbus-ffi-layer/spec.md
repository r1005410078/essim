## ADDED Requirements

### Requirement: vendored 仅表示内置源码静态链接

sys SHALL 通过 vendored feature 启用可选的 libmodbus-src 构建依赖并静态链接；不启用时 SHALL 仅接受显式 LIBMODBUS_DIR 指向的自行编译动态产物。MUST 不探测或回退 Homebrew/pkg-config/vcpkg。

#### Scenario: 开启 vendored
- **WHEN** 依赖图启用了 sys/vendored，且无冲突动态目录
- **THEN** 编译固定内置源码并静态链接，不要求已安装 libmodbus

#### Scenario: 选择动态路径
- **WHEN** vendored 未开启且 LIBMODBUS_DIR 指向符合约定的自行编译动态前缀
- **THEN** 使用该前缀的库和头文件，输出其动态链接与运行产物元数据

#### Scenario: 缺目录或双重选择
- **WHEN** 非 vendored 未提供目录，或 vendored 与显式动态目录同时配置
- **THEN** 构建失败并说明修复方式，不静默决定来源

### Requirement: 验证原生来源和 ABI

sys MUST 校验显式前缀中的头文件、库、构建清单、版本和目标匹配；头文件绑定以固定官方 API 为依据，C 对象 SHALL 使用 opaque 类型或经过验证的布局。必要的 C 辅助函数 MUST 只处理通用底层操作。

#### Scenario: 动态产物不匹配
- **WHEN** 前缀缺文件、清单不符、版本不是 3.2.0 或架构错误
- **THEN** 拒绝链接或在对应原生验证步骤明确失败，且不尝试另一份宿主库

#### Scenario: 核查绑定
- **WHEN** 构建当前 API 子集的 C 编译/链接探针
- **THEN** 函数签名、缓冲区上限和辅助访问与同一份生成头文件一致，教学地址/初值不出现在辅助函数中

### Requirement: 唯一链接所有者与原始契约

sys SHALL 声明 links=modbus，集中输出链接和下游构建元数据，并为 unsafe API 说明指针、缓冲、socket、线程与释放前提。MUST 不负责设备策略或最终应用打包。

#### Scenario: 下游链接
- **WHEN** rs 和 ESSim 通过 sys 使用原生库
- **THEN** 原生链接由一个 sys 实例统一声明，下游可取得正确的头文件、模式及动态产物信息

#### Scenario: 迁移旧变量
- **WHEN** 构建仅设置已废弃的 LIBMODBUS_PREFIX
- **THEN** 给出新 LIBMODBUS_DIR 或 vendored 配置的迁移诊断，不悄悄维持旧 Homebrew 自动搜索

### Requirement: 四区本地访问辅助函数

C 辅助访问 SHALL 按明确数据区选择对应数组、校验地址和指针，位值限定 0/1；MUST 不改变上游源码或让 Rust 依赖 C 映射布局。

#### Scenario: 非法本地访问
- **WHEN** 数据区编号、位值或地址无效
- **THEN** 返回 -1 并设置 errno，不访问区间外内存

### Requirement: RTU 原始 API 与串口所有权契约

sys SHALL 按固定 libmodbus 3.2.0 头文件声明 modbus_new_rtu、modbus_connect、modbus_set_slave、modbus_close 及 RTU ADU 常量，并在静态/动态模式验证签名及链接。每个 unsafe API MUST 写明指针、C 字符串、串行访问、errno 和资源创建/释放责任，不改变已有链接来源或平台限制。

#### Scenario: RTU ABI 探针
- **WHEN** 编译和链接 RTU API 探针
- **THEN** 签名、字符参数及 RTU 最大 ADU 与同一份固定头文件一致，缺符号或不匹配明确失败

#### Scenario: 串口创建释放配对
- **WHEN** 调用者查阅 RTU 原始绑定
- **THEN** 契约明确 new/free、connect/close 配对，以及打开失败、已打开和关闭后各状态的使用条件，禁止多个所有者重复关闭串口


### Requirement: 主站读写 ABI

sys SHALL 声明固定 3.2.0 的四个读函数与线圈/保持寄存器单写、多写函数，以官方头文件 C 编译链接探针验证签名。每个 unsafe 声明 MUST 注明独占有效 context、零基范围、缓冲长度/对齐/生命周期、返回数量及 errno，不转移资源所有权。

#### Scenario: 两种模式绑定
- **WHEN** 主站在 vendored 静态或显式动态模式构建
- **THEN** 使用相同签名和调用 API；不自动改用宿主库或修改第三方源码
