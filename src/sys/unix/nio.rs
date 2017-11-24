use std::cell::UnsafeCell;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::os::unix::io::{AsRawFd, RawFd};
use std::rc::Rc;

use libc;

use reactor::CURRENT_LOOP;
use task::TaskId;
use sys::{ReadyTasks, Token};
use sys::nio::{Borrow, BorrowMut};
use sys::unix::err::cvt;
use sys::unix::poll::{Event, Ops};

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
            base,
            self.inner.iov_len as usize
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

pub struct Nio<H: AsRawFd + fmt::Debug, T: AsRef<H> = H> {
    io: T,
    token: Token,
    interested_ops: Ops,
    _marker: PhantomData<H>,
}

impl<H, T> Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        let token = Self::register(io.as_ref().as_raw_fd())?;
        Ok(Nio {
            io,
            token,
            interested_ops: Ops::empty(),
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.io
    }

    #[inline]
    pub fn schedule_read(&mut self) -> io::Result<()> {
        if self.interested_ops.contains(Ops::READ) {
            Ok(self.reschedule(Ops::READ))
        } else {
            let ops = self.interested_ops | Ops::READ;
            self.interest_ops(ops)
        }
    }

    #[inline]
    pub fn schedule_write(&mut self) -> io::Result<()> {
        if self.interested_ops.contains(Ops::WRITE) {
            Ok(self.reschedule(Ops::WRITE))
        } else {
            let ops = self.interested_ops | Ops::WRITE;
            self.interest_ops(ops)
        }
    }

    #[inline]
    pub fn cancel_write(&mut self) -> io::Result<()> {
        if self.interested_ops.contains(Ops::WRITE) {
            let ops = self.interested_ops - Ops::WRITE;
            self.interest_ops(ops)
        } else {
            Ok(())
        }
    }

    #[inline]
    pub fn is_read_ready(&self) -> bool {
        match self.interested_ops.contains(Ops::READ) {
            true => self.token.is_read_ready(),
            false => true,
        }
    }

    #[inline]
    pub fn is_write_ready(&self) -> bool {
        match self.interested_ops.contains(Ops::WRITE) {
            true => self.token.is_write_ready(),
            false => true,
        }
    }

    #[inline]
    fn register(fd: RawFd) -> io::Result<Token> {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            let token = eloop.schedule(Ops::empty());
            match eloop
                .as_poller()
                .register(fd, Ops::empty(), token.as_inner())
            {
                Ok(()) => Ok(token),
                Err(e) => {
                    eloop.cancel(&token);
                    Err(e)
                }
            }
        })
    }

    #[inline]
    fn reregister(fd: RawFd, ops: Ops, new_ops: Ops, token: &Token) -> io::Result<()> {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            eloop.as_poller().reregister(fd, ops, token.as_inner())?;
            eloop.reschedule(token, new_ops);
            Ok(())
        })
    }

    #[inline]
    fn deregister(fd: RawFd, token: &Token) -> io::Result<()> {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            eloop.as_poller().deregister(fd)?;
            eloop.cancel(token);
            Ok(())
        })
    }

    #[inline]
    fn interest_ops(&mut self, ops: Ops) -> io::Result<()> {
        let new_ops = ops - self.interested_ops;
        Self::reregister(self.io.as_ref().as_raw_fd(), ops, new_ops, &self.token)?;
        self.interested_ops = ops;
        return Ok(());
    }

    #[inline]
    fn reschedule(&self, ops: Ops) {
        CURRENT_LOOP.with(|eloop| {
            unsafe { eloop.as_mut() }
                .as_mut_inner()
                .reschedule(&self.token, ops);
        })
    }
}

impl<H, T> fmt::Debug for Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    fn fmt(&self, _f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: impl Debug for Nio
        unimplemented!()
    }
}

impl<H, T> Drop for Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    fn drop(&mut self) {
        Self::deregister(self.io.as_ref().as_raw_fd(), &self.token).unwrap_or_else(|e| {
            error!("Failed to deregister {:?}: {}", self.io.as_ref(), e)
        });
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

#[inline]
pub(in sys) fn get_ready_tasks(
    event: &Event,
    read_task: TaskId,
    write_task: TaskId,
) -> (ReadyTasks, Ops) {
    match event.is_read() {
        true => match event.is_write() {
            true => (ReadyTasks::Pair(read_task, write_task), Ops::all()),
            false => (ReadyTasks::Single(read_task), Ops::READ),
        },
        false => (ReadyTasks::Single(write_task), Ops::WRITE),
    }
}
