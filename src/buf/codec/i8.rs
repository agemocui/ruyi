use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::u8;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<i8, Error> {
    u8::read(chain).map(|v| v as i8)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<i8, Error> {
    u8::get(chain).map(|v| v as i8)
}

#[inline]
pub fn set(v: i8, chain: &mut SetIter) -> Result<usize, Error> {
    u8::set(v as u8, chain)
}

#[inline]
pub fn append(v: i8, chain: &mut Appender) -> Result<usize, ()> {
    u8::append(v as u8, chain)
}

#[inline]
pub fn prepend(v: i8, chain: &mut Prepender) -> Result<usize, ()> {
    u8::prepend(v as u8, chain)
}
