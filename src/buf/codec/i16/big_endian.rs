use std::io::{self, Error};

use super::super::u16;
use super::super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i16> {
    match u16::big_endian::read(chain) {
        Ok(r) => Ok(r as i16),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::big_endian::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i16> {
    match u16::big_endian::get(chain) {
        Ok(r) => Ok(r as i16),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::big_endian::get")),
    }
}

#[inline]
pub fn set(v: i16, chain: &mut SetIter) -> io::Result<usize> {
    match u16::big_endian::set(v as u16, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i16::big_endian::set")),
    }
}

#[inline]
pub fn append(v: i16, chain: &mut Appender) -> io::Result<usize> {
    u16::big_endian::append(v as u16, chain)
}

#[inline]
pub fn prepend(v: i16, chain: &mut Prepender) -> io::Result<usize> {
    u16::big_endian::prepend(v as u16, chain)
}
