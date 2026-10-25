//! 拥有型 ASDU 与支持的信息对象；C 编解码和对象布局由上游维护。
use crate::{failed, invalid, parameters, sys, ThreadBound};
use std::{
    io,
    marker::PhantomData,
    ptr::{self, NonNull},
};

/// 首批支持的传送原因，未提供任意整数转换。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(i32)]
pub enum Cause {
    /// 周期上送。
    Periodic = 1,
    /// 自发上送。
    Spontaneous = 3,
    /// 激活请求。
    Activation = 6,
    /// 激活确认。
    ActivationConfirmation = 7,
    /// 激活终止。
    ActivationTermination = 10,
    /// 响应站总召的数据。
    Interrogated = 20,
}
/// 质量描述符；允许 OV、BL、SB、NT、IV，单点额外禁止 OV。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Quality(u8);
impl Quality {
    /// 没有质量异常。
    pub const GOOD: Self = Self(0);
    /// 从协议位构造质量，拒绝保留位。
    /// # Errors
    /// bits 包含 0x0e 保留位。
    pub fn new(bits: u8) -> io::Result<Self> {
        if bits & 0x0e != 0 {
            return Err(invalid("质量包含保留位"));
        }
        Ok(Self(bits))
    }
    /// 获取协议质量位。
    pub fn bits(self) -> u8 {
        self.0
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
enum Value {
    Single(bool),
    Measured(f32),
}
/// 已校验的无时标单点或短浮点信息；地址是零基协议 IOA，不含设备业务约束。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    address: u32,
    value: Value,
    quality: Quality,
}
impl Point {
    /// 构造 M_SP_NA_1 单点遥信。
    /// # Errors
    /// IOA 超过 24 位，或质量包含单点不支持的 OV 位。
    pub fn single(address: u32, value: bool, quality: Quality) -> io::Result<Self> {
        if address > 0xff_ffff || quality.0 & 1 != 0 {
            return Err(invalid("单点 IOA 或质量无效"));
        }
        Ok(Self {
            address,
            value: Value::Single(value),
            quality,
        })
    }
    /// 构造 M_ME_NC_1 短浮点遥测；不进行倍率换算。
    /// # Errors
    /// IOA 超过 24 位，或 value 为非有限值。
    pub fn measured(address: u32, value: f32, quality: Quality) -> io::Result<Self> {
        if address > 0xff_ffff || !value.is_finite() {
            return Err(invalid("遥测 IOA 或浮点值无效"));
        }
        Ok(Self {
            address,
            value: Value::Measured(value),
            quality,
        })
    }
    /// 获取信息体地址。
    pub fn address(self) -> u32 {
        self.address
    }
    /// 获取质量位。
    pub fn quality(self) -> Quality {
        self.quality
    }
    /// 获取单点值；遥测返回 None。
    pub fn single_value(self) -> Option<bool> {
        if let Value::Single(v) = self.value {
            Some(v)
        } else {
            None
        }
    }
    /// 获取遥测值；单点返回 None。
    pub fn measured_value(self) -> Option<f32> {
        if let Value::Measured(v) = self.value {
            Some(v)
        } else {
            None
        }
    }
    fn type_id(self) -> i32 {
        match self.value {
            Value::Single(_) => 1,
            Value::Measured(_) => 13,
        }
    }
}
struct Object(NonNull<sys::Opaque>);
impl Drop for Object {
    fn drop(&mut self) {
        // SAFETY: Object 唯一拥有构造／getElement 分配的对象，不转移给 ASDU，只释放一次。
        unsafe { sys::InformationObject_destroy(self.0.as_ptr()) }
    }
}
/// 独占的上游 ASDU；参数堆地址稳定，先销毁 ASDU 后释放参数。
///
/// ```compile_fail
/// let asdu = lib60870_rs::Asdu::new(1, lib60870_rs::Cause::Periodic, false).unwrap();
/// std::thread::spawn(move || drop(asdu)); // C 资源不能跨线程
/// ```
pub struct Asdu {
    pub(crate) handle: NonNull<sys::Opaque>,
    _parameters: Box<sys::AppParameters>,
    sequence: bool,
    first: Option<u32>,
    _thread: ThreadBound,
}
impl Asdu {
    #[cfg(feature = "g0-monitor")]
    pub(crate) fn interrogation_response(
        ca: u16,
        oa: u8,
        qoi: u8,
        test: bool,
        negative: bool,
        cause: Cause,
    ) -> io::Result<Self> {
        let mut result = Self::with_header(ca, cause, false, oa, test, negative)?;
        // SAFETY: 信息对象由本函数唯一拥有，ASDU只复制编码；两者参数匹配，均在同线程存活。
        unsafe {
            let object = Object(
                NonNull::new(sys::InterrogationCommand_create(ptr::null_mut(), 0, qoi))
                    .ok_or_else(|| failed("总召对象分配失败"))?,
            );
            if !sys::CS101_ASDU_addInformationObject(result.handle.as_ptr(), object.0.as_ptr()) {
                return Err(invalid("总召响应编码失败"));
            }
        }
        result.first = Some(0);
        Ok(result)
    }
    /// 创建标准 104 参数的无时标 ASDU，起源地址固定为 0。
    /// sequence=true 要求同类型且连续 IOA；ca 为协议公共地址，不自动绑定设备。
    /// # Errors
    /// C 内存分配失败。
    pub fn new(ca: u16, cause: Cause, sequence: bool) -> io::Result<Self> {
        Self::with_header(ca, cause, sequence, 0, false, false)
    }
    pub(crate) fn with_header(
        ca: u16,
        cause: Cause,
        sequence: bool,
        oa: u8,
        test: bool,
        negative: bool,
    ) -> io::Result<Self> {
        let mut p = Box::new(parameters());
        // SAFETY: 参数位宽与 ASDU 上限已固定为 104；Box 在句柄销毁前保持地址稳定且不修改。
        let handle = unsafe {
            sys::CS101_ASDU_create(
                &mut *p,
                sequence,
                cause as i32,
                oa.into(),
                ca.into(),
                test,
                negative,
            )
        };
        Ok(Self {
            handle: NonNull::new(handle).ok_or_else(|| failed("ASDU 分配失败"))?,
            _parameters: p,
            sequence,
            first: None,
            _thread: PhantomData,
        })
    }
    /// 添加一个已校验对象，上游复制编码内容，临时信息对象随后释放。
    /// # Errors
    /// 混合类型、序列地址不连续、数量或长度超限，或分配失败；拒绝不改变已有内容。
    pub fn add(&mut self, point: Point) -> io::Result<()> {
        let count = self.len();
        if count >= 127
            || (count > 0 && self.type_id() != point.type_id())
            || (self.sequence
                && self
                    .first
                    .is_some_and(|a| a.checked_add(count as u32) != Some(point.address)))
        {
            return Err(invalid("ASDU 类型、数量或连续地址不匹配"));
        }
        // SAFETY: Point 的地址、有限浮点、质量已校验，空 self 要求 C 分配独占对象。
        let raw = unsafe {
            match point.value {
                Value::Single(v) => sys::SinglePointInformation_create(
                    ptr::null_mut(),
                    point.address as i32,
                    v,
                    point.quality.0,
                ),
                Value::Measured(v) => sys::MeasuredValueShort_create(
                    ptr::null_mut(),
                    point.address as i32,
                    v,
                    point.quality.0,
                ),
            }
        };
        let object = Object(NonNull::new(raw).ok_or_else(|| failed("信息对象分配失败"))?);
        // SAFETY: 两对象均独占存活，类型／序列已校验，C 复制内容不接管临时对象。
        if !unsafe { sys::CS101_ASDU_addInformationObject(self.handle.as_ptr(), object.0.as_ptr()) }
        {
            return Err(invalid("ASDU 编码长度超限"));
        }
        if self.first.is_none() {
            self.first = Some(point.address);
        }
        Ok(())
    }
    /// 当前信息对象数量。
    pub fn len(&self) -> usize {
        // SAFETY: 自有句柄存活，线程绑定且只读查询。
        unsafe { sys::CS101_ASDU_getNumberOfElements(self.handle.as_ptr()) as usize }
    }
    /// 是否尚无信息对象。
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    /// 获取公共地址。
    pub fn common_address(&self) -> u16 {
        // SAFETY: 自有句柄存活，公共地址由 u16 构造。
        unsafe { sys::CS101_ASDU_getCA(self.handle.as_ptr()) as u16 }
    }
    /// 获取 TypeID；添加首个对象前为 0。
    pub fn type_id(&self) -> i32 {
        // SAFETY: 自有句柄存活，线程绑定且只读查询。
        unsafe { sys::CS101_ASDU_getTypeID(self.handle.as_ptr()) }
    }
    /// 解码支持的信息对象并返回独立 Rust 值。
    /// # Errors
    /// 索引越界、类型尚未封装、对象无效或分配失败。
    pub fn point(&self, index: usize) -> io::Result<Point> {
        // SAFETY: 自有 ASDU 生命周期覆盖整次解码。
        unsafe { read_point(self.handle.as_ptr(), index) }
    }
}
impl Drop for Asdu {
    fn drop(&mut self) {
        // SAFETY: 唯一拥有 create 句柄；在参数 Box 的字段析构之前释放，当前无其他借用。
        unsafe { sys::CS101_ASDU_destroy(self.handle.as_ptr()) }
    }
}
/// 从存活、已由上游接收校验的 ASDU 读取支持类型，不保留任何 C 指针。
/// # Safety
/// handle 必须在整个调用中存活且同线程独占；包含有效参数与完整类型载荷。
pub(crate) unsafe fn read_point(handle: sys::Asdu, index: usize) -> io::Result<Point> {
    // SAFETY: 调用方保证句柄和参数存活，查询不转移所有权。
    let (kind, count) = unsafe {
        (
            sys::CS101_ASDU_getTypeID(handle),
            sys::CS101_ASDU_getNumberOfElements(handle),
        )
    };
    if index >= count as usize || !matches!(kind, 1 | 13) {
        return Err(invalid("索引越界或信息类型尚未封装"));
    }
    // SAFETY: 索引和类型已验证；getElement 分配新对象，由 Object 唯一释放。
    let object = Object(
        NonNull::new(unsafe { sys::CS101_ASDU_getElement(handle, index as i32) })
            .ok_or_else(|| failed("信息对象无效"))?,
    );
    let p = object.0.as_ptr();
    // SAFETY: p 是按上游 TypeID 解码的独占存活对象，以下访问器与其具体类型一致。
    unsafe {
        match kind {
            1 => Point::single(
                sys::InformationObject_getObjectAddress(p) as u32,
                sys::SinglePointInformation_getValue(p),
                Quality::new(sys::SinglePointInformation_getQuality(p))?,
            ),
            _ => Point::measured(
                sys::InformationObject_getObjectAddress(p) as u32,
                sys::MeasuredValueShort_getValue(p),
                Quality::new(sys::MeasuredValueShort_getQuality(p))?,
            ),
        }
    }
}
