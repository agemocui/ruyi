#[macro_use]
extern crate log;

#[cfg(unix)]
extern crate libc;

extern crate net2;
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
    extern crate unreachable;
    unsafe { unreachable::unreachable() }
}

use std::fmt::Display;
use std::io;

#[inline]
fn other_io_err<E: Display>(e: E) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e.to_string())
}

pub mod slab;
pub mod buf;
pub mod nio;
pub mod reactor;
pub mod channel;
pub mod net;
