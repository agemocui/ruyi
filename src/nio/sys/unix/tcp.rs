use std::io;
use std::mem;
use std::net::{SocketAddr, TcpStream, TcpListener};
use std::os::unix::io::{AsRawFd, FromRawFd};

use libc;

use super::IoVec;

#[inline]
pub fn new_v4() -> io::Result<TcpStream> {
    let res = unsafe {
        libc::socket(libc::AF_INET,
                     libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                     0)
    };
    let fd = super::cvt(res)?;
    Ok(unsafe { TcpStream::from_raw_fd(fd) })
}

#[inline]
pub fn new_v6() -> io::Result<TcpStream> {
    let res = unsafe {
        libc::socket(libc::AF_INET6,
                     libc::SOCK_STREAM | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
                     0)
    };
    let fd = super::cvt(res)?;
    Ok(unsafe { TcpStream::from_raw_fd(fd) })
}

#[inline]
pub fn connect(addr: &SocketAddr) -> io::Result<(TcpStream, bool)> {
    let (sock, addr, len) = match *addr {
        SocketAddr::V4(ref a) => {
            (new_v4()?, a as *const _ as *const _, mem::size_of_val(a) as libc::socklen_t)
        }
        SocketAddr::V6(ref a) => {
            (new_v6()?, a as *const _ as *const _, mem::size_of_val(a) as libc::socklen_t)
        }
    };
    let res = unsafe { libc::connect(sock.as_raw_fd(), addr, len) };
    match super::cvt(res) {
        Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => Ok((sock, false)),
        Ok(..) => Ok((sock, true)),
        Err(e) => Err(e),
    }
}

#[inline]
pub fn readv(sock: &TcpStream, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    super::readv(sock.as_raw_fd(), iov_ptr, len)
}

#[inline]
pub fn writev(sock: &TcpStream, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    super::writev(sock.as_raw_fd(), iov_ptr, len)
}

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
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid argument")),
    }
}

#[inline]
pub fn accept(listener: &TcpListener) -> io::Result<(TcpStream, SocketAddr)> {
    let mut storage: libc::sockaddr_storage = unsafe { mem::uninitialized() };
    let mut len = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
    let res = unsafe {
        libc::accept4(listener.as_raw_fd(),
                      &mut storage as *mut _ as *mut _,
                      &mut len,
                      libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC)
    };
    let fd = super::cvt(res)?;
    let sock = unsafe { TcpStream::from_raw_fd(fd) };
    let addr = sockaddr_to_addr(&storage, len as usize)?;
    Ok((sock, addr))
}
