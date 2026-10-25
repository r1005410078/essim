//! lib60870-C 固定提交的原始 FFI 子集；不提供线程或生命周期安全保证。
#![allow(non_snake_case)]
use std::ffi::{c_char, c_int, c_void};
/// 不透明上游 C 对象；仅使用指针，禁止从 Rust 解引用其内部布局。
#[repr(C)]
pub struct Opaque {
    _private: [u8; 0],
}
/// ASDU 所有者或借用指针，所有权由具体函数约定决定。
pub type Asdu = *mut Opaque;
/// 104 服务端句柄。
pub type Slave = *mut Opaque;
/// 回调内借用的主站连接。
pub type Connection = *mut Opaque;
/// 信息对象句柄。
pub type InformationObject = *mut Opaque;
/// 上游应用层参数布局；必须与固定头文件一致。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AppParameters {
    /// TypeID 字节数，固定为 1。
    pub type_id: c_int,
    /// VSQ 字节数，固定为 1。
    pub vsq: c_int,
    /// COT 字节数。
    pub cot: c_int,
    /// 起源地址。
    pub originator: c_int,
    /// 公共地址字节数。
    pub ca: c_int,
    /// 信息体地址字节数。
    pub ioa: c_int,
    /// ASDU 最大字节数。
    pub max_asdu: c_int,
}
/// 上游运行时报告的版本，可能与来源标签不同。
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    /// 主版本。
    pub major: c_int,
    /// 次版本。
    pub minor: c_int,
    /// 修订版本。
    pub patch: c_int,
}
/// C 回调；返回 true 表示报文已处理。参数与 ASDU/连接均为回调期间借用。
pub type AsduHandler = Option<unsafe extern "C" fn(*mut c_void, Connection, Asdu) -> bool>;
/// 上游连接事件回调；event 为固定头文件的 CS104_PeerConnectionEvent。
pub type ConnectionEventHandler = Option<unsafe extern "C" fn(*mut c_void, Connection, c_int)>;
/// 原始完整APDU通知；msg仅在回调内借用，sent区分收发。
pub type RawMessageHandler =
    Option<unsafe extern "C" fn(*mut c_void, Connection, *mut u8, c_int, bool)>;
