use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use libc;
use sys::unix::err::cvt;
use sys::unix::syscall::pipe;

#[derive(Debug)]
pub struct Awakener {
    r_fd: RawFd,
    w_fd: RawFd,
}

impl Awakener {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let (r_fd, w_fd) = pipe()?;
        Ok(Awakener { r_fd, w_fd })
    }

    #[inline]
    pub fn wakeup(&self) -> io::Result<()> {
        let byte = 0u8;
        let res = unsafe { libc::write(self.w_fd, &byte as *const _ as *const _, 1) };
        match res {
            1 => Ok(()),
            n if n < 0 => Err(io::Error::last_os_error()),
            _ => {
                error!("Error to wakeup {:?}, written: {}", self, res);
                Ok(())
            }
        }
    }

    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        let mut buf = 0u8;
        let res = unsafe { libc::read(self.r_fd, &mut buf as *mut _ as *mut _, 1) };
        match res {
            1 => Ok(()),
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
        self.r_fd
    }
}

impl Drop for Awakener {
    fn drop(&mut self) {
        let mut res = unsafe { libc::close(self.w_fd) };
        cvt(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close pipe w_fd {:?}: {}", self, e));
        res = unsafe { libc::close(self.r_fd) };
        cvt(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close pipe r_fd {:?}: {}", self, e));
    }
}
