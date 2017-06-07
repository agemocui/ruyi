use std::iter::Iterator;

use super::{ByteBuf, Inner};

pub struct Window<'a> {
    blocks: &'a [Inner],
    off: usize,
    size: usize,
}

impl<'a> PartialEq<&'a [u8]> for Window<'a> {
    #[inline]
    fn eq(&self, other: &&[u8]) -> bool {
        let mut that = *other;
        if self.size != that.len() {
            return false;
        }
        let mut off = self.off;
        for block in self.blocks {
            off += block.read_pos();
            let len = block.write_pos() - off;
            if len >= that.len() {
                let bytes = block.get_bytes(off, that.len());
                return bytes == that;
            }
            let (left, right) = that.split_at(len);
            if block.get_bytes(off, len) != left {
                return false;
            }
            that = right;
            off = 0;
        }
        false
    }
}

impl<'a> PartialEq<Window<'a>> for &'a [u8] {
    #[inline]
    fn eq(&self, other: &Window) -> bool {
        if self.len() != other.size {
            return false;
        }
        let mut off = other.off;
        let mut this = *self;
        for block in other.blocks {
            off += block.read_pos();
            let len = block.write_pos() - off;
            if len >= this.len() {
                let bytes = block.get_bytes(off, this.len());
                return bytes == this;
            }
            let (left, right) = this.split_at(len);
            if block.get_bytes(off, len) != left {
                return false;
            }
            this = right;
            off = 0;
        }
        false
    }
}

pub struct Windows<'a> {
    blocks: &'a [Inner],
    off: usize,
    size: usize, // window size
}

pub fn new<'a>(buf: &'a ByteBuf, size: usize) -> Windows<'a> {
    let blocks: &[Inner] = &buf.blocks[buf.pos..];
    Windows {
        blocks,
        off: 0,
        size,
    }
}

impl<'a> Iterator for Windows<'a> {
    type Item = Window<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.blocks.first() {
                Some(block) => {
                    if block.len() > self.off {
                        let window = Window {
                            blocks: self.blocks,
                            off: self.off,
                            size: self.size,
                        };
                        self.off += 1;
                        return Some(window);
                    }
                }
                None => return None,
            }
            self.blocks = &self.blocks[1..];
            self.off = 0;
        }
    }

    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        let mut i = 0;
        for block in self.blocks {
            if n < block.len() {
                break;
            }
            n -= block.len();
            i += 1;
        }
        self.blocks = &self.blocks[i..];
        self.off = n;
        self.next()
    }
}
