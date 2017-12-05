use std::io;
use std::mem;
use std::net::SocketAddr;
use std::os::unix::io::RawFd;

use libc;

use sys::unix::err::cvt;

#[inline]
fn set_cloexec_and_nonblock(fd: RawFd) -> io::Result<()> {
    let mut res = unsafe { libc::fcntl(fd, libc::F_SETFD, libc::O_CLOEXEC) };
    cvt(res)?;
    res = unsafe { libc::fcntl(fd, libc::F_SETFL, libc::O_NONBLOCK) };
    cvt(res)?;
    Ok(())
}

#[inline]
#[allow(dead_code)]
pub fn pipe() -> io::Result<(RawFd, RawFd)> {
    let mut fds = [0 as libc::c_int; 2];
    cvt(unsafe { libc::pipe(fds.as_mut_ptr()) })?;
    set_cloexec_and_nonblock(fds[0])?;
    set_cloexec_and_nonblock(fds[1])?;
    Ok((fds[0], fds[1]))
}

#[inline]
pub fn socket_v4() -> io::Result<RawFd> {
    let res = unsafe { libc::socket(libc::AF_INET, libc::SOCK_STREAM, 0) };
    let fd = cvt(res)?;
    set_cloexec_and_nonblock(fd)?;
    Ok(fd)
}

#[inline]
pub fn socket_v6() -> io::Result<RawFd> {
    let res = unsafe { libc::socket(libc::AF_INET6, libc::SOCK_STREAM, 0) };
    let fd = cvt(res)?;
    set_cloexec_and_nonblock(fd)?;
    Ok(fd)
}

#[inline]
pub fn accept(listener_fd: RawFd) -> io::Result<(RawFd, SocketAddr)> {
    let mut storage: libc::sockaddr_storage = unsafe { mem::uninitialized() };
    let mut len = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let res =
        unsafe { libc::accept(listener_fd, &mut storage as *mut _ as *mut _, &mut len) };
    let sock = cvt(res)?;
    set_cloexec_and_nonblock(sock)?;
    let addr = super::sockaddr_to_addr(&storage, len as usize)?;
    Ok((sock, addr))
}
