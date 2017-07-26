use std::io::{self, Error};

use super::super::u64;
use super::super::super::{Appender, GetIter, Prepender, ReadIter, SetIter};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i64> {
    match u64::big_endian::read(chain) {
        Ok(r) => Ok(r as i64),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::big_endian::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i64> {
    match u64::big_endian::get(chain) {
        Ok(r) => Ok(r as i64),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::big_endian::get")),
    }
}

#[inline]
pub fn set(v: i64, chain: &mut SetIter) -> io::Result<usize> {
    match u64::big_endian::set(v as u64, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::big_endian::set")),
    }
}

#[inline]
pub fn append(v: i64, chain: &mut Appender) -> io::Result<usize> {
    u64::big_endian::append(v as u64, chain)
}

#[inline]
pub fn prepend(v: i64, chain: &mut Prepender) -> io::Result<usize> {
    u64::big_endian::prepend(v as u64, chain)
}
