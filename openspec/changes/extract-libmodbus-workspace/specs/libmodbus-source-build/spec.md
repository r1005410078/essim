## ADDED Requirements

### Requirement: 固定官方源码且离线构建

libmodbus-src MUST 携带固定的官方 libmodbus 3.2.0 源码，记录完整提交、实际验证的归档校验和、许可证及本地补丁。常规构建 SHALL 不联网、不修改宿主机安装和 vendor 源码。

#### Scenario: 离线构建相同源码
- **WHEN** 已具备规定构建工具且网络不可用时执行源码构建
- **THEN** 使用保存的同一源码完成构建，不执行 git clone、下载或系统包安装

#### Scenario: 上游归档不匹配
- **WHEN** 源码导入时计算出的 SHA-256 不符合锁定来源记录
- **THEN** 拒绝导入，不将其作为候选发布源码

### Requirement: 提供分离的静态和动态构建产物

libmodbus-src SHALL 提供构建辅助 API 与显式动态构建入口，返回头文件、库目录、动态运行文件（若有）及包含源码/补丁/target/profile/链接模式的构建清单。输出 MUST 按这些设置隔离，不把自行编译动态等同于 vendored 静态。

#### Scenario: 静态构建
- **WHEN** sys 的 vendored 路径调用 src 构建静态库
- **THEN** 返回目标平台静态库及匹配生成头文件，构建缓存不包含旧动态模式的可变状态

#### Scenario: 显式编译动态库
- **WHEN** 开发者执行动态构建入口并指定独立输出前缀
- **THEN** 生成该平台动态库、所需链接产物、头文件和清单，供 LIBMODBUS_DIR 使用

### Requirement: 工具链与目标错误可操作

构建 MUST 使用目标工具链；缺工具、配置失败、未知 target 或未支持交叉编译 SHALL 明确报错，不回退 HOST 或系统 libmodbus。

#### Scenario: 缺少原生工具
- **WHEN** 编译器或上游构建所需工具缺失
- **THEN** 输出缺失项与准备方式，且不输出可用库的成功声明

#### Scenario: 当前入口不支持交叉目标
- **WHEN** 请求的 TARGET 与 HOST 不同且对应交叉路径尚未实现
- **THEN** 明确拒绝，不把本机库复制成目标库
