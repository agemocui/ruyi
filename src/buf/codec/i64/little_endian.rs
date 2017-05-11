use std::io::{self, Error};

use super::super::u64;
use super::super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<i64> {
    match u64::little_endian::read(chain) {
        Ok(r) => Ok(r as i64),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::little_endian::read")),
    }
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<i64> {
    match u64::little_endian::get(chain) {
        Ok(r) => Ok(r as i64),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::little_endian::get")),
    }
}

#[inline]
pub fn set(v: i64, chain: &mut SetIter) -> io::Result<usize> {
    match u64::little_endian::set(v as u64, chain) {
        Ok(n) => Ok(n),
        Err(e) => Err(Error::new(e.kind(), "codec::i64::little_endian::set")),
    }
}

#[inline]
pub fn append(v: i64, chain: &mut Appender) -> io::Result<usize> {
    u64::little_endian::append(v as u64, chain)
}

#[inline]
pub fn prepend(v: i64, chain: &mut Prepender) -> io::Result<usize> {
    u64::little_endian::prepend(v as u64, chain)
}
