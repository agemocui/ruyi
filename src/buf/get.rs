use super::Block;

pub struct GetBlock<'a> {
    inner: &'a Block,
    get_pos: usize,
}

pub struct GetIter<'a> {
    blocks: &'a [Block],
    idx: usize,
    // offset of the first u8 in the first block
    init_pos: usize,
}

#[inline]
pub(super) fn iter<'a>(blocks: &'a [Block], init_pos: usize) -> GetIter<'a> {
    GetIter {
        blocks,
        idx: 0,
        init_pos,
    }
}

impl<'a> GetBlock<'a> {
    #[inline]
    fn new(inner: &'a Block) -> Self {
        GetBlock {
            inner,
            get_pos: inner.read_pos(),
        }
    }

    #[inline]
    fn with_index(inner: &'a Block, index: usize) -> Self {
        GetBlock {
            inner,
            get_pos: inner.read_pos() + index,
        }
    }

    #[inline]
    pub fn read_pos(&self) -> usize {
        self.get_pos
    }

    #[inline]
    pub fn write_pos(&self) -> usize {
        self.inner.write_pos()
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.write_pos() - self.read_pos()
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.inner.as_ptr()
    }
}

impl<'a> GetIter<'a> {
    pub fn len(&self) -> usize {
        let init_val = unsafe { self.blocks.get_unchecked(self.idx).len() - self.init_pos };
        self.blocks[self.idx + 1..]
            .iter()
            .fold(0, |remaining, b| remaining + b.len()) + init_val
    }
}

impl<'a> Iterator for GetIter<'a> {
    type Item = GetBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.blocks.len() {
            let inner = unsafe { self.blocks.get_unchecked(self.idx) };
            self.idx += 1;
            if self.init_pos > 0 {
                let pos = self.init_pos;
                self.init_pos = 0;
                if inner.len() > pos {
                    return Some(GetBlock::with_index(inner, pos));
                }
            } else if !inner.is_empty() {
                return Some(GetBlock::new(inner));
            }
        }
        None
    }
}
