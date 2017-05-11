use std::io::{self, Error};

use super::super::u32;
use super::super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i32> {
    match u32::big_endian::read(chain) {
        Ok(r) => Ok(r as i32),
        Err(e) => Err(Error::new(e.kind(), "codec::i32::big_endian::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i32> {
    match u32::big_endian::get(chain) {
        Ok(r) => Ok(r as i32),
        Err(e) => Err(Error::new(e.kind(), "codec::i32::big_endian::get")),
    }
}

#[inline]
pub fn set(v: i32, chain: &mut SetIter) -> io::Result<usize> {
    match u32::big_endian::set(v as u32, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i32::big_endian::set")),
    }
}

#[inline]
pub fn append(v: i32, chain: &mut Appender) -> io::Result<usize> {
    u32::big_endian::append(v as u32, chain)
}

#[inline]
pub fn prepend(v: i32, chain: &mut Prepender) -> io::Result<usize> {
    u32::big_endian::prepend(v as u32, chain)
}
