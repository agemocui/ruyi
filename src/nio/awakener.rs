use std::fmt;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};

use super::sys;
use super::poll::{Ops, Token, Pollable, Poller};

pub struct Awakener {
    inner: sys::Awakener,
    _padding0: [usize; cache_line_pad!(1)],

    need_wakeup: AtomicBool,
    _padding1: [u8; word_len!() - 1],

    _padding2: [usize; cache_line_pad!(0)],
}

impl Awakener {
    #[inline]
    pub fn new() -> io::Result<Self> {
        Ok(Awakener {
            inner: sys::Awakener::new()?,
            _padding0: [0; cache_line_pad!(1)],
            need_wakeup: AtomicBool::new(true),
            _padding1: [0; word_len!() - 1],
            _padding2: [0; cache_line_pad!(0)],
        })
    }

    pub fn wakeup(&self) -> io::Result<()> {
        // Ordering::SeqCst ensures all the write-ops before this
        // need_wakeup.load since last wakeup will be seen by all
        // the read-ops after the need_wakeup.store in reset thread.
        if self.need_wakeup.load(Ordering::SeqCst) == true {
            self.inner.wakeup()?;
            self.need_wakeup.store(false, Ordering::Relaxed);
        }
        Ok(())
    }

    pub fn wakeup_m(&self) -> io::Result<()> {
        match self.need_wakeup
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => self.inner.wakeup(),
            _ => Ok(()),
        }
    }

    pub fn reset(&self) -> io::Result<()> {
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

impl Pollable for Awakener {
    #[inline]
    fn register(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.register(poller, interest, token)
    }

    #[inline]
    fn reregister(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.reregister(poller, interest, token)
    }

    #[inline]
    fn deregister(&self, poller: &Poller) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

impl fmt::Debug for Awakener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Awakener {{ inner: {:?}, need_wakeup: {:?} }}",
            self.inner,
            self.need_wakeup
        )
    }
}
