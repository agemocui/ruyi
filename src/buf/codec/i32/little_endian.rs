use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::u32;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<i32, Error> {
    u32::little_endian::read(chain).map(|v| v as i32)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<i32, Error> {
    u32::little_endian::get(chain).map(|v| v as i32)
}

#[inline]
pub fn set(v: i32, chain: &mut SetIter) -> Result<usize, Error> {
    u32::little_endian::set(v as u32, chain)
}

#[inline]
pub fn append(v: i32, chain: &mut Appender) -> Result<usize, ()> {
    u32::little_endian::append(v as u32, chain)
}

#[inline]
pub fn prepend(v: i32, chain: &mut Prepender) -> Result<usize, ()> {
    u32::little_endian::prepend(v as u32, chain)
}
