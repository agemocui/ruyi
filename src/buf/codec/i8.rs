use std::io::{self, Error};

use super::u8;
use super::super::{Appender, GetIter, Prepender, ReadIter, SetIter};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i8> {
    match u8::read(chain) {
        Ok(r) => Ok(r as i8),
        Err(e) => Err(Error::new(e.kind(), "codec::i8::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i8> {
    match u8::get(chain) {
        Ok(r) => Ok(r as i8),
        Err(e) => Err(Error::new(e.kind(), "codec::i8::get")),
    }
}

#[inline]
pub fn set(v: i8, chain: &mut SetIter) -> io::Result<usize> {
    match u8::set(v as u8, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i8::set")),
    }
}

#[inline]
pub fn append(v: i8, chain: &mut Appender) -> io::Result<usize> {
    u8::append(v as u8, chain)
}

#[inline]
pub fn prepend(v: i8, chain: &mut Prepender) -> io::Result<usize> {
    u8::prepend(v as u8, chain)
}
