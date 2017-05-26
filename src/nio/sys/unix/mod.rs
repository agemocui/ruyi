use std::io;
use std::mem;
use std::os::unix::io::{RawFd, AsRawFd};

use libc;

trait ErrRes {
    fn err_res() -> Self;
}

impl ErrRes for i32 {
    #[inline]
    fn err_res() -> Self {
        -1
    }
}

impl ErrRes for isize {
    #[inline]
    fn err_res() -> Self {
        -1
    }
}

#[inline]
fn io_result<V: ErrRes + PartialEq<V>>(v: V) -> io::Result<V> {
    if v != V::err_res() {
        Ok(v)
    } else {
        Err(io::Error::last_os_error())
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IoVec {
    inner: libc::iovec,
}

impl IoVec {
    #[inline]
    pub fn from_mut(base: *mut u8, len: usize) -> Self {
        IoVec {
            inner: libc::iovec {
                iov_base: base as *mut libc::c_void,
                iov_len: len,
            },
        }
    }

    #[inline]
    pub fn from(base: *const u8, len: usize) -> Self {
        IoVec {
            inner: libc::iovec {
                iov_base: unsafe { mem::transmute(base) },
                iov_len: len,
            },
        }
    }
}

impl fmt::Debug for IoVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let base: usize = unsafe { mem::transmute(self.inner.iov_base) };
        write!(f,
               "{{ iov_base: 0x{:08x}, iov_len: {} }}",
               base,
               self.inner.iov_len as usize)
    }
}

#[inline]
fn readv(fd: RawFd, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    let res = unsafe { libc::readv(fd, iov_ptr as *const libc::iovec, len as libc::c_int) };
    let n = io_result(res)? as usize;
    Ok((n))
}

#[inline]
fn writev(fd: RawFd, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    let res = unsafe { libc::writev(fd, iov_ptr as *const libc::iovec, len as libc::c_int) };
    let n = io_result(res)? as usize;
    Ok((n))
}

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;

#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::linux::*;

#[cfg(any(
    target_os = "freebsd", target_os = "netbsd", target_os = "openbsd",
    target_os = "ios", target_os = "macos",
    target_os = "bitrig", target_os = "dragonfly",
))]
mod bsd;

#[cfg(any(
    target_os = "freebsd", target_os = "netbsd", target_os = "openbsd",
    target_os = "ios", target_os = "macos",
    target_os = "bitrig", target_os = "dragonfly",
))]
pub use self::bsd::*;

mod tcp;

pub use self::tcp::*;

use std::fmt;
use super::super::poll::selector;
use super::super::{Pollable, Poller};
use super::super::{Ops, Token};

impl<F: AsRawFd + fmt::Debug> Pollable for F {
    #[inline]
    fn register(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()> {
        selector(poller).register(self.as_raw_fd(), interested_ops, token)
    }

    #[inline]
    fn reregister(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()> {
        selector(poller).reregister(self.as_raw_fd(), interested_ops, token)
    }

    #[inline]
    fn deregister(&self, poller: &Poller) -> io::Result<()> {
        selector(poller).deregister(self.as_raw_fd())
    }
}
