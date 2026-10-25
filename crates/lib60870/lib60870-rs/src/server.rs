//! 104 无线程服务端；所有 C 操作及同步回调均在创建线程执行。
use crate::{asdu::read_point, failed, invalid, parameters, sys, Asdu, Point, ThreadBound};
use std::{
    ffi::{c_void, CString},
    io,
    marker::PhantomData,
    net::SocketAddr,
    panic::{catch_unwind, AssertUnwindSafe},
    ptr::NonNull,
};

/// 当前回调期间的请求和连接；不能保留连接或把请求交给其他线程。
///
/// 返回 true 表示业务已处理报文；false 让上游返回未知类型响应。
/// 总召／控制的业务校验与确认顺序由调用者明确实现。
pub struct Request<'a> {
    connection: sys::Connection,
    asdu: sys::Asdu,
    _borrow: PhantomData<&'a mut sys::Opaque>,
    _thread: ThreadBound,
    #[cfg(feature = "g0-monitor")]
    session: Option<SessionToken>,
}
impl Request<'_> {
    /// 原始 TypeID，不把未知类型转换成不存在的 Rust 枚举变体。
    pub fn type_id(&self) -> i32 {
        // SAFETY: Request 仅由同步 C 回调构造，借用不会逃出该回调。
        unsafe { sys::CS101_ASDU_getTypeID(self.asdu) }
    }
    /// 公共地址；服务端使用固定两字节 CA。
    pub fn common_address(&self) -> u16 {
        // SAFETY: 当前回调 ASDU 有效，服务端参数固定 CA 两字节。
        unsafe { sys::CS101_ASDU_getCA(self.asdu) as u16 }
    }
    /// 原始传送原因编号。
    pub fn cause(&self) -> i32 {
        // SAFETY: 回调借用存活且没有异步或跨线程访问。
        unsafe { sys::CS101_ASDU_getCOT(self.asdu) }
    }
    /// 读取支持的监视信息对象为自有 Rust 值。
    /// # Errors
    /// 类型未封装、索引越界、编码无效或分配失败。
    pub fn point(&self, index: usize) -> io::Result<Point> {
        // SAFETY: 上游交付完整 ASDU，借用覆盖整次读取；read_point 校验类型和索引。
        unsafe { read_point(self.asdu, index) }
    }
    /// 读取 C_IC_NA_1 总召唤限定词；不自动接受站总召或分组总召。
    /// # Errors
    /// 类型／数量不匹配，或上游不能解码该信息对象。
    pub fn interrogation_qualifier(&self) -> io::Result<u8> {
        // SAFETY: ASDU 为当前回调借用，所有查询及 getElement 均同步且独占。
        unsafe {
            if self.type_id() != 100 || sys::CS101_ASDU_getNumberOfElements(self.asdu) != 1 {
                return Err(invalid("请求不是单对象总召"));
            }
            let object = NonNull::new(sys::CS101_ASDU_getElement(self.asdu, 0))
                .ok_or_else(|| invalid("总召对象无效"))?;
            let qoi = sys::InterrogationCommand_getQOI(object.as_ptr());
            sys::InformationObject_destroy(object.as_ptr());
            Ok(qoi)
        }
    }
    /// 在本连接发送或排队一个非空 ASDU；成功不表示远端已经收到或执行。
    /// # Errors
    /// ASDU 为空，或上游连接／队列拒绝发送。
    pub fn send(&mut self, asdu: &Asdu) -> io::Result<()> {
        if asdu.is_empty() {
            return Err(invalid("不能发送空 ASDU"));
        }
        // SAFETY: 连接仅在当前回调借用，Asdu 参数与服务端固定参数一致，C 复制编码不接管句柄。
        accepted(unsafe { sys::IMasterConnection_sendASDU(self.connection, asdu.handle.as_ptr()) })
    }
    /// 基于当前请求发送正／负激活确认；上游会修改该请求的 COT／否定标记。
    /// # Errors
    /// 连接／队列拒绝发送。
    pub fn confirm(&mut self, negative: bool) -> io::Result<()> {
        // SAFETY: 独占当前 Request，两个借用句柄覆盖调用期且不转移所有权。
        accepted(unsafe {
            sys::IMasterConnection_sendACT_CON(self.connection, self.asdu, negative)
        })
    }
    /// 基于当前请求发送激活终止；业务必须先确认操作确实完成。
    /// # Errors
    /// 连接／队列拒绝发送。
    pub fn finish(&mut self) -> io::Result<()> {
        // SAFETY: 独占当前 Request，两个借用句柄覆盖调用期且不转移所有权。
        accepted(unsafe { sys::IMasterConnection_sendACT_TERM(self.connection, self.asdu) })
    }
}
fn accepted(result: bool) -> io::Result<()> {
    if result {
        Ok(())
    } else {
        Err(failed("上游连接／发送队列拒绝 ASDU"))
    }
}
type Handler = dyn for<'a> FnMut(&mut Request<'a>) -> bool;
struct CallbackState {
    handler: Box<Handler>,
    panicked: bool,
    #[cfg(feature = "g0-monitor")]
    monitor: MonitorState,
}

