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
    if cfg!(debug_assertions) {
        unreachable!();
    } else {
        extern crate unreachable;
        unsafe { unreachable::unreachable() }
    }
}

use std::fmt::Display;
use std::io::{Error, ErrorKind};

#[inline]
fn other_io_err<E: Display>(e: E) -> Error {
    Error::new(ErrorKind::Other, e.to_string())
}

pub mod slab;
pub mod buf;
pub mod nio;
pub mod io;
pub mod reactor;
pub mod channel;
pub mod net;
