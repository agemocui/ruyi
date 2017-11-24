use std::sync::atomic::{self, Ordering};

#[cfg_attr(nightly, repr(align(64)))]
#[derive(Debug)]
struct AtomicBool(atomic::AtomicBool);

impl AtomicBool {
    #[inline]
    fn new(v: bool) -> Self {
        AtomicBool(atomic::AtomicBool::new(v))
    }

    #[inline]
    fn load(&self, order: Ordering) -> bool {
        self.0.load(order)
    }

    #[inline]
    fn store(&self, val: bool, order: Ordering) {
        self.0.store(val, order);
    }
}

mod nio;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub(crate) use self::windows::{net, Awakener, Recv};
#[cfg(windows)]
use self::windows::poll;
#[cfg(windows)]
use self::windows::nio::get_ready_tasks;

#[cfg(unix)]
mod unix;
#[cfg(unix)]
pub(crate) use self::unix::{net, Awakener, Recv};
#[cfg(unix)]
use self::unix::poll;
#[cfg(unix)]
use self::unix::nio::get_ready_tasks;

mod eloop;
pub(crate) use self::eloop::{EventLoop, ReadyTasks};
use self::eloop::*;
