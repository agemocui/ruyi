use std::cell::UnsafeCell;
use std::fmt;
use std::io;
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use std::rc::Rc;

use libc;

use sys::nio::{Borrow, BorrowMut};
use sys::unix::err::cvt;

pub(in sys) use sys::unix::nio_imp::{get_ready_tasks, Nio};

#[repr(C)]
pub struct IoVec {
    inner: libc::iovec,
}

impl From<(*const u8, usize)> for IoVec {
    #[inline]
    fn from(slice: (*const u8, usize)) -> Self {
        IoVec {
            inner: libc::iovec {
                iov_base: slice.0 as *mut _,
                iov_len: slice.1,
            },
        }
    }
}

impl fmt::Debug for IoVec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let base: usize = unsafe { mem::transmute(self.inner.iov_base) };
        write!(
            f,
            "{{ iov_base: 0x{:08x}, iov_len: {} }}",
            base, self.inner.iov_len as usize
        )
    }
}

#[inline]
fn readv(fd: RawFd, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    let res = unsafe { libc::readv(fd, iov_ptr as *const libc::iovec, len as libc::c_int) };
    let n = cvt(res)? as usize;
    Ok((n))
}

#[inline]
fn writev(fd: RawFd, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
    let res = unsafe { libc::writev(fd, iov_ptr as *const libc::iovec, len as libc::c_int) };
    let n = cvt(res)? as usize;
    Ok((n))
}

pub trait Readv {
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize>;
}

pub trait Writev {
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize>;
}

impl<R: AsRawFd> Readv for R {
    #[inline]
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        readv(self.as_raw_fd(), iovs.as_ptr() as *const IoVec, iovs.len())
    }
}

impl<W: AsRawFd> Writev for W {
    #[inline]
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        writev(self.as_raw_fd(), iovs.as_ptr() as *const IoVec, iovs.len())
    }
}

impl<H, T> Borrow<Nio<H, T>> for Rc<UnsafeCell<Nio<H, T>>>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    fn borrow(&self) -> &Nio<H, T> {
        unsafe { &*self.as_ref().get() }
    }
}

impl<H, T> BorrowMut<Nio<H, T>> for Rc<UnsafeCell<Nio<H, T>>>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    fn borrow_mut(&mut self) -> &mut Nio<H, T> {
        unsafe { &mut *self.as_ref().get() }
    }
}
