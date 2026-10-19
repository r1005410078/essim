//! Unix串口独占与RAII；C拥有原始fd，Rust持有独立dup，串口仅在所属线程使用。
use crate::{sys, RtuConfig, RtuContext};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{self, Read, Write},
    os::fd::{AsRawFd, FromRawFd},
    os::unix::fs::MetadataExt,
    sync::{Mutex, OnceLock},
};
static CLAIMS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
/// 串口文件和配置上下文绑定生命周期，Drop恢复参数后释放内核与进程占用。
pub struct SerialPort {
    context: Option<RtuContext>,
    file: Option<File>,
    key: String,
}
impl SerialPort {
    /// 校验实际设备身份并独占打开串口；不发送请求。
    /// # Errors
    /// 路径、别名冲突、参数、TIOCEXCL或驱动配置失败。
    pub fn open(path: &str, config: &RtuConfig) -> io::Result<Self> {
        if config.device_path() != path {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "串口路径与配置不一致",
            ));
        }
        let canonical = std::fs::canonicalize(path)?;
        let m = std::fs::metadata(&canonical)?;
        let text = canonical.to_string_lossy();
        let key = if cfg!(target_os = "macos")
            && (text.starts_with("/dev/cu.") || text.starts_with("/dev/tty."))
        {
            format!("mac-serial:{}", text.split_once('.').unwrap().1)
        } else {
            format!("{}:{}:{}", m.dev(), m.rdev(), m.ino())
        };
        let mut claims = CLAIMS
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| io::Error::other("串口占用表不可用"))?;
        if !claims.insert(key.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::AddrInUse,
                "串口已被运行实例占用（含路径别名）",
            ));
        }
        drop(claims);
        let mut port = Self {
            context: None,
            file: None,
            key,
        };
        let context = RtuContext::open(config)?;
        // SAFETY: context存活且当前线程独占；get_socket仅借用，dup创建独立拥有的fd。
        let fd = unsafe { sys::modbus_get_socket(context.handle.as_ptr()) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: fd有效；成功返回的新fd仅由随后的File拥有，不与C重复关闭。
        let duplicated = unsafe { libc::dup(fd) };
        if duplicated < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: dup成功产生独立拥有的描述符，恰好转换一次。
        let file = unsafe { File::from_raw_fd(duplicated) };
        port.context = Some(context);
        port.file = Some(file);
        // SAFETY: 存活串口fd；TIOCEXCL无指针参数，成功后阻止后续非特权打开。
        if unsafe { libc::ioctl(fd, libc::TIOCEXCL as _) } < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: 当前线程独占fd；fcntl读取及设置文件状态，不修改内存。
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        // SAFETY: fd仍由port持有，F_SETFL仅设置独占文件描述的非阻塞标志。
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(port)
    }
}
impl Read for SerialPort {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let file = self
            .file
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?;
        let mut event = libc::pollfd {
            fd: file.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: event指向一个有效pollfd，count=1，非阻塞查询且fd由file持有。
        let result = unsafe { libc::poll(&mut event, 1, 0) };
        if result < 0 {
            return Err(io::Error::last_os_error());
        }
        if event.revents & (libc::POLLHUP | libc::POLLERR | libc::POLLNVAL) != 0 {
            return Err(io::ErrorKind::BrokenPipe.into());
        }
        if result == 0 {
            return Err(io::ErrorKind::WouldBlock.into());
        }
        file.read(buf)
    }
}
impl Write for SerialPort {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.file
            .as_mut()
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotConnected))?
            .write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Drop for SerialPort {
    fn drop(&mut self) {
        drop(self.context.take());
        if let Some(file) = self.file.take() {
            // SAFETY: dup仍存活，C原始fd已关闭并恢复termios；清除内核独占后再关闭最后fd。
            unsafe { libc::ioctl(file.as_raw_fd(), libc::TIOCNXCL as _) };
            drop(file);
        }
        if let Ok(mut claims) = CLAIMS.get_or_init(Default::default).lock() {
            claims.remove(&self.key);
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn aliases_conflict_and_drop_allows_reopen() {
        let pty = crate::test_pty::Pty::new();
        let config = RtuConfig::new(&pty.path, 19200, 'N', 8, 2, 1, 50, 2).unwrap();
        let alias = std::env::temp_dir().join(format!("essim-serial-{}", std::process::id()));
        let _ = std::fs::remove_file(&alias);
        std::os::unix::fs::symlink(&pty.path, &alias).unwrap();
        let port = SerialPort::open(&pty.path, &config).unwrap();
        let other = RtuConfig::new(alias.to_str().unwrap(), 19200, 'N', 8, 2, 1, 50, 2).unwrap();
        assert!(SerialPort::open(alias.to_str().unwrap(), &other).is_err());
        drop(port);
        drop(SerialPort::open(&pty.path, &config).unwrap());
        std::fs::remove_file(alias).unwrap();
    }
}