/// C 回调只借用句柄；该函数不释放 server，也不让 panic 跨越 ABI。
/// # Safety
/// parameter 指向未移动且独占的 CallbackState，connection/asdu 为当前有效借用。
unsafe extern "C" fn callback(
    parameter: *mut c_void,
    connection: sys::Connection,
    asdu: sys::Asdu,
) -> bool {
    if parameter.is_null() || connection.is_null() || asdu.is_null() {
        return false;
    }
    // SAFETY: 唯一注册位置传入 Box 稳定地址；仅无线程模式调用，Server 销毁前保持存活。
    let state = unsafe { &mut *parameter.cast::<CallbackState>() };
    if state.panicked {
        return false;
    }
    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut request = Request {
            connection,
            asdu,
            _borrow: PhantomData,
            _thread: PhantomData,
            #[cfg(feature = "g0-monitor")]
            session: state.monitor.token(connection),
        };
        (state.handler)(&mut request)
    }));
    match result {
        Ok(handled) => handled,
        Err(payload) => {
            // 用户自定义 panic payload 的 Drop 也可能 panic；仅此故障路径保留 payload，避免二次 unwind 跨 ABI。
            std::mem::forget(payload);
            state.panicked = true;
            false
        }
    }
}
/// 创建线程独占的 104 无线程服务端，显式 tick 驱动；Drop 先停止再销毁 C 资源。
///
/// 当前使用固定标准 104 应用参数、单冗余组、最多 10 个客户端及各 64 条高／低优先级队列。
/// 不承诺 tick 非阻塞、事件队列无损或实时性，不隐含业务数据模型。
pub struct Server {
    handle: NonNull<sys::Opaque>,
    // Box 保持回调 cookie 地址稳定；析构发生在 Drop 的 stop/destroy 之后。
    state: Box<CallbackState>,
    _thread: ThreadBound,
}
impl Server {
    /// 单冗余组中待发送或待确认的低优先级事件数，不表示业务完成数量。
    pub fn queued(&self) -> usize {
        // SAFETY: 句柄存活且本封装固定为单冗余组；空 group 为该模式的合法查询。
        unsafe {
            sys::CS104_Slave_getNumberOfQueueEntries(self.handle.as_ptr(), std::ptr::null_mut())
                .max(0) as usize
        }
    }
    /// 复制一个非空 ASDU 到低优先级队列；可用于周期／自发上送。
    ///
    /// 在同一所有者线程中先检查 64 条队列上限，拒绝替换旧事件；成功只表示本地排队。
    /// # Errors
    /// 服务停止、ASDU 为空，或低优先级队列已满。
    pub fn enqueue(&mut self, asdu: &Asdu) -> io::Result<()> {
        if !self.is_running() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "104 服务未运行",
            ));
        }
        if asdu.is_empty() {
            return Err(invalid("不能排队空 ASDU"));
        }
        if self.queued() >= 64 {
            return Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "104 低优先级队列已满",
            ));
        }
        // SAFETY: 句柄和 ASDU 独占存活，参数均固定为标准 104；同线程检查后没有其他写入者，C 复制内容。
        unsafe { sys::CS104_Slave_enqueueASDU(self.handle.as_ptr(), asdu.handle.as_ptr()) };
        Ok(())
    }
    /// 在明确的本地地址启动服务；回调只在 tick 中同步执行。
    ///
    /// 回调应快速完成，不执行无界等待，不重入该 server。panic 后本次 tick 返回错误并停止服务。
    /// # Errors
    /// 端口为 0、内存分配失败，或地址不能监听。
    pub fn bind(
        address: SocketAddr,
        handler: impl for<'a> FnMut(&mut Request<'a>) -> bool + 'static,
    ) -> io::Result<Self> {
        if address.port() == 0 {
            return Err(invalid("首版要求显式非零端口"));
        }
        let local = CString::new(address.ip().to_string()).map_err(|_| invalid("绑定地址无效"))?;
        // SAFETY: 两队列长度为受限正数；返回句柄交由 Server 唯一拥有。
        let raw = unsafe { sys::CS104_Slave_create(64, 64) };
        let mut server = Self {
            handle: NonNull::new(raw).ok_or_else(|| failed("104 服务端分配失败"))?,
            state: Box::new(CallbackState {
                handler: Box::new(handler),
                panicked: false,
                #[cfg(feature = "g0-monitor")]
                monitor: MonitorState::default(),
            }),
            _thread: PhantomData,
        };
        let cookie = (&mut *server.state as *mut CallbackState).cast::<c_void>();
        // SAFETY: 句柄未启动且独占；应用参数只在启动前设置，C 复制地址字符串；cookie 的堆地址稳定到 destroy 返回。
        unsafe {
            sys::CS104_Slave_setLocalAddress(raw, local.as_ptr());
            sys::CS104_Slave_setLocalPort(raw, address.port().into());
            sys::CS104_Slave_setMaxOpenConnections(raw, 10);
            *sys::CS104_Slave_getAppLayerParameters(raw) = parameters();
            sys::CS104_Slave_setASDUHandler(raw, Some(callback), cookie);
            #[cfg(feature = "g0-monitor")]
            sys::CS104_Slave_setConnectionEventHandler(raw, Some(connection_event), cookie);
            #[cfg(feature = "g0-monitor")]
            sys::CS104_Slave_setRawMessageHandler(raw, Some(startdt_reset), cookie);
            sys::CS104_Slave_startThreadless(raw);
        }
        if !server.is_running() {
            return Err(io::Error::new(
                io::ErrorKind::AddrNotAvailable,
                "104 监听失败",
            ));
        }
        Ok(server)
    }
    /// 推进一轮连接、报文、发送和定时任务；由应用按实际负载周期调用。
    /// # Errors
    /// 服务已停止，或用户回调 panic；panic 后停止服务且后续仍返回故障。
    pub fn tick(&mut self) -> io::Result<()> {
        if self.state.panicked {
            return Err(failed("104 用户回调 panic，服务已停止"));
        }
        if !self.is_running() {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "104 服务未运行",
            ));
        }
        // SAFETY: 服务已在当前线程启动，&mut self 保证无重入；回调状态未被其他位置借用。
        unsafe { sys::CS104_Slave_tick(self.handle.as_ptr()) };
        if self.state.panicked {
            self.stop();
            return Err(failed("104 用户回调 panic，服务已停止"));
        }
        Ok(())
    }
    /// 查询上游服务是否运行，不等同于客户端已连接或数据传输已激活。
    pub fn is_running(&self) -> bool {
        // SAFETY: 句柄存活，线程绑定且只读查询。
        unsafe { sys::CS104_Slave_isRunning(self.handle.as_ptr()) }
    }
    /// 关闭监听及连接；可重复调用，随后不能重新启动同一句柄。
    pub fn stop(&mut self) {
        // 先使外部token失效；上游stop也可能同步产生连接关闭通知。
        #[cfg(feature = "g0-monitor")]
        self.state.monitor.sessions.clear();
        // SAFETY: 当前线程独占无线程服务端，回调 cookie 仍存活，没有回调在执行。
        unsafe { sys::CS104_Slave_stopThreadless(self.handle.as_ptr()) }
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop();
        // SAFETY: 句柄已停止且唯一拥有；回调 Box 仍活到本函数结束后的字段析构。
        unsafe { sys::CS104_Slave_destroy(self.handle.as_ptr()) }
    }
}

