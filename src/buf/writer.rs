use std::io::{self, Write};
use std::ptr;

use super::ByteBuf;

pub struct Writer<'a> {
    inner: &'a mut ByteBuf,
}

#[inline]
pub(super) fn new<'a>(inner: &'a mut ByteBuf) -> Writer<'a> {
    Writer { inner }
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut n = buf.len();
        let mut src_dst = buf.as_ptr();
        loop {
            if let Some(block) = self.inner.last_mut() {
                let dst_off = block.write_pos();
                let appendable = block.appendable();
                if appendable >= n {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            src_dst,
                            block.as_mut_ptr().offset(dst_off as isize),
                            n,
                        );
                    }
                    block.set_write_pos(dst_off + n);
                    return Ok(buf.len());
                } else {
                    unsafe {
                        ptr::copy_nonoverlapping(
                            src_dst,
                            block.as_mut_ptr().offset(dst_off as isize),
                            appendable,
                        );
                        src_dst = src_dst.offset(appendable as isize);
                    }
                    n -= appendable;
                    let cap = block.capacity();
                    block.set_write_pos(cap);
                }
            }
            self.inner.append_block(0);
        }
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
