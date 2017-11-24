use std::io;
use std::os::unix::io::{AsRawFd, RawFd};
use std::sync::atomic::Ordering;

use sys::AtomicBool;
use sys::unix::AwakenerImpl;

#[derive(Debug)]
pub(crate) struct Awakener {
    need_wakeup: AtomicBool,
    inner: AwakenerImpl,
}

impl Awakener {
    #[inline]
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Awakener {
            need_wakeup: AtomicBool::new(true),
            inner: AwakenerImpl::new()?,
        })
    }

    pub(crate) fn wakeup(&self) -> io::Result<()> {
        // Ordering::SeqCst ensures all the write-ops before this
        // need_wakeup.load since last wakeup will be seen by all
        // the read-ops after the need_wakeup.store in reset thread.
        if self.need_wakeup.load(Ordering::SeqCst) == true {
            self.inner.wakeup()?;
            self.need_wakeup.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    pub(crate) fn reset(&self) -> io::Result<()> {
        // This awakener has to be registered to poll in level mode
        // so that if this need_wakeup.load sees true, which may happen
        // with Ordering::Relaxed, it will be re-triggered by next poll immediately.
        if !self.need_wakeup.load(Ordering::Relaxed) {
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
