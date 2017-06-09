use std::ptr;

use super::{ByteBuf, Block};

enum Off {
    Left(usize),
    Right(usize),
}

pub struct Window<'a> {
    blocks: &'a [Block],
    off: Off,
    size: usize,
}

impl<'a> Window<'a> {
    pub fn as_bytes(&self) -> Option<&[u8]> {
        if self.size == 0 {
            return Some(&[]);
        }
        match self.blocks.len() == 1 {
            true => {
                let b = unsafe { self.blocks.get_unchecked(0) };
                let bytes = match self.off {
                    Off::Left(off) => b.as_bytes_range(b.read_pos() + off, self.size),
                    Off::Right(off) => b.as_bytes_range(b.write_pos() - off - self.size, self.size),
                };
                Some(bytes)
            }
            false => None,
        }
    }

    #[inline]
    pub fn to_bytes(self) -> Vec<u8> {
        match self.off {
            Off::Left(off) => Self::to_bytes_left(self.blocks, off, self.size),
            Off::Right(off) => Self::to_bytes_right(self.blocks, off, self.size),
        }
    }

    fn to_bytes_left(blocks: &'a [Block], mut off: usize, mut size: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(size);
        unsafe { bytes.set_len(size) }
        let mut p_dst = bytes.as_mut_ptr();
        for block in blocks {
            off = block.read_pos() + off;
            let len = block.write_pos() - off;
            let p_src = block.ptr_at(off);
            if len >= size {
                unsafe {
                    ptr::copy_nonoverlapping(p_src, p_dst, size);
                }
                break;
            } else {
                unsafe {
                    ptr::copy_nonoverlapping(p_src, p_dst, len);
                    p_dst = p_dst.offset(len as isize);
                }
                size -= len;
                off = 0;
            }
        }
        bytes
    }

    fn to_bytes_right(blocks: &'a [Block], mut off: usize, mut size: usize) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(size);
        unsafe { bytes.set_len(size) }
        let mut p_dst = unsafe { bytes.as_mut_ptr().offset(size as isize) };
        for block in blocks.iter().rev() {
            off = block.write_pos() - off;
            let len = off - block.read_pos();
            if len >= size {
                let p_src = block.ptr_at(off - size);
                unsafe {
                    p_dst = p_dst.offset(-(size as isize));
                    ptr::copy_nonoverlapping(p_src, p_dst, size);
                }
                break;
            } else {
                let p_src = block.ptr_at(block.read_pos());
                unsafe {
                    p_dst = p_dst.offset(-(len as isize));
                    ptr::copy_nonoverlapping(p_src, p_dst, len);
                }
                size -= len;
                off = 0;
            }
        }
        bytes
    }

    #[inline]
    fn from_left(blocks: &'a [Block], off: usize, size: usize) -> Self {
        Window {
            blocks,
            off: Off::Left(off),
            size,
        }
    }

    #[inline]
    fn from_right(blocks: &'a [Block], off: usize, size: usize) -> Self {
        Window {
            blocks,
            off: Off::Right(off),
            size,
        }
    }

    #[inline]
    fn eq_left(&self, mut off: usize, mut other: &[u8]) -> bool {
        if self.size == other.len() {
            for block in self.blocks {
                off += block.read_pos();
                let len = block.write_pos() - off;
                if len >= other.len() {
                    let bytes = block.as_bytes_range(off, other.len());
                    return bytes == other;
                }
                let (left, right) = other.split_at(len);
                if block.as_bytes_range(off, len) != left {
                    return false;
                }
                other = right;
                off = 0;
            }
        }
        false
    }

    #[inline]
    fn eq_right(&self, mut off: usize, mut other: &[u8]) -> bool {
        if self.size == other.len() {
            for block in self.blocks.iter().rev() {
                off = block.write_pos() - off;
                let len = off - block.read_pos();
                if len >= other.len() {
                    let bytes = block.as_bytes_range(off - other.len(), other.len());
                    return bytes == other;
                }
                let (left, right) = other.split_at(other.len() - len);
                if block.as_bytes_range(block.read_pos(), len) != right {
                    return false;
                }
                other = left;
                off = 0;
            }
        }
        false
    }
}

impl<'a> PartialEq<&'a [u8]> for Window<'a> {
    fn eq(&self, other: &&[u8]) -> bool {
        match self.off {
            Off::Left(off) => self.eq_left(off, *other),
            Off::Right(off) => self.eq_right(off, *other),
        }
    }
}

impl<'a> PartialEq<Window<'a>> for &'a [u8] {
    fn eq(&self, other: &Window) -> bool {
        match other.off {
            Off::Left(off) => other.eq_left(off, self),
            Off::Right(off) => other.eq_right(off, self),
        }
    }
}

pub struct Windows<'a> {
    blocks: &'a [Block],
    off: usize,
    roff: usize,
    len: usize,
    size: usize, // window size
}

pub fn windows<'a>(buf: &'a ByteBuf, size: usize) -> Windows<'a> {
    let blocks: &[Block] = &buf.blocks[buf.idx..];
    let mut len = blocks.iter().fold(0, |n, b| n + b.len());
    len = if len >= size { len - size + 1 } else { 0 };
    Windows {
        blocks,
        off: 0,
        roff: 0,
        len,
        size,
    }
}

impl<'a> Iterator for Windows<'a> {
    type Item = Window<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        loop {
            match self.blocks.first() {
                Some(block) => {
                    if block.len() > self.off {
                        let window = Window::from_left(self.blocks, self.off, self.size);
                        self.off += 1;
                        return Some(window);
                    }
                }
                None => ::unreachable(),
            }
            self.blocks = &self.blocks[1..];
            self.off = 0;
        }
    }

    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if n >= self.len {
            self.len = 0;
            return None;
        }
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
        self.len -= n;
        self.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.len, Some(self.len))
    }
}

impl<'a> DoubleEndedIterator for Windows<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        loop {
            match self.blocks.last() {
                Some(block) => {
                    if block.len() > self.roff {
                        let window = Window::from_right(self.blocks, self.roff, self.size);
                        self.roff += 1;
                        return Some(window);
                    }
                }
                None => ::unreachable(),
            }
            self.blocks = &self.blocks[..self.blocks.len() - 1];
            self.roff = 0;
        }
    }
}

impl<'a> ExactSizeIterator for Windows<'a> {
    #[inline]
    fn len(&self) -> usize {
        self.len
    }
}
