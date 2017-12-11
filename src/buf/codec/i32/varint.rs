use buf::{Appender, BufError, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::u32;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<i32, BufError> {
    u32::varint::read(chain).map(|v| v as i32)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<i32, BufError> {
    u32::varint::get(chain).map(|v| v as i32)
}

#[inline]
pub fn set(v: i32, chain: &mut SetIter) -> Result<usize, BufError> {
    u32::varint::set(v as u32, chain)
}

#[inline]
pub fn append(v: i32, chain: &mut Appender) -> Result<usize, ()> {
    u32::varint::append(v as u32, chain)
}

#[inline]
pub fn prepend(v: i32, chain: &mut Prepender) -> Result<usize, ()> {
    u32::varint::prepend(v as u32, chain)
}
