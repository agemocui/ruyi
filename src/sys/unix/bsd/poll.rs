use std::{io, mem, ptr};
use std::os::unix::io::RawFd;
use std::time::Duration;

use libc;
use sys::unix::err::cvt;

type FilterType = libc::int16_t;

pub enum Filter {
    Read = libc::EVFILT_READ as isize,
    Write = libc::EVFILT_WRITE as isize,
}

#[repr(C)]
pub struct Event {
    inner: libc::kevent,
}

impl Event {
    #[inline]
    fn new(
        ident: RawFd,
        filter: libc::int16_t,
        flags: libc::uint16_t,
        fflags: libc::uint32_t,
        data: libc::intptr_t,
        token: usize,
    ) -> Self {
        Event {
            inner: libc::kevent {
                ident: ident as libc::uintptr_t,
                filter,
                flags,
                fflags,
                data,
                udata: unsafe { mem::transmute(token) },
            },
        }
    }

    #[inline]
    pub fn token(&self) -> usize {
        unsafe { mem::transmute(self.inner.udata) }
    }

    #[inline]
    pub(in sys::unix) fn is_read(&self) -> bool {
        self.inner.filter == libc::EVFILT_READ
    }
}

#[derive(Debug)]
pub struct Poller {
    kq: RawFd,
}

impl Poller {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let kq = cvt(unsafe { libc::kqueue() })?;
        Ok(Poller { kq })
    }

    #[inline]
    pub fn poll(&self, events: &mut [Event], timeout: Option<Duration>) -> io::Result<usize> {
        let to = timeout.map(|dur| {
            libc::timespec {
                tv_sec: dur.as_secs() as libc::time_t,
                tv_nsec: dur.subsec_nanos() as libc::c_long,
            }
        });
        let to_ptr = match to.as_ref() {
            Some(s) => s as *const libc::timespec,
            None => ptr::null(),
        };
        let res = unsafe {
            libc::kevent(
                self.kq,
                ptr::null(),
                0,
                events.as_mut_ptr() as *mut libc::kevent,
                events.len() as libc::c_int,
                to_ptr,
            )
        };
        Ok(cvt(res)? as usize)
    }

    #[inline]
    pub(super) fn register<T>(&self, fd: RawFd, filter: Filter, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let event = Event::new(fd, filter as FilterType, libc::EV_ADD, 0, 0, token.into());
        let res = unsafe {
            libc::kevent(
                self.kq,
                &event as *const Event as *const libc::kevent,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        cvt(res)?;
        Ok(())
    }

    #[inline]
    pub(super) fn deregister<T>(&self, fd: RawFd, filter: Filter, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let event = Event::new(
            fd,
            filter as FilterType,
            libc::EV_DELETE,
            0,
            0,
            token.into(),
        );
        let res = unsafe {
            libc::kevent(
                self.kq,
                &event as *const Event as *const libc::kevent,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        cvt(res)?;
        Ok(())
    }
}
