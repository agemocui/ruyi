use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::Ordering;

use sys::AtomicBool;
use sys::unix::awakener_imp;

#[derive(Debug)]
pub(crate) struct Awakener {
    need_wakeup: AtomicBool,
    inner: awakener_imp::Awakener,
}

impl Awakener {
    #[inline]
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Awakener {
            need_wakeup: AtomicBool::new(true),
            inner: awakener_imp::Awakener::new()?,
        })
    }

    pub(crate) fn wakeup(&self) -> io::Result<()> {
        // Ordering::SeqCst ensures all the write-ops before this
        // need_wakeup.load since last wakeup will be seen by all
        // the read-ops after the need_wakeup.store in reset thread.
        if self.need_wakeup.load(Ordering::SeqCst) == true {
            self.inner.wakeup()?;
            self.need_wakeup.store(false, Ordering::SeqCst);
        }
        Ok(())
    }

    pub(crate) fn reset(&self) -> io::Result<()> {
        if !self.need_wakeup.load(Ordering::SeqCst) {
            self.inner.reset()?;
            self.need_wakeup.store(true, Ordering::SeqCst);
        }
        Ok(())
    }
}

impl AsRawFd for Awakener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}
