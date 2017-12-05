#[cfg(any(target_os = "ios", target_os = "macos"))]
mod std;
#[cfg(any(target_os = "ios", target_os = "macos"))]
pub use self::std::*;

#[cfg(not(any(target_os = "ios", target_os = "macos")))]
mod ext;
#[cfg(not(any(target_os = "ios", target_os = "macos")))]
pub use self::ext::*;

use std::io;
use std::mem;
use std::net::SocketAddr;

use libc;

#[inline]
fn sockaddr_to_addr(storage: &libc::sockaddr_storage, len: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            debug_assert!(len >= mem::size_of::<libc::sockaddr_in>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in) };
            Ok(SocketAddr::V4(unsafe { mem::transmute(addr) }))
        }
        libc::AF_INET6 => {
            debug_assert!(len >= mem::size_of::<libc::sockaddr_in6>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in6) };
            Ok(SocketAddr::V6(unsafe { mem::transmute(addr) }))
        }
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "invalid argument",
        )),
    }
}
