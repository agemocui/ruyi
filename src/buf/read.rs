use super::{Block, ByteBuf};

pub struct ReadBlock<'a> {
    inner: &'a mut Block,
}

pub struct ReadIter<'a> {
    inner: &'a mut ByteBuf,
}

#[inline]
pub(super) fn new<'a>(inner: &'a mut Block) -> ReadBlock<'a> {
    ReadBlock { inner }
}

#[inline]
pub(super) fn iter<'a>(inner: &'a mut ByteBuf) -> ReadIter<'a> {
    ReadIter { inner }
}

impl<'a> ReadBlock<'a> {
    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }

    #[inline]
    pub fn read_pos(&self) -> usize {
        self.inner.read_pos()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn write_pos(&self) -> usize {
        self.inner.write_pos()
    }

    #[inline]
    pub fn set_read_pos(&mut self, read_pos: usize) {
        self.inner.set_read_pos(read_pos)
    }
}

impl<'a> ReadIter<'a> {
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }
}

impl<'a> Iterator for ReadIter<'a> {
    type Item = ReadBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.inner.pos() < self.inner.num_of_blocks() {
            let i = self.inner.pos();
            let inner = unsafe { &mut *self.inner.mut_ptr_at(i) };
            if !inner.is_empty() {
                return Some(new(inner));
            }
            self.inner.inc_pos();
        }
        None
    }
}
