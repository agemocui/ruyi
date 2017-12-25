use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::u64;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<i64, Error> {
    u64::varint::read(chain).map(|v| v as i64)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<i64, Error> {
    u64::varint::get(chain).map(|v| v as i64)
}

#[inline]
pub fn set(v: i64, chain: &mut SetIter) -> Result<usize, Error> {
    u64::varint::set(v as u64, chain)
}

#[inline]
pub fn append(v: i64, chain: &mut Appender) -> Result<usize, ()> {
    u64::varint::append(v as u64, chain)
}

#[inline]
pub fn prepend(v: i64, chain: &mut Prepender) -> Result<usize, ()> {
    u64::varint::prepend(v as u64, chain)
}
