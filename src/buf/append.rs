use super::{Block, ByteBuf};

pub struct AppendBlock<'a> {
    inner: &'a mut Block,
}

pub struct Appender<'a> {
    inner: &'a mut ByteBuf,
}

#[inline]
pub(super) fn appender<'a>(inner: &'a mut ByteBuf) -> Appender<'a> {
    Appender { inner }
}

impl<'a> AppendBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Block) -> Self {
        AppendBlock { inner: inner }
    }

    #[inline]
    pub fn write_pos(&self) -> usize {
        self.inner.write_pos()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }

    #[inline]
    pub fn appendable(&self) -> usize {
        self.inner.appendable()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    pub fn set_write_pos(&mut self, write_pos: usize) {
        self.inner.set_write_pos(write_pos)
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }
}

impl<'a> Appender<'a> {
    #[inline]
    pub fn last_mut(&mut self) -> Option<AppendBlock> {
        match self.inner.last_mut() {
            Some(block) => Some(AppendBlock::new(block)),
            None => None,
        }
    }

    pub fn append(&mut self, min_capacity: usize) {
        self.inner.append_block(min_capacity)
    }

    pub fn append_bytes(&mut self, bytes: Vec<u8>) {
        self.inner.append_bytes(bytes)
    }
}
