use std::io;
use std::os::unix::io::{AsRawFd, RawFd};

use libc;
use sys::unix::err::cvt;

#[derive(Debug)]
pub struct AwakenerImpl {
    event_fd: RawFd,
}

impl AwakenerImpl {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let res = unsafe { libc::eventfd(0, libc::EFD_CLOEXEC | libc::EFD_NONBLOCK) };
        Ok(AwakenerImpl {
            event_fd: cvt(res)?,
        })
    }

    #[inline]
    pub fn wakeup(&self) -> io::Result<()> {
        const NUM: *const u64 = &(::std::u64::MAX - 1);
        let res = unsafe { libc::write(self.event_fd, NUM as *const libc::c_void, 8) };
        if res == 8 {
            cvt(res).map(drop)
        } else {
            error!("Error to write 8 bytes to {:?}, written: {}", self, res);
            Ok(())
        }
    }

    #[inline]
    pub fn reset(&self) -> io::Result<()> {
        let mut data = 0u64;
        let buf = &mut data as *mut u64;
        let res = unsafe { libc::read(self.event_fd, buf as *mut libc::c_void, 8) };
        if res == 8 {
            cvt(res).map(drop)
        } else {
            error!("Error to read 8 bytes from {:?}, read: {}", self, res);
            Ok(())
        }
    }
}

impl AsRawFd for AwakenerImpl {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.event_fd
    }
}

impl Drop for AwakenerImpl {
    fn drop(&mut self) {
        let res = unsafe { libc::close(self.event_fd) };
        cvt(res)
            .map(drop)
            .unwrap_or_else(|e| error!("Failed to close {:?}: {}", self, e));
    }
}
