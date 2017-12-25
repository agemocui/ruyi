use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::u16;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<i16, Error> {
    u16::varint::read(chain).map(|v| v as i16)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<i16, Error> {
    u16::varint::get(chain).map(|v| v as i16)
}

#[inline]
pub fn set(v: i16, chain: &mut SetIter) -> Result<usize, Error> {
    u16::varint::set(v as u16, chain)
}

#[inline]
pub fn append(v: i16, chain: &mut Appender) -> Result<usize, ()> {
    u16::varint::append(v as u16, chain)
}

#[inline]
pub fn prepend(v: i16, chain: &mut Prepender) -> Result<usize, ()> {
    u16::varint::prepend(v as u16, chain)
}