/// G0监控实验的会话身份；不含可访问的C指针，绑定创建线程和所属Server。
///
/// 只在观测到的当前STARTDT激活期有效；重复STARTDT、STOPDT、断连和stop使其失效。
/// 底层断线尚未被tick观测时，接纳仍不代表实际交付。
/// ```compile_fail
/// fn move_token(token: lib60870_rs::SessionToken) {
///     std::thread::spawn(move || drop(token));
/// }
/// ```
#[cfg(feature = "g0-monitor")]
#[derive(Clone, Debug)]
pub struct SessionToken {
    owner: std::rc::Rc<()>,
    generation: u64,
}
#[cfg(feature = "g0-monitor")]
struct SessionEntry {
    connection: sys::Connection,
    generation: u64,
}
#[cfg(feature = "g0-monitor")]
#[derive(Default)]
struct MonitorState {
    owner: std::rc::Rc<()>,
    next: u64,
    sessions: std::collections::BTreeMap<usize, SessionEntry>,
}
#[cfg(feature = "g0-monitor")]
impl MonitorState {
    fn token(&self, connection: sys::Connection) -> Option<SessionToken> {
        self.sessions
            .get(&(connection as usize))
            .map(|entry| SessionToken {
                owner: self.owner.clone(),
                generation: entry.generation,
            })
    }
}
/// 内部事件处理不调用用户代码，也不解引用连接；C在同一tick内释放前通知CLOSED。
/// # Safety
/// parameter为注册的稳定CallbackState，connection在当前通知期间有效，event匹配固定枚举ABI。
#[cfg(feature = "g0-monitor")]
unsafe extern "C" fn connection_event(
    parameter: *mut c_void,
    connection: sys::Connection,
    event: i32,
) {
    if parameter.is_null() || connection.is_null() {
        return;
    }
    // SAFETY: 无线程模式同步通知，cookie在stop/destroy返回前存活；不重入用户handler。
    let state = unsafe { &mut *parameter.cast::<CallbackState>() };
    let result = catch_unwind(AssertUnwindSafe(|| match event {
        2 => {
            let Some(generation) = state.monitor.next.checked_add(1) else {
                state.panicked = true;
                return;
            };
            state.monitor.next = generation;
            // Server固定为单冗余组；上游在激活新连接前关闭旧活动连接，CLOSED可能到下一tick。
            // 这里先撤销旧token，不让它在通知延迟期间继续接纳ASDU。
            state.monitor.sessions.clear();
            state.monitor.sessions.insert(
                connection as usize,
                SessionEntry {
                    connection,
                    generation,
                },
            );
        }
        0 | 1 | 3 => {
            state.monitor.sessions.remove(&(connection as usize));
        }
        _ => {
            state.panicked = true;
            state.monitor.sessions.clear();
        }
    }));
    if let Err(payload) = result {
        std::mem::forget(payload);
        state.panicked = true;
        state.monitor.sessions.clear();
    }
}

