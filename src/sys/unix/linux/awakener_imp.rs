use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use libc;
use sys::unix::err::cvt;

#[derive(Debug)]
pub struct Awakener {
    event_fd: RawFd,
}

impl Awakener {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let res = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        Ok(Awakener {
            event_fd: cvt(res)?,
        })
    }

    #[inline]
    pub fn wakeup(&self) -> io::Result<()> {
        let data = ::std::u64::MAX - 1;
        let res = unsafe { libc::write(self.event_fd, &data as *const _ as *const _, 8) };
        match res {
            8 => Ok(()),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => {
                error!("Error to wakeup {:?}, written: {}", self, res);
                Ok(())
            }
        }
    }

    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        let mut buf = 0u64;
        let res = unsafe { libc::read(self.event_fd, &mut buf as *mut _ as *mut _, 8) };
        match res {
            8 => Ok(()),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => {
                error!("Error to reset {:?}, written: {}", self, res);
                Ok(())
            }
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
    #[inline]
    fn drop(&mut self) {
        let res = unsafe { libc::close(self.event_fd) };
        cvt(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close {:?}: {}", self, e));
    }
}
