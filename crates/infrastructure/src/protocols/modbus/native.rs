use essim_core::domain::modbus::SOC_ADDRESS;
use libmodbus_rs::{Exception, HoldingRegisters, TcpContext};
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};
/// 将已校验的 Modbus TCP 请求交给 libmodbus，执行教学 SOC 的只读应答策略。
///
/// 外部 TCP 连接由 worker 管理；这里通过内部 socketpair 向 libmodbus 输入完整
/// ADU，再取回响应字节，便于 worker 记录报文并控制外部连接的收发和停机。
/// SOC 地址和访问策略属于 ESSim，不放入通用的 libmodbus 安全封装。
/// context 和 mapping 均在 worker 线程内创建、使用并自动释放。
pub(super) struct Native {
    /// 协议处理上下文，持有内部 socketpair 的 libmodbus 一端，不连接外部客户端。
    context: TcpContext<UnixStream>,
    /// 仅包含教学 SOC 地址的保持寄存器映射，保存当前原始值。
    mapping: HoldingRegisters,
    /// 内部通道的 ESSim 一端：写入请求，再读取 libmodbus 生成的响应。
    transport: UnixStream,
}
impl Native {
    /// 创建内部通信通道和单寄存器映射，并写入 SOC 初始原始值。
    ///
    /// `soc` 的业务范围由上层 `Soc` 类型校验；这里仅保存传入的 `u16`。
    ///
    /// # Errors
    /// socketpair 创建、超时设置、映射分配或 libmodbus 上下文初始化失败时返回错误。
    pub fn new(soc: u16) -> std::io::Result<Self> {
        // 两端互通：transport 写入的请求由 context 接收，响应沿相反方向返回。
        let (library_socket, transport) = UnixStream::pair()?;
        for s in [&library_socket, &transport] {
            s.set_read_timeout(Some(Duration::from_millis(100)))?;
            s.set_write_timeout(Some(Duration::from_millis(100)))?;
        }
        let mut mapping = HoldingRegisters::new(SOC_ADDRESS, 1)?;
        mapping.set(SOC_ADDRESS, soc)?;
        let context = TcpContext::with_socket(library_socket, Duration::from_millis(100))?;
        Ok(Self {
            context,
            mapping,
            transport,
        })
    }
    /// 更新本地 SOC 寄存器，供后续读取请求使用，不主动向客户端发送数据。
    ///
    /// # Errors
    /// 寄存器映射访问失败时返回错误；业务范围仍由调用方校验。
    pub fn update(&mut self, soc: u16) -> std::io::Result<()> {
        self.mapping.set(SOC_ADDRESS, soc)
    }
    /// 处理一条完整 Modbus TCP ADU，返回正常响应或协议异常响应的字节。
    ///
    /// 调用方须先通过 worker 的 `frame_length` 检查 MBAP、功能码对应的请求形状
    /// 和完整帧长度，并检查 Unit ID；本方法不接受未经检查的任意网络切片。
    /// FC03 读取交给 libmodbus 按映射检查地址；FC06/FC16 写入按只读策略拒绝，
    /// 其他功能码返回非法功能异常。协议异常响应仍作为 `Ok` 返回。
    ///
    /// # Errors
    /// 内部通道收发、libmodbus 接收或应答失败，接收内容不匹配，
    /// 或响应长度异常时返回错误。
    ///
    /// # Panics
    /// 调用方违反帧校验约定、传入过短切片时，功能码或读取数量的索引可能越界。
    pub fn reply(&mut self, frame: &[u8]) -> std::io::Result<Vec<u8>> {
        self.transport.write_all(frame)?;
        let request = self.context.receive()?;
        if request.as_bytes() != frame {
            return Err(std::io::Error::other("libmodbus 接收长度或字节不匹配"));
        }
        // MBAP 占 7 字节，索引 7 为功能码；FC03 的索引 10–11 为大端读取数量。
        let exception = match frame[7] {
            3 => {
                let quantity = u16::from_be_bytes([frame[10], frame[11]]);
                if quantity == 0 || quantity > 125 {
                    Some(Exception::IllegalValue)
                } else {
                    None
                }
            }
            // 不将写请求交给通用 reply，避免 libmodbus 修改只读 SOC 映射。
            6 | 16 => Some(Exception::IllegalAddress),
            _ => Some(Exception::IllegalFunction),
        };
        let n = if let Some(e) = exception {
            self.context.reply_exception(&request, e)?
        } else {
            self.context.reply(&request, &mut self.mapping)?
        };
        if n == 0 || n > libmodbus_rs::MAX_ADU_LENGTH {
            return Err(std::io::Error::other("libmodbus 响应长度异常"));
        }
        // reply 已将响应写入内部 socket；按其返回长度取回，外部发送由 worker 完成。
        let mut reply = vec![0; n];
        self.transport.read_exact(&mut reply)?;
        Ok(reply)
    }
}