unsafe extern "C" {
    /// 注册原始报文通知；不获取cookie所有权。
    /// # Safety
    /// slave未启动，cookie存活到stop/destroy返回；msg仅回调内读取，不得unwind或重入。
    pub fn CS104_Slave_setRawMessageHandler(
        slave: Slave,
        handler: RawMessageHandler,
        parameter: *mut c_void,
    );
    /// 注册连接／激活事件通知，不获取 parameter 所有权。
    /// # Safety
    /// slave 未启动，cookie 稳定到 stop/destroy 完成；回调不得 unwind、重入或释放连接。
    pub fn CS104_Slave_setConnectionEventHandler(
        slave: Slave,
        handler: ConnectionEventHandler,
        parameter: *mut c_void,
    );
    /// 读取起源地址；无所有权转移。
    /// # Safety
    /// asdu 为存活且同线程独占的完整 ASDU。
    pub fn CS101_ASDU_getOA(asdu: Asdu) -> c_int;
    /// 读取测试标志；无所有权转移。
    /// # Safety
    /// asdu 为存活且同线程独占的完整 ASDU。
    pub fn CS101_ASDU_isTest(asdu: Asdu) -> bool;
    /// 分配总召信息对象，失败返回空；最终由 InformationObject_destroy 释放。
    /// # Safety
    /// self_ 为空，ioa 在24位范围内；调用方独占返回对象并只释放一次。
    pub fn InterrogationCommand_create(
        self_: InformationObject,
        ioa: c_int,
        qoi: u8,
    ) -> InformationObject;
    /// 读取单冗余组低优先级队列条目数；无所有权转移。
    /// # Safety
    /// slave 存活且配置为单冗余组，group 传空；调用期间独占同线程。
    pub fn CS104_Slave_getNumberOfQueueEntries(slave: Slave, group: *mut Opaque) -> c_int;
    /// 将 ASDU 内容复制到低优先级队列，ASDU 仍归调用方；满队列可能替换旧条目。
    /// # Safety
    /// slave 和 asdu 存活且参数匹配、同线程独占；需在调用前自行实施队列背压。
    pub fn CS104_Slave_enqueueASDU(slave: Slave, asdu: Asdu);
    /// 读取总召唤限定词，无所有权转移。
    /// # Safety
    /// object 须为存活且类型为 C_IC_NA_1 的 InterrogationCommand，禁止并发释放。
    pub fn InterrogationCommand_getQOI(object: InformationObject) -> u8;
    /// 查询上游运行时版本，无资源转移。
    /// # Safety
    /// 无参数，无额外前置条件。
    pub fn Lib60870_getLibraryVersionInfo() -> Version;
    /// 查询本项目 C 头文件版本探针。
    /// # Safety
    /// 必须链接本项目对应 ABI 探针；无参数和资源转移。
    pub fn essim_60870_header_version() -> c_int;
    /// 分配 ASDU；参数指针由 C 借用，调用者最终 destroy；失败返回空指针。
    /// # Safety
    /// 参数结构须稳定有效直至 ASDU 释放；地址、长度和标志须符合固定头文件约束。
    pub fn CS101_ASDU_create(
        p: *mut AppParameters,
        sequence: bool,
        cot: c_int,
        oa: c_int,
        ca: c_int,
        test: bool,
        negative: bool,
    ) -> Asdu;
    /// 释放本调用方拥有的 ASDU；不释放借用的参数。
    /// # Safety
    /// asdu 必须为存活且唯一拥有的 create 结果，未在使用中且只释放一次；禁止销毁回调借用。
    pub fn CS101_ASDU_destroy(asdu: Asdu);
    /// 复制信息对象到 ASDU，true 表示成功；对象仍归调用方。
    /// # Safety
    /// asdu 和 object 须存活、同线程独占且参数有效，类型与序列地址须合法。
    pub fn CS101_ASDU_addInformationObject(asdu: Asdu, object: InformationObject) -> bool;
    /// 读取 ASDU 类型编号。
    /// # Safety
    /// asdu 须存活，借用期内禁止并发访问或释放。
    pub fn CS101_ASDU_getTypeID(asdu: Asdu) -> c_int;
    /// 读取 ASDU 传送原因。
    /// # Safety
    /// asdu 须存活，借用期内禁止并发访问或释放。
    pub fn CS101_ASDU_getCOT(asdu: Asdu) -> c_int;
    /// 读取 ASDU 公共地址。
    /// # Safety
    /// asdu 须存活，借用期内禁止并发访问或释放。
    pub fn CS101_ASDU_getCA(asdu: Asdu) -> c_int;
    /// 读取 ASDU 信息对象数量。
    /// # Safety
    /// asdu 须存活，借用期内禁止并发访问或释放。
    pub fn CS101_ASDU_getNumberOfElements(asdu: Asdu) -> c_int;
    /// 读取否定确认标记。
    /// # Safety
    /// asdu 须存活，借用期内禁止并发访问或释放。
    pub fn CS101_ASDU_isNegative(asdu: Asdu) -> bool;
    /// 取得新分配的信息对象；调用方最终 InformationObject_destroy，失败返回空指针。
    /// # Safety
    /// asdu 须为有效、完整且符合类型长度的 ASDU，index 在对象数量内。
    pub fn CS101_ASDU_getElement(asdu: Asdu, index: c_int) -> InformationObject;
    /// 分配单点遥信，失败返回空指针。
    /// # Safety
    /// self_ 传空，ioa 为 24 位无符号地址，quality 必须符合单点质量位规则；调用者最终释放。
    pub fn SinglePointInformation_create(
        self_: InformationObject,
        ioa: c_int,
        value: bool,
        quality: u8,
    ) -> InformationObject;
    /// 读取单点遥信值。
    /// # Safety
    /// object 须为存活且类型为 M_SP_NA_1 的对象，禁止并发释放。
    pub fn SinglePointInformation_getValue(object: InformationObject) -> bool;
    /// 读取单点质量位。
    /// # Safety
    /// object 须为存活且类型为 M_SP_NA_1 的对象，禁止并发释放。
    pub fn SinglePointInformation_getQuality(object: InformationObject) -> u8;
    /// 分配短浮点遥测，失败返回空指针。
    /// # Safety
    /// self_ 传空，ioa 为 24 位地址，value 和质量位由调用者校验；调用者最终释放。
    pub fn MeasuredValueShort_create(
        self_: InformationObject,
        ioa: c_int,
        value: f32,
        quality: u8,
    ) -> InformationObject;
    /// 读取短浮点遥测值。
    /// # Safety
    /// object 须为存活且类型为 M_ME_NC_1 的对象，禁止并发释放。
    pub fn MeasuredValueShort_getValue(object: InformationObject) -> f32;
    /// 读取短浮点质量位。
    /// # Safety
    /// object 须为存活且类型为 M_ME_NC_1 的对象，禁止并发释放。
    pub fn MeasuredValueShort_getQuality(object: InformationObject) -> u8;
    /// 读取对象地址。
    /// # Safety
    /// object 须为存活的信息对象，不得并发释放。
    pub fn InformationObject_getObjectAddress(object: InformationObject) -> c_int;
    /// 释放信息对象及其相关资源。
    /// # Safety
    /// object 须为唯一拥有的构造／getElement 结果，只释放一次；不能传静态或借用对象。
    pub fn InformationObject_destroy(object: InformationObject);
    /// 创建 104 服务端，失败返回空指针，最终由 destroy 释放。
    /// # Safety
    /// 队列长度须为正且受资源预算约束；句柄不能并发访问。
    pub fn CS104_Slave_create(low: c_int, high: c_int) -> Slave;
    /// 设置服务端绑定地址，库内部复制字符串。
    /// # Safety
    /// slave 未启动，address 为有效 NUL 结尾字符串；只在配置阶段独占调用。
    pub fn CS104_Slave_setLocalAddress(slave: Slave, address: *const c_char);
    /// 设置服务端 TCP 端口。
    /// # Safety
    /// slave 未启动且独占，port 为 1 至 65535。
    pub fn CS104_Slave_setLocalPort(slave: Slave, port: c_int);
    /// 设置最大客户端数。
    /// # Safety
    /// slave 未启动且独占；count 不超过上游编译配置的上限。
    pub fn CS104_Slave_setMaxOpenConnections(slave: Slave, count: c_int);
    /// 借用服务端应用层参数。
    /// # Safety
    /// slave 须存活；指针不得释放，仅可在启动前配置，运行后不得修改。
    pub fn CS104_Slave_getAppLayerParameters(slave: Slave) -> *mut AppParameters;
    /// 注册通用 ASDU 回调；不获取 parameter 的所有权。
    /// # Safety
    /// 仅启动前调用，parameter 须保持稳定有效直到 stop 和 destroy 完成；回调不能 unwind 或留下借用指针。
    pub fn CS104_Slave_setASDUHandler(slave: Slave, handler: AsduHandler, parameter: *mut c_void);
    /// 启动无线程服务端，由调用方周期 tick。
    /// # Safety
    /// slave 已配置且独占，之后只在同一线程推进；失败用 isRunning 查询。
    pub fn CS104_Slave_startThreadless(slave: Slave);
    /// 查询服务端运行状态。
    /// # Safety
    /// slave 须存活，不得与释放并发。
    pub fn CS104_Slave_isRunning(slave: Slave) -> bool;
    /// 执行无线程服务端一轮协议处理，可能同步调用注册回调。
    /// # Safety
    /// slave 已以无线程模式启动，当前线程独占；回调不得释放或重入同一 server。
    pub fn CS104_Slave_tick(slave: Slave);
    /// 关闭无线程服务端的监听及连接。
    /// # Safety
    /// slave 为存活的无线程服务端，不在回调内，不得并发访问。
    pub fn CS104_Slave_stopThreadless(slave: Slave);
    /// 释放服务端、连接和队列。
    /// # Safety
    /// 调用者唯一拥有，已停止且无回调执行，回调状态须保持到本函数返回；只销毁一次。
    pub fn CS104_Slave_destroy(slave: Slave);
    /// 向当前回调连接发送或排队 ASDU，false 表示失败；ASDU 仍归调用方。
    /// # Safety
    /// connection 须为有效连接：当前回调借用，或无线程所有者通过连接事件确认仍存活的内部句柄；
    /// 后者须在 CLOSED/DEACTIVATED、stop/destroy 前失效，不能跨线程。asdu 存活且参数匹配。
    pub fn IMasterConnection_sendASDU(connection: Connection, asdu: Asdu) -> bool;
    /// 发送激活确认并修改借用 ASDU 的 COT；false 表示失败。
    /// # Safety
    /// connection 与 asdu 来自当前回调且存活，ASDU 独占访问；不转移所有权。
    pub fn IMasterConnection_sendACT_CON(
        connection: Connection,
        asdu: Asdu,
        negative: bool,
    ) -> bool;
    /// 发送激活终止并修改借用 ASDU 的 COT；false 表示失败。
    /// # Safety
    /// connection 与 asdu 来自当前回调且存活，ASDU 独占访问；不转移所有权。
    pub fn IMasterConnection_sendACT_TERM(connection: Connection, asdu: Asdu) -> bool;
}
