use std::io;
use std::os::unix::io::{RawFd, AsRawFd};
use std::time::Duration;
use std::ptr;

use libc;

use super::io_result;
use super::super::into_millis;

pub const OP_READ: usize = libc::EPOLLIN as usize;
pub const OP_WRITE: usize = libc::EPOLLOUT as usize;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    inner: libc::epoll_event,
}

impl Event {
    #[inline]
    fn new(ops: usize, token: usize) -> Self {
        Event {
            inner: libc::epoll_event {
                events: ops as u32,
                u64: token as u64,
            },
        }
    }

    #[inline]
    pub fn ops(&self) -> usize {
        self.inner.events as usize
    }

    #[inline]
    pub fn token(&self) -> usize {
        self.inner.u64 as usize
    }
}

#[derive(Debug)]
pub struct Selector {
    epfd: RawFd,
}

impl Selector {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let res = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        let epfd = io_result(res)?;
        Ok(Selector { epfd: epfd })
    }

    #[inline]
    pub fn select(&self,
                  event_ptr: *mut Event,
                  max_events: usize,
                  timeout: Option<Duration>)
                  -> io::Result<usize> {
        let events = event_ptr as *mut libc::epoll_event;
        let len = max_events as libc::c_int;
        let millis = match timeout {
            Some(dur) => into_millis(dur) as libc::c_int,
            None => -1,
        };
        let res = unsafe { libc::epoll_wait(self.epfd, events, len, millis) };
        let n = io_result(res)? as usize;
        Ok((n))
    }

    #[inline]
    pub fn register<O, T>(&self, fd: RawFd, interested_ops: O, token: T) -> io::Result<()>
        where O: Into<usize>,
              T: Into<usize>
    {
        let mut ev = Event::new(interested_ops.into(), token.into());
        let res = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut ev.inner) };
        io_result(res).map(drop)
    }

    #[inline]
    pub fn reregister<O, T>(&self, fd: RawFd, interested_ops: O, token: T) -> io::Result<()>
        where O: Into<usize>,
              T: Into<usize>
    {
        let mut ev = Event::new(interested_ops.into(), token.into());
        let res = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut ev.inner) };
        io_result(res).map(drop)
    }

    #[inline]
    pub fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let res = unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut()) };
        io_result(res).map(drop)
    }
}

impl Drop for Selector {
    #[inline]
    fn drop(&mut self) {
        let res = unsafe { libc::close(self.epfd) };
        io_result(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close {:?}: {}", self, e));
    }
}

#[derive(Debug)]
pub struct Awakener {
    event_fd: RawFd,
}

impl Awakener {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let res = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        Ok(Awakener { event_fd: io_result(res)? })
    }

    pub fn wakeup(&self) -> io::Result<()> {
        const NUM: *const u64 = &(::std::u64::MAX - 1);
        let res = unsafe { libc::write(self.event_fd, NUM as *const libc::c_void, 8) };
        if res == 8 {
            io_result(res).map(drop)
        } else {
            error!("Error to write 8 bytes to {:?}, writen: {}", self, res);
            Ok(())
        }
    }

    pub fn reset(&self) -> io::Result<()> {
        let mut data = 0u64;
        let buf = &mut data as *mut u64;
        let res = unsafe { libc::read(self.event_fd, buf as *mut libc::c_void, 8) };
        if res == 8 {
            io_result(res).map(drop)
        } else {
            error!("Error to read 8 bytes from {:?}, read: {}", self, res);
            Ok(())
        }
    }
}

impl AsRawFd for Awakener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.event_fd
    }
}

impl Drop for Awakener {
    fn drop(&mut self) {
        let res = unsafe { libc::close(self.event_fd) };
        io_result(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close {:?}: {}", self, e));
    }
}
