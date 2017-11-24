#![cfg_attr(nightly, feature(attr_literals))]
#![cfg_attr(nightly, feature(repr_align))]

#[macro_use]
extern crate log;

#[cfg(unix)]
extern crate libc;

#[cfg(windows)]
extern crate kernel32;
#[cfg(windows)]
extern crate winapi;
#[cfg(windows)]
extern crate ws2_32;

#[macro_use]
extern crate bitflags;

extern crate net2;

#[macro_use]
extern crate futures;

#[inline]
pub fn unreachable() -> ! {
    if cfg!(debug_assertions) {
        unreachable!();
    } else {
        extern crate unreachable;
        unsafe { unreachable::unreachable() }
    }
}

use std::time::Duration;

pub fn into_millis(dur: Duration) -> u64 {
    const NANOS_PER_MILLI: u32 = 1_000_000;
    const MILLIS_PER_SEC: u64 = 1_000;

    let millis = (dur.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI;
    dur.as_secs()
        .wrapping_mul(MILLIS_PER_SEC)
        .wrapping_add(millis as u64)
}

pub mod future;
pub mod stream;

pub mod slab;

mod task;
pub use self::task::*;

mod sys;

pub mod buf;
pub mod channel;
pub mod reactor;
pub mod net;

pub mod proto;
pub mod service;
