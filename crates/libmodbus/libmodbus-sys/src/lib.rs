//! libmodbus 3.2.0 原始 API 子集，无所有权/线程安全保证。
//!
//! # Safety
//! 所有函数要求指针有效且独占串行访问；调用者维护对象/socket 生命周期，
//! 按官方要求提供缓冲和长度，不得重复释放。receive 的缓冲按协议至少为 TCP 的 MAX_ADU_LENGTH 或 RTU_MAX_ADU_LENGTH。
//! reply 要求与 context 协议匹配的完整、有效请求；C 对象布局不可从 Rust 解引用。
#![allow(non_camel_case_types)]
use std::ffi::{c_char, c_int, c_uint};
#[repr(C)]
pub struct modbus_t {
    _opaque: [u8; 0],
}
#[repr(C)]
pub struct modbus_mapping_t {
    _opaque: [u8; 0],
}
/// TCP ADU 上限，保留旧名称兼容。
pub const MAX_ADU_LENGTH: usize = 260;
/// RTU ADU 上限，包含站号与 CRC。
pub const RTU_MAX_ADU_LENGTH: usize = 256;
/// 固定官方头文件的自定义协议错误起点。
pub const MODBUS_ENOBASE: i32 = 112345678;
// FFI 边界：声明由 libmodbus 3.2.0 和本项目 native/mapping.c 提供的符号，
// extern "C" 指定使用 C ABI（调用约定），函数实现位于 C 侧。
// unsafe 表示 Rust 无法验证这些外部声明及其安全约束，并不代表调用已被安全封装。
// 调用者必须保证声明与实际链接库的 ABI 一致、指针/缓冲区有效、长度正确，
// 并管理句柄和 socket 的生命周期，避免并发访问及重复释放。
// 业务代码应通过 libmodbus-rs 的安全封装调用，具体公共约束见模块级 Safety 文档。
unsafe extern "C" {
    /// 上游获取借用描述符；不转移所有权，失败返回-1／errno。
    /// # Safety
    /// ctx须存活且独占；返回描述符仅可在上下文生命周期内借用。
    pub fn modbus_get_socket(ctx:*mut modbus_t)->c_int;
    /// 上游发送站号加PDU，按后端补CRC；成功为字节数，失败-1／errno。
    /// # Safety
    /// ctx存活且独占，无在途请求；req指向至少len字节，2～254字节合法请求。
    pub fn modbus_send_raw_request(ctx:*mut modbus_t,req:*const u8,len:c_int)->c_int;

    /// 上游主站 read_bits，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u8，调用期间有效且可写且无其他访问。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_read_bits(ctx: *mut modbus_t, addr: c_int, nb: c_int, data: *mut u8) -> c_int;
    /// 上游主站 read_input_bits，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u8，调用期间有效且可写且无其他访问。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_read_input_bits(
        ctx: *mut modbus_t,
        addr: c_int,
        nb: c_int,
        data: *mut u8,
    ) -> c_int;
    /// 上游主站 read_registers，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u16，调用期间有效且可写且无其他访问。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_read_registers(
        ctx: *mut modbus_t,
        addr: c_int,
        nb: c_int,
        data: *mut u16,
    ) -> c_int;
    /// 上游主站 read_input_registers，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u16，调用期间有效且可写且无其他访问。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_read_input_registers(
        ctx: *mut modbus_t,
        addr: c_int,
        nb: c_int,
        data: *mut u16,
    ) -> c_int;
    /// 上游主站 write_bits，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u8，调用期间有效且可读，位值须为 0/1。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_write_bits(ctx: *mut modbus_t, addr: c_int, nb: c_int, data: *const u8) -> c_int;
    /// 上游主站 write_registers，零基地址；成功返回处理数量，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 必须存活、已连接且独占串行访问；nb 必须为正且不超过协议上限，地址区间不得溢出。
    /// data 必须指向至少 nb 个正确对齐的 u16，调用期间有效且可读，位值须为 0/1。
    /// 不转移句柄或缓冲所有权；调用者负责最终关闭通道及释放 context，不得与其他 I/O 并发。
    pub fn modbus_write_registers(
        ctx: *mut modbus_t,
        addr: c_int,
        nb: c_int,
        data: *const u16,
    ) -> c_int;
    /// 上游主站 write_bit，零基地址；确认成功返回 1，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 须存活、已连接且独占串行访问，addr 为 0–65535；value 为 0/1。
    /// 不转移资源所有权；调用者最终关闭通道并释放 context，不得与其他 I/O 并发。
    pub fn modbus_write_bit(ctx: *mut modbus_t, addr: c_int, value: c_int) -> c_int;
    /// 上游主站 write_register，零基地址；确认成功返回 1，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 须存活、已连接且独占串行访问，addr 为 0–65535；value 为原始寄存器值。
    /// 不转移资源所有权；调用者最终关闭通道并释放 context，不得与其他 I/O 并发。
    pub fn modbus_write_register(ctx: *mut modbus_t, addr: c_int, value: u16) -> c_int;

    /// 实际链接的 libmodbus 主版本号；当前要求为 3。
    pub static libmodbus_version_major: c_uint;
    /// 实际链接的 libmodbus 次版本号；当前要求为 2。
    pub static libmodbus_version_minor: c_uint;
    /// 实际链接的 libmodbus 修订版本号；当前要求为 0。
    pub static libmodbus_version_micro: c_uint;

    /// 创建 TCP 上下文，仅分配和初始化资源，不建立网络连接。
    /// `port` 为 TCP 端口；成功返回句柄，失败返回空指针并设置 errno。
    ///
    /// # Safety
    /// `ip` 必须为空指针或指向调用期间有效、以 NUL 结尾的 C 字符串。
    /// 成功返回的句柄由调用者持有，最终须用 `modbus_free` 释放一次。
    pub fn modbus_new_tcp(ip: *const c_char, port: c_int) -> *mut modbus_t;

    /// 创建 RTU context，不打开串口；成功返回句柄，失败为空并设置 errno。
    ///
    /// # Safety
    /// device 须指向调用期间有效且 NUL 终止的字符串；参数须符合上游约束。
    /// 返回句柄须独占串行访问并最终 free；成功 connect 后先 close，再 free。
    pub fn modbus_new_rtu(
        device: *const c_char,
        baud: c_int,
        parity: c_char,
        data_bit: c_int,
        stop_bit: c_int,
    ) -> *mut modbus_t;
    /// 根据 context 打开通信通道；成功 0，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 须为未连接且独占的有效 context。RTU 成功后串口归 context 管理，
    /// 调用方须 close 一次后 free；固定 Unix 实现 connect 失败会清理其部分 fd。
    pub fn modbus_connect(ctx: *mut modbus_t) -> c_int;
    /// 设置 RTU 站号；成功 0，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 须存活且独占，不能与收发并发调用；站号含义及广播规则由调用者保证。
    pub fn modbus_set_slave(ctx: *mut modbus_t, slave: c_int) -> c_int;
    /// 关闭 context 管理的通信通道，不释放 context，无返回值。
    ///
    /// # Safety
    /// ctx 须存活且独占，所有 I/O 已结束；不得用于关闭由 Rust 管理的借用 socket。
    /// RTU 已打开的串口由此关闭一次，随后仍须 free context，关闭后不得继续 I/O。
    pub fn modbus_close(ctx: *mut modbus_t);
    /// 设置接收确认帧的等待超时；成功 0，失败 -1 并设置 errno。
    ///
    /// # Safety
    /// ctx 须存活且独占；usec 为 0–999999，参数不能与收发并发修改。
    pub fn modbus_set_response_timeout(ctx: *mut modbus_t, sec: u32, usec: u32) -> c_int;

    /// 将已有 socket 描述符关联到上下文；传入 -1 可解除关联。
    /// 本函数不创建或关闭 socket。成功返回 0，失败返回 -1 并设置 errno。
    ///
    /// # Safety
    /// `ctx` 必须是存活且独占访问的上下文。通信期间 socket 必须保持有效，
    /// 调用者须协调 Rust 与 C 侧的描述符管理，避免提前关闭或重复关闭。
    pub fn modbus_set_socket(ctx: *mut modbus_t, socket: c_int) -> c_int;

    /// 设置从站等待新请求的超时：`sec` 为秒，`usec` 为微秒（0..=999999）。
    /// 两者均为 0 时无限等待。成功返回 0，失败返回 -1 并设置 errno。
    ///
    /// # Safety
    /// `ctx` 必须是存活且独占访问的上下文，不能与正在进行的收发并发修改。
    pub fn modbus_set_indication_timeout(ctx: *mut modbus_t, sec: u32, usec: u32) -> c_int;

    /// 设置同一报文相邻字节之间的接收超时：`sec` 为秒，`usec` 为微秒（0..=999999）。
    /// 两者均为 0 时禁用字节间超时。成功返回 0，失败返回 -1 并设置 errno。
    ///
    /// # Safety
    /// `ctx` 必须是存活且独占访问的上下文，不能与正在进行的收发并发修改。
    pub fn modbus_set_byte_timeout(ctx: *mut modbus_t, sec: u32, usec: u32) -> c_int;

    /// 释放上下文及其内部资源，不关闭关联的 socket 或串口；空指针无操作。
    ///
    /// # Safety
    /// 非空 `ctx` 必须由 libmodbus 创建且尚未释放；释放前须结束所有使用它的操作，
    /// 释放后不得再次访问或释放该句柄。寄存器映射须另外释放。
    pub fn modbus_free(ctx: *mut modbus_t);

    /// 创建四类数据区的地址映射，初始值全部为 0；数量为 0 表示不分配该数据区。
    /// 每组 `start_*` 为协议中的零基起始地址，随后参数为数量：
    /// `start_bits/bits` 为线圈，`start_input_bits/input_bits` 为离散输入，
    /// `start_registers/registers` 为保持寄存器，
    /// `start_input_registers/input_registers` 为输入寄存器。
    /// 成功返回映射句柄，失败返回空指针并设置 errno。
    ///
    /// # Safety
    /// 调用者须检查返回值并管理句柄所有权，最终用 `modbus_mapping_free` 释放一次。
    /// 地址与数量须按目标协议范围校验，不能将展示用的 40001 等编号直接当作零基地址。
    pub fn modbus_mapping_new_start_address(
        start_bits: c_uint,
        bits: c_uint,
        start_input_bits: c_uint,
        input_bits: c_uint,
        start_registers: c_uint,
        registers: c_uint,
        start_input_registers: c_uint,
        input_registers: c_uint,
    ) -> *mut modbus_mapping_t;

    /// 释放映射及四类数据区；空指针无操作。
    ///
    /// # Safety
    /// 非空 `mapping` 必须由 libmodbus 创建且尚未释放；释放前须结束所有读写和应答，
    /// 释放后不得再使用该映射及其内部数据区。
    pub fn modbus_mapping_free(mapping: *mut modbus_mapping_t);

    /// 从上下文关联的 socket 接收一条请求，写入 `request`。
    /// 返回请求字节数，0 表示请求被忽略，-1 表示错误并设置 errno。
    ///
    /// # Safety
    /// `ctx` 必须存活、独占访问且关联有效 socket；`request` 必须指向至少
    /// TCP 的 `MAX_ADU_LENGTH` 或 RTU 的 `RTU_MAX_ADU_LENGTH` 字节的可写缓冲区，调用期间不得被其他操作访问。
    pub fn modbus_receive(ctx: *mut modbus_t, request: *mut u8) -> c_int;

    /// 根据请求和映射生成并发送响应；写请求可能修改映射中的值。
    /// `length` 为请求的实际字节数。成功返回发送字节数，失败返回 -1 并设置 errno。
    ///
    /// # Safety
    /// `ctx` 和 `mapping` 必须存活且独占访问，socket 必须有效。
    /// `request` 必须指向与上下文协议匹配、完整且有效的请求，至少可读 `length` 字节；
    /// 不能把任意字节切片直接交给此函数，须先确认帧长度及功能码对应的请求结构。
    pub fn modbus_reply(
        ctx: *mut modbus_t,
        request: *const u8,
        length: c_int,
        mapping: *mut modbus_mapping_t,
    ) -> c_int;

    /// 向请求方发送 Modbus 异常响应，`code` 为协议异常码。
    /// 成功返回发送字节数，失败返回 -1 并设置 errno。
    ///
    /// # Safety
    /// `ctx` 必须存活、独占访问且关联有效 socket；`request` 必须指向与上下文
    /// 协议匹配的完整有效请求。此 API 没有长度参数，调用者须预先确认缓冲区足够长。
    pub fn modbus_reply_exception(ctx: *mut modbus_t, request: *const u8, code: c_uint) -> c_int;

    /// 本项目提供的 C 辅助函数：按零基协议地址访问一个保持寄存器，非上游 API。
    /// `write == 0` 时将寄存器读入 `*value`，否则将 `*value` 写入寄存器。
    /// 成功返回 0；参数或地址无效时返回 -1，并将 errno 设为 EINVAL。
    ///
    /// # Safety
    /// `mapping` 必须是存活且独占访问的映射；`value` 必须指向已初始化、正确对齐的
    /// u16，读寄存器时该内存必须可写，写寄存器时必须可读，且不得与映射数据区重叠。
    pub fn libmodbus_rs_register(
        mapping: *mut modbus_mapping_t,
        address: c_uint,
        value: *mut u16,
        write: c_int,
    ) -> c_int;
    /// 本项目四区本地访问辅助函数，非上游 API；不发送网络请求。
    /// `area` 为 0 线圈、1 离散输入、2 保持寄存器、3 输入寄存器。
    /// `write == 0` 时读取至 `value`，否则写入；位区写入值仅允许 0/1。
    /// 成功返回 0，非法数据区、地址或位值返回 -1 并设置 errno=EINVAL。
    ///
    /// # Safety
    /// 空 mapping/value 指针会被拒绝；非空 `mapping` 须为存活且串行访问的映射，
    /// 写入时须独占访问。非空 `value` 须指向已初始化且对齐的 u16，读取时可写、写入时可读，
    /// 调用期间有效且不与映射重叠；本函数不转移所有权或释放资源。
    pub fn libmodbus_rs_value(
        mapping: *mut modbus_mapping_t,
        area: c_uint,
        address: c_uint,
        value: *mut u16,
        write: c_int,
    ) -> c_int;
}
