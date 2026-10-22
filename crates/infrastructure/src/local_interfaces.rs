//! 本机IPv4发现，只读取接口，不配置地址或试占监听端口。
use std::{collections::BTreeSet, net::Ipv4Addr};
/// 返回系统实际配置的IPv4地址，不将整个127/8推断为已配置。
/// # Errors
/// 系统接口枚举失败时返回错误。
#[cfg(unix)]
pub fn ipv4_addresses() -> Result<BTreeSet<Ipv4Addr>, String> {
    let mut head = std::ptr::null_mut();
    // SAFETY: head是有效可写输出槽，getifaddrs成功时分配完整链表，下面由Guard恰好释放一次。
    if unsafe { libc::getifaddrs(&mut head) } != 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    struct Guard(*mut libc::ifaddrs);
    impl Drop for Guard {
        fn drop(&mut self) {
            // SAFETY: 指针来自成功的getifaddrs且本Guard唯一拥有，读取链表已结束。
            unsafe { libc::freeifaddrs(self.0) };
        }
    }
    let guard = Guard(head);
    let mut current = guard.0;
    let mut result = BTreeSet::new();
    while !current.is_null() {
        // SAFETY: 链表由getifaddrs提供且guard仍存活；ifa_next和ifa_addr均在该分配生命周期内。
        let entry = unsafe { &*current };
        if !entry.ifa_addr.is_null() {
            // SAFETY: 非空地址由OS提供，先读取通用family，再仅对AF_INET按sockaddr_in解释。
            if unsafe { (*entry.ifa_addr).sa_family as i32 } == libc::AF_INET {
                // SAFETY: family为AF_INET，OS保证地址按sockaddr_in对齐且初始化。
                let address = unsafe { &*(entry.ifa_addr as *const libc::sockaddr_in) };
                result.insert(Ipv4Addr::from(address.sin_addr.s_addr.to_ne_bytes()));
            }
        }
        current = entry.ifa_next;
    }
    Ok(result)
}
