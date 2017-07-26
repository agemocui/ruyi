use std::io::{self, Error};

use super::super::u16;
use super::super::super::{Appender, GetIter, Prepender, ReadIter, SetIter};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i16> {
    match u16::varint::read(chain) {
        Ok(r) => Ok(r as i16),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::varint::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i16> {
    match u16::varint::get(chain) {
        Ok(r) => Ok(r as i16),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::varint::get")),
    }
}

#[inline]
pub fn set(v: i16, chain: &mut SetIter) -> io::Result<usize> {
    match u16::varint::set(v as u16, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::varint::set")),
    }
}

#[inline]
pub fn append(v: i16, chain: &mut Appender) -> io::Result<usize> {
    u16::varint::append(v as u16, chain)
}

#[inline]
pub fn prepend(v: i16, chain: &mut Prepender) -> io::Result<usize> {
    u16::varint::prepend(v as u16, chain)
}
