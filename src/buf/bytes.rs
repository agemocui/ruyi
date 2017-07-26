use std::slice::{self, Iter};

use super::{Block, ByteBuf};

pub struct Bytes<'a> {
    iter_inner: Iter<'a, Block>,
    iter_u8: Option<Iter<'a, u8>>,
}

#[inline]
pub(super) fn new<'a>(buf: &'a ByteBuf) -> Bytes<'a> {
    let mut iter_inner = (&buf.blocks[buf.idx..]).iter();
    let iter_u8 = match iter_inner.next() {
        Some(inner) => {
            let ptr = inner.ptr_at(inner.read_pos());
            Some(unsafe { slice::from_raw_parts(ptr, inner.len()) }.iter())
        }
        None => None,
    };
    Bytes {
        iter_inner,
        iter_u8,
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter_u8.as_mut() {
                Some(iter_u8) => if let Some(b) = iter_u8.next() {
                    return Some(*b);
                },
                None => return None,
            }
            self.iter_u8 = match self.iter_inner.next() {
                Some(inner) => Some(inner.as_bytes().iter()),
                None => None,
            };
        }
    }
}
