use super::Block;

pub struct SetBlock<'a> {
    inner: &'a mut Block,
    set_pos: usize,
}

pub struct SetIter<'a> {
    blocks: &'a mut [Block],
    idx: usize,
    // offset of the first u8 in the first block
    init_pos: usize,
}

#[inline]
pub(super) fn iter<'a>(blocks: &'a mut [Block], init_pos: usize) -> SetIter {
    SetIter {
        blocks,
        idx: 0,
        init_pos,
    }
}

impl<'a> SetBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Block) -> Self {
        let set_pos = inner.read_pos();
        SetBlock { inner, set_pos }
    }

    #[inline]
    fn with_offset(inner: &'a mut Block, off: usize) -> Self {
        let set_pos = inner.read_pos() + off;
        SetBlock { inner, set_pos }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.write_pos() - self.read_pos()
    }

    #[inline]
    pub fn read_pos(&self) -> usize {
        self.set_pos
    }

    #[inline]
    pub fn write_pos(&self) -> usize {
        self.inner.write_pos()
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.inner.as_mut_ptr()
    }
}

impl<'a> Iterator for SetIter<'a> {
    type Item = SetBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.idx < self.blocks.len() {
            let inner = unsafe { &mut *self.blocks.as_mut_ptr().offset(self.idx as isize) };
            self.idx += 1;
            if self.init_pos > 0 {
                let pos = self.init_pos;
                self.init_pos = 0;
                if inner.len() > pos {
                    return Some(SetBlock::with_offset(inner, pos));
                }
            } else if !inner.is_empty() {
                return Some(SetBlock::new(inner));
            }
        }
        None
    }
}
