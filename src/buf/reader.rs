use std::io::{self, Read};
use std::ptr;

use super::{read, ReadBlock, ByteBuf};

pub struct Reader<'a> {
    inner: &'a mut ByteBuf,
}

#[inline]
pub(super) fn new<'a>(inner: &'a mut ByteBuf) -> Reader<'a> {
    Reader { inner }
}

impl<'a> Iterator for Reader<'a> {
    type Item = ReadBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.inner.pos() < self.inner.num_of_blocks() {
            let i = self.inner.pos();
            let block = unsafe { &mut *self.inner.mut_ptr_at(i) };
            if !block.is_empty() {
                return Some(read::new(block));
            }
            self.inner.inc_pos();
        }
        None
    }
}

impl<'a> Read for Reader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut n = buf.len();
        let mut dst_ptr = buf.as_mut_ptr();
        for mut block in self {
            let len = block.len();
            let src_off = block.read_pos();
            if len >= n {
                unsafe {
                    ptr::copy_nonoverlapping(block.as_ptr().offset(src_off as isize), dst_ptr, n);
                }
                block.set_read_pos(src_off + n);
                return Ok(buf.len());
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(block.as_ptr().offset(src_off as isize), dst_ptr, len);
                    dst_ptr = dst_ptr.offset(len as isize);
                }
                n -= len;
                let write_pos = block.write_pos();
                block.set_read_pos(write_pos);
            }
        }
        Ok(buf.len() - n)
    }
}
