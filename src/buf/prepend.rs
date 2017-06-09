use super::{Block, ByteBuf};

pub struct PrependBlock<'a> {
    inner: &'a mut Block,
}

pub struct Prepender<'a> {
    inner: &'a mut ByteBuf,
}

#[inline]
pub(super) fn prepender<'a>(inner: &'a mut ByteBuf) -> Prepender<'a> {
    Prepender { inner }
}

impl<'a> PrependBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Block) -> Self {
        PrependBlock { inner: inner }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn prependable(&self) -> usize {
        self.inner.prependable()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }

    #[inline]
    pub fn read_pos(&self) -> usize {
        self.inner.read_pos()
    }

    #[inline]
    pub fn set_read_pos(&mut self, read_pos: usize) {
        self.inner.set_read_pos(read_pos)
    }
}

impl<'a> Prepender<'a> {
    #[inline]
    pub fn first_mut(&mut self) -> Option<PrependBlock> {
        match self.inner.first_mut() {
            Some(block) => Some(PrependBlock::new(block)),
            None => None,
        }
    }

    #[inline]
    pub fn prepend(&mut self, min_capacity: usize) {
        self.inner.prepend_block(min_capacity)
    }
}