/// 总召请求的自有监控数据；没有Request、ASDU或C连接借用。
/// 仅支持C_IC_NA_1激活、IOA=0、站总召QOI=20；不提供延迟控制接口。
#[cfg(feature = "g0-monitor")]
#[derive(Clone, Debug)]
pub struct Interrogation {
    session: SessionToken,
    ca: u16,
    oa: u8,
    test: bool,
}
#[cfg(feature = "g0-monitor")]
impl Interrogation {
    /// 请求所属的激活会话；发送时仍须由原Server验证。
    pub fn session(&self) -> &SessionToken {
        &self.session
    }
    /// 请求公共地址；业务层另行核对模型范围。
    pub fn common_address(&self) -> u16 {
        self.ca
    }
    /// 创建响应总召的空监视ASDU，保留CA、OA与测试标志；调用者添加首批支持的Point。
    /// # Errors
    /// 上游内存分配失败。
    pub fn data(&self, sequence: bool) -> io::Result<Asdu> {
        Asdu::with_header(
            self.ca,
            crate::Cause::Interrogated,
            sequence,
            self.oa,
            self.test,
            false,
        )
    }
    /// 构造独立激活确认，保留请求的OA／测试标志。
    /// # Errors
    /// 上游内存分配或编码失败。
    pub fn confirmation(&self, negative: bool) -> io::Result<Asdu> {
        Asdu::interrogation_response(
            self.ca,
            self.oa,
            20,
            self.test,
            negative,
            crate::Cause::ActivationConfirmation,
        )
    }
    /// 构造独立激活终止；调用者只能在业务批次完成后排入FIFO。
    /// # Errors
    /// 上游内存分配或编码失败。
    pub fn termination(&self) -> io::Result<Asdu> {
        Asdu::interrogation_response(
            self.ca,
            self.oa,
            20,
            self.test,
            false,
            crate::Cause::ActivationTermination,
        )
    }
}
#[cfg(feature = "g0-monitor")]
impl Request<'_> {
    /// 复制受限站总召数据，供所有者在回调返回后分批处理；不会保留C借用。
    /// # Errors
    /// 请求不是单对象激活站总召、IOA非0、否定激活，或当前连接未激活。
    pub fn owned_interrogation(&self) -> io::Result<Interrogation> {
        if self.cause() != 6 || self.interrogation_qualifier()? != 20 {
            return Err(invalid("G0仅支持激活站总召"));
        }
        // SAFETY: 当前回调ASDU存活且已校验类型／对象数；getElement返回独占对象，同步释放。
        let (ioa, oa, test, negative) = unsafe {
            let object = NonNull::new(sys::CS101_ASDU_getElement(self.asdu, 0))
                .ok_or_else(|| invalid("总召对象无效"))?;
            let ioa = sys::InformationObject_getObjectAddress(object.as_ptr());
            sys::InformationObject_destroy(object.as_ptr());
            (
                ioa,
                sys::CS101_ASDU_getOA(self.asdu),
                sys::CS101_ASDU_isTest(self.asdu),
                sys::CS101_ASDU_isNegative(self.asdu),
            )
        };
        if ioa != 0 || negative {
            return Err(invalid("总召IOA／否定标志无效"));
        }
        Ok(Interrogation {
            session: self
                .session
                .clone()
                .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "会话未激活"))?,
            ca: self.common_address(),
            oa: oa as u8,
            test,
        })
    }
}
#[cfg(feature = "g0-monitor")]
impl Server {
    /// 查询凭证是否属于本Server当前已观测的激活代际；不发送ASDU、不推进tick。
    ///
    /// 调用方在每次tick后检查持有的凭证，即使应用FIFO为空也能发现重置并启动快照重同步。
    /// false包括外部Server凭证、已观测失效、停止或回调故障；尚未观测到的网络故障不能由此排除。
    pub fn session_is_current(&self, session: &SessionToken) -> bool {
        !self.state.panicked
            && self.is_running()
            && std::rc::Rc::ptr_eq(&session.owner, &self.state.monitor.owner)
            && self
                .state
                .monitor
                .sessions
                .values()
                .any(|entry| entry.generation == session.generation)
    }
    /// 当前已观测的激活会话，最多10项；只供G0监控实验。
    pub fn active_sessions(&self) -> Vec<SessionToken> {
        self.state
            .monitor
            .sessions
            .values()
            .map(|entry| SessionToken {
                owner: self.state.monitor.owner.clone(),
                generation: entry.generation,
            })
            .collect()
    }
    /// 经会话代际校验，将ASDU复制到该连接的高优先级FIFO；成功仅表示库接纳。
    ///
    /// 不调用tick，不绕过已有队列；WouldBlock时调用者保留FIFO队首，之后推进tick重试。
    /// # Errors
    /// 空ASDU、非所属Server、已失效会话／服务停止，或队列满（WouldBlock）。
    pub fn try_send_to(&mut self, session: &SessionToken, asdu: &Asdu) -> io::Result<()> {
        if !self.is_running() {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "服务已停止"));
        }
        if asdu.is_empty() {
            return Err(invalid("不能发送空ASDU"));
        }
        if !std::rc::Rc::ptr_eq(&session.owner, &self.state.monitor.owner) {
            return Err(invalid("会话不属于此Server"));
        }
        if !self.session_is_current(session) {
            return Err(io::Error::new(io::ErrorKind::NotConnected, "会话已失效"));
        }
        let entry = self
            .state
            .monitor
            .sessions
            .values()
            .find(|entry| entry.generation == session.generation)
            .ok_or_else(|| io::Error::new(io::ErrorKind::NotConnected, "会话已失效"))?;
        // SAFETY: 内部表仅保存同线程ACTIVATED到DEACTIVATED/CLOSED的连接；stop先清表。
        // &mut self禁止并发tick及回调释放，调用仅复制ASDU，不泄露指针。
        if unsafe { sys::IMasterConnection_sendASDU(entry.connection, asdu.handle.as_ptr()) } {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::WouldBlock,
                "高优先级队列暂不能接纳",
            ))
        }
    }
}

