use std::io;
use std::mem;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use libc;

use sys::unix::err::cvt;

#[inline]
#[allow(dead_code)]
pub fn pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    cvt(unsafe { libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC | libc::O_NONBLOCK) })?;
    Ok((fds[0], fds[1]))
}

#[inline]
pub fn socket_v4() -> io::Result<RawFd> {
    let res = unsafe {
        libc::socket(
            libc::AF_INET,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    cvt(res)
}

#[inline]
pub fn socket_v6() -> io::Result<RawFd> {
    let res = unsafe {
        libc::socket(
            libc::AF_INET6,
            libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
        )
    };
    cvt(res)
}

#[inline]
pub fn accept(listener_fd: RawFd) -> io::Result<(RawFd, SocketAddr)> {
    let mut storage: libc::sockaddr_storage = unsafe { mem::uninitialized() };
    let mut len = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let res = unsafe {
        libc::accept4(
            listener_fd,
            &mut storage as *mut _ as *mut _,
            &mut len,
            libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
        )
    };
    let sock = cvt(res)?;
    let addr = super::sockaddr_to_addr(&storage, len as usize)?;
    Ok((sock, addr))
}
