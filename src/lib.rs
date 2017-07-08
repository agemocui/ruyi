#[macro_use]
extern crate log;

#[cfg(unix)]
extern crate libc;

extern crate net2;

#[macro_use]
extern crate futures;

#[cfg(target_pointer_width = "32")]
macro_rules! word_len {
    () => { 4 }
}

#[cfg(target_pointer_width = "64")]
macro_rules! word_len {
    () => { 8 }
}

macro_rules! cache_line_pad { ($N:expr) => { 64 / word_len!() - 1 - $N }}

#[inline]
fn unreachable() -> ! {
    if cfg!(debug_assertions) {
        unreachable!();
    } else {
        extern crate unreachable;
        unsafe { unreachable::unreachable() }
    }
}

pub mod slab;
pub mod buf;
pub mod nio;
pub mod io;
pub mod reactor;
pub mod channel;
pub mod net;
pub mod proto;
pub mod service;
