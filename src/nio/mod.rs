mod sys;

mod poll;
pub use self::poll::{Ops, Token, Event, Pollable, Poller};

mod awakener;
pub use self::awakener::*;

use std::io;

#[repr(C)]
#[derive(Debug)]
pub struct IoVec {
    inner: sys::IoVec,
}

impl IoVec {
    #[inline]
    pub fn from_mut(base: &mut u8, len: usize) -> Self {
        IoVec { inner: sys::IoVec::from_mut(base, len) }
    }

    #[inline]
    pub fn from(base: &u8, len: usize) -> Self {
        IoVec { inner: sys::IoVec::from(base, len) }
    }
}

pub trait ReadV {
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize>;
}

pub trait WriteV {
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize>;
}

impl<'a, R: ReadV> ReadV for &'a mut R {
    #[inline]
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        ReadV::readv(*self, iovs)
    }
}

impl<'a, W: WriteV> WriteV for &'a mut W {
    #[inline]
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        WriteV::writev(*self, iovs)
    }
}

mod tcp;
pub use self::tcp::*;