/// 固定上游在每次STARTDT时清高队列，但已激活时不再次通知ACTIVATED。
/// 按固定源码handleMessage的分支优先级识别实际会重置的输入，包括其接受的非标准U帧。
/// 在该tick返回给应用前使旧token失效，不把这些输入声明为合法104报文，也不暴露发送证据。
/// # Safety
/// cookie指向稳定CallbackState；msg在此回调期间可读size字节；无线程模式、禁止重入。
#[cfg(feature = "g0-monitor")]
unsafe extern "C" fn startdt_reset(
    parameter: *mut c_void,
    connection: sys::Connection,
    msg: *mut u8,
    size: i32,
    sent: bool,
) {
    // 发送通知可能出现在C调用栈内；完全不触碰state，避免与其他回调的可变借用重叠。
    if sent || parameter.is_null() || connection.is_null() || msg.is_null() || size < 3 {
        return;
    }
    // SAFETY: 上游通知缓冲区在回调期间有size字节，已排除负数及过短消息；不保留指针。
    let bytes = unsafe { std::slice::from_raw_parts(msg, size as usize) };
    if bytes[0] != 0x68 || usize::from(bytes[1]) != bytes.len() - 2 {
        return;
    }
    // 必须与固定上游一致：I帧和TESTFR_ACT分支先于STARTDT，且不校验U帧保留字节。
    let control = bytes[2];
    if control & 1 == 0 || control & 0x43 == 0x43 || control & 7 != 7 {
        return;
    }
    // SAFETY: 接收通知在handleASDU/连接事件之前同步调用，无其他state借用；Box地址固定。
    let state = unsafe { &mut *parameter.cast::<CallbackState>() };
    if let Some(entry) = state.monitor.sessions.get_mut(&(connection as usize)) {
        if let Some(generation) = state.monitor.next.checked_add(1) {
            state.monitor.next = generation;
            entry.generation = generation;
        } else {
            state.panicked = true;
            state.monitor.sessions.clear();
        }
    }
}
