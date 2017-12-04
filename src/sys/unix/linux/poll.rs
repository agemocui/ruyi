use std::io;
use std::os::unix::io::RawFd;
use std::ptr;
use std::time::Duration;

use libc;
use sys::unix::err::cvt;

bitflags! {
    pub(super) struct Ops: u32 {
        const READ      = libc::EPOLLIN as u32;
        const WRITE     = libc::EPOLLOUT as u32;
    }
}

#[repr(C)]
pub struct Event {
    inner: libc::epoll_event,
}

impl Event {
    #[inline]
    fn new(ops: Ops, token: usize) -> Self {
        Event {
            inner: libc::epoll_event {
                events: ops.bits(),
                u64: token as u64,
            },
        }
    }

    #[inline]
    pub fn token(&self) -> usize {
        self.inner.u64 as usize
    }

    #[inline]
    pub(in sys::unix) fn is_read(&self) -> bool {
        (self.inner.events & Ops::READ.bits()) == Ops::READ.bits()
    }

    #[inline]
    pub(in sys::unix) fn is_write(&self) -> bool {
        (self.inner.events & Ops::WRITE.bits()) == Ops::WRITE.bits()
    }
}

#[derive(Debug)]
pub struct Poller {
    epfd: RawFd,
}

impl Poller {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let res = unsafe { libc::epoll_create1(libc::EPOLL_CLOEXEC) };
        Ok(Poller { epfd: cvt(res)? })
    }

    #[inline]
    pub fn poll(&self, events: &mut [Event], timeout: Option<Duration>) -> io::Result<usize> {
        let event_ptr = events.as_mut_ptr() as *mut libc::epoll_event;
        let len = events.len() as libc::c_int;
        let millis = match timeout {
            Some(dur) => ::into_millis(dur) as libc::c_int,
            None => -1,
        };
        let res = unsafe { libc::epoll_wait(self.epfd, event_ptr, len, millis) };
        Ok(cvt(res)? as usize)
    }

    #[inline]
    pub(super) fn register<T>(&self, fd: RawFd, ops: Ops, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let mut ev = Event::new(ops, token.into());
        let res =
            unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_ADD, fd, &mut ev.inner) };
        cvt(res).map(drop)
    }

    #[inline]
    pub(super) fn reregister<T>(&self, fd: RawFd, ops: Ops, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let mut ev = Event::new(ops, token.into());
        let res =
            unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_MOD, fd, &mut ev.inner) };
        cvt(res).map(drop)
    }

    #[inline]
    pub(super) fn deregister(&self, fd: RawFd) -> io::Result<()> {
        let res =
            unsafe { libc::epoll_ctl(self.epfd, libc::EPOLL_CTL_DEL, fd, ptr::null_mut()) };
        cvt(res).map(drop)
    }
}
