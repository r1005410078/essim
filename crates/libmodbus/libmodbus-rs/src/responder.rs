//! 将完整TCP帧交给libmodbus生成响应；外部网络I/O由调用方独立管理。
use super::{invalid, Exception, Mapping, Request, TcpContext, MAX_ADU_LENGTH};
use std::{
    io::{self, Read},
    os::unix::net::UnixStream,
    time::Duration,
};
impl Request {
    /// 校验并复制一个完整Modbus TCP ADU，不执行网络接收。
    ///
    /// 只负责MBAP边界；功能码及其参数仍须交给库／上层策略校验。
    /// # Errors
    /// 长度不是8～260、协议标识非0、MBAP长度不一致时拒绝。
    pub fn from_tcp_adu(bytes: &[u8]) -> io::Result<Self> {
        if !(8..=MAX_ADU_LENGTH).contains(&bytes.len())
            || bytes[2..4] != [0, 0]
            || usize::from(u16::from_be_bytes([bytes[4], bytes[5]])) + 6 != bytes.len()
        {
            return Err(invalid("非法或不完整的Modbus TCP ADU"));
        }
        let mut request = Self {
            bytes: [0; MAX_ADU_LENGTH],
            length: bytes.len(),
        };
        request.bytes[..bytes.len()].copy_from_slice(bytes);
        Ok(request)
    }
}
/// 同一工作线程内的响应生成器；C句柄与私有socket均由RAII释放。
///
/// 私有socket pair只承载库生成的至多260字节响应，生成后立即排空。
/// 外部客户端的阻塞、分帧及发送队列不进入C上下文，句柄不跨线程。
pub struct TcpResponder {
    context: TcpContext<UnixStream>,
    output: UnixStream,
}
impl TcpResponder {
    /// 创建线程绑定的libmodbus响应上下文，不监听外部端口。
    /// # Errors
    /// 本地socket或C上下文分配失败时返回错误。
    pub fn new() -> io::Result<Self> {
        let (input, output) = UnixStream::pair()?;
        input.set_write_timeout(Some(Duration::from_secs(1)))?;
        output.set_read_timeout(Some(Duration::from_secs(1)))?;
        Ok(Self {
            context: TcpContext::with_socket(input, Duration::from_millis(10))?,
            output,
        })
    }
    /// 由libmodbus处理完整请求，返回准备好的响应，尚未发送至外部客户端。
    /// # Errors
    /// 请求非法、库响应失败或私有socket读取失败。
    pub fn respond(
        &mut self,
        bytes: &[u8],
        mapping: &mut Mapping,
        exception: Option<Exception>,
    ) -> io::Result<Vec<u8>> {
        let request = Request::from_tcp_adu(bytes)?;
        let n = match exception {
            Some(code) => self.context.reply_exception(&request, code)?,
            None => self.context.reply(&request, mapping)?,
        };
        let mut result = vec![0; n];
        self.output.read_exact(&mut result)?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_frame_validation_and_library_response() {
        for bytes in [
            &[0u8; 7][..],
            &[0u8; 261][..],
            &[0, 1, 0, 1, 0, 2, 1, 3],
            &[0, 1, 0, 0, 0, 3, 1, 3],
        ] {
            assert!(Request::from_tcp_adu(bytes).is_err());
        }
        let mut responder = TcpResponder::new().unwrap();
        let mut mapping = Mapping::new(
            None,
            None,
            Some(super::super::AddressRange::new(0, 1).unwrap()),
            None,
        )
        .unwrap();
        mapping.set_holding(0, 123).unwrap();
        let read = [0, 1, 0, 0, 0, 6, 1, 3, 0, 0, 0, 1];
        assert_eq!(
            responder.respond(&read, &mut mapping, None).unwrap(),
            [0, 1, 0, 0, 0, 5, 1, 3, 2, 0, 123]
        );
        assert_eq!(
            responder
                .respond(&read, &mut mapping, Some(Exception::IllegalAddress))
                .unwrap(),
            [0, 1, 0, 0, 0, 3, 1, 0x83, 2]
        );
    }
}
