pub mod codec;

mod block;
use self::block::Block;

mod read;
pub use self::read::{ReadBlock, ReadIter};

mod get;
pub use self::get::{GetBlock, GetIter};

mod set;
pub use self::set::{SetBlock, SetIter};

mod append;
pub use self::append::{AppendBlock, Appender};

mod prepend;
pub use self::prepend::{PrependBlock, Prepender};

mod bytes;
pub use self::bytes::Bytes;

mod window;
pub use self::window::{Window, Windows};

mod hex_dump;
pub use self::hex_dump::HexDump;

mod reader;
pub use self::reader::Reader;

mod writer;
pub use self::writer::Writer;

use std::cmp::Ordering;
use std::mem;
use std::io::{Result, Read, Write, Error, ErrorKind};
use std::ptr;
use std::slice;

use nio::{IoVec, ReadV, WriteV};

const EMPTY: &[u8] = &[];

#[derive(Debug)]
pub struct ByteBuf {
    blocks: Vec<Block>,
    idx: usize,
    growth: usize,
}

impl ByteBuf {
    #[inline]
    pub fn new() -> Self {
        Self::with_growth(0)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = if capacity == 0 { 8 * 1024 } else { capacity };
        let inner = Block::with_capacity(cap);
        let growth = inner.capacity();
        ByteBuf {
            blocks: vec![inner],
            idx: 0,
            growth,
        }
    }

    #[inline]
    pub fn with_growth(mut growth: usize) -> Self {
        if growth < word_len!() {
            growth = 8 * 1024;
        }
        ByteBuf {
            blocks: Vec::new(),
            idx: 0,
            growth,
        }
    }

    #[inline]
    pub fn set_growth(&mut self, growth: usize) {
        self.growth = growth;
    }

    pub fn len(&self) -> usize {
        (&self.blocks[self.idx..])
            .iter()
            .fold(0, |len, b| len + b.len())
    }

    #[inline]
    pub fn read<T, R>(&mut self, read: R) -> Result<T>
        where R: Fn(&mut ReadIter) -> Result<T>
    {
        read(&mut self.read_iter())
    }

    #[inline]
    pub fn read_exact<T, R>(&mut self, len: usize, read_exact: R) -> Result<T>
        where R: Fn(&mut ReadIter, usize) -> Result<T>
    {
        read_exact(&mut self.read_iter(), len)
    }

    #[inline]
    pub fn get<T, G>(&mut self, mut index: usize, get: G) -> Result<T>
        where G: Fn(&mut GetIter) -> Result<T>
    {
        let idx = self.locate_idx(&mut index)?;
        get(&mut self.get_iter(idx, index))
    }

    #[inline]
    pub fn get_exact<T, G>(&mut self, mut index: usize, len: usize, get_exact: G) -> Result<T>
        where G: Fn(&mut GetIter, usize) -> Result<T>
    {
        let idx = self.locate_idx(&mut index)?;
        get_exact(&mut self.get_iter(idx, index), len)
    }

    #[inline]
    pub fn set<T, S>(&mut self, mut index: usize, t: T, set: S) -> Result<usize>
        where S: Fn(T, &mut SetIter) -> Result<usize>
    {
        let idx = self.locate_idx(&mut index)?;
        set(t, &mut self.set_iter(idx, index))
    }

    #[inline]
    pub fn append<T, A>(&mut self, t: T, append: A) -> Result<usize>
        where A: Fn(T, &mut Appender) -> Result<usize>
    {
        append(t, &mut self.appender())
    }

    #[inline]
    pub fn prepend<T, P>(&mut self, t: T, prepend: P) -> Result<usize>
        where P: Fn(T, &mut Prepender) -> Result<usize>
    {
        prepend(t, &mut self.prepender())
    }

    pub fn try_reserve_in_head(&mut self, len: usize) -> usize {
        match self.first_mut() {
            Some(first) => {
                if first.is_empty() {
                    let reserved = if len > first.capacity() {
                        first.capacity()
                    } else {
                        len
                    };

                    first.set_read_pos(0);
                    first.set_write_pos(reserved);
                    first.set_read_pos(reserved);
                    reserved
                } else {
                    first.read_pos()
                }
            }
            None => 0,
        }
    }

    /// Reserves the minimum capacity for exactly additional more bytes to be appended to
    /// the given `ByteBuf`.  Does nothing if the capacity is already sufficient.
    pub fn reserve(&mut self, additional: usize) {
        let appendable = match self.last() {
            Some(last) => last.appendable(),
            None => 0,
        };
        if appendable < additional {
            self.append_block(additional - appendable);
        }
    }

    pub fn extend(&mut self, mut other: Self) {
        let pos = other.idx;
        let n = other.blocks.len() - pos;
        let off = self.blocks.len();
        self.blocks.reserve(n);
        let src_ptr = other.blocks.as_ptr();
        let dst_ptr = self.blocks.as_mut_ptr();
        unsafe {
            self.blocks.set_len(off + n);
            ptr::copy_nonoverlapping(src_ptr.offset(pos as isize),
                                     dst_ptr.offset(off as isize),
                                     n);
            other.blocks.set_len(pos);
        }
    }

    pub fn split_off(&mut self, mut at: usize) -> Result<Self> {
        let idx = self.locate_idx(&mut at)?;
        let len = idx + 1;
        let n = self.blocks.len() - len;
        let mut other_blocks = Vec::new();
        let mut other_idx = 0;
        let other_len;
        let mut dst_ptr;
        {
            let block = unsafe { self.blocks.get_unchecked_mut(idx) };
            let other_block = block.split_off(at);
            if !other_block.is_empty() || other_block.appendable() >= 8 {
                other_len = n + 1;
                other_blocks.reserve(other_len);
                dst_ptr = other_blocks.as_mut_ptr();
                unsafe {
                    ptr::write(dst_ptr, other_block);
                    dst_ptr = dst_ptr.offset(1);
                }
                if self.idx > idx {
                    other_idx = self.idx - idx;
                    self.idx = idx;
                }
            } else if n > 0 {
                other_len = n;
                other_blocks.reserve(other_len);
                dst_ptr = other_blocks.as_mut_ptr();
                if self.idx > idx {
                    other_idx = self.idx - len;
                    self.idx = idx;
                }
            } else {
                return Ok(Self::with_growth(self.growth));
            }
        }

        unsafe {
            self.blocks.set_len(len);
            other_blocks.set_len(other_len);
            ptr::copy_nonoverlapping(self.blocks.as_ptr().offset(len as isize), dst_ptr, n);
        }

        Ok(ByteBuf {
               blocks: other_blocks,
               idx: other_idx,
               growth: self.growth,
           })
    }

    #[inline]
    pub fn drain_to(&mut self, at: usize) -> Result<Self> {
        let mut other = self.split_off(at)?;
        mem::swap(self, &mut other);
        Ok(other)
    }

    pub fn as_bytes(&self) -> Option<&[u8]> {
        let mut idx = self.idx;
        let mut n = None;
        while idx < self.blocks.len() {
            if !unsafe { self.blocks.get_unchecked(idx) }.is_empty() {
                if n.is_some() {
                    return None;
                }
                n = Some(idx);
            }
            idx += 1;
        }

        match n {
            Some(i) => Some(unsafe { self.blocks.get_unchecked(i) }.as_bytes()),
            None => Some(EMPTY),
        }
    }

    #[inline]
    fn starts_with_internal(&self, mut needle: &[u8], pos: usize) -> bool {
        for block in &self.blocks[pos..] {
            if block.len() < needle.len() {
                let (l, r) = needle.split_at(block.len());
                needle = r;
                if !block.starts_with(l) {
                    return false;
                }
            } else {
                return block.starts_with(needle);
            }
        }
        false
    }

    pub fn starts_with(&self, needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        self.starts_with_internal(needle, self.idx)
    }

    #[inline]
    fn ends_with_internal(&self, mut needle: &[u8], rpos: usize) -> bool {
        for block in (&self.blocks[self.idx..rpos]).iter().rev() {
            if block.len() < needle.len() {
                let (l, r) = needle.split_at(needle.len() - block.len());
                needle = l;
                if !block.ends_with(r) {
                    return false;
                }
            } else {
                return block.ends_with(needle);
            }
        }
        false
    }

    pub fn ends_with(&self, needle: &[u8]) -> bool {
        if needle.is_empty() {
            return true;
        }
        self.ends_with_internal(needle, self.blocks.len())
    }

    pub fn windows(&self, size: usize) -> Windows {
        window::windows(self, size)
    }

    pub fn find_from(&self, needle: &[u8], mut pos: usize) -> Option<usize> {
        let mut off = pos;
        let mut idx = match self.locate_idx(&mut off) {
            Ok(i) => i,
            Err(..) => return None,
        };
        if needle.is_empty() {
            return Some(pos);
        }
        let mut bytes = unsafe { self.blocks.get_unchecked(idx) }.as_bytes_from(off);
        loop {
            if bytes.len() >= needle.len() {
                bytes = match bytes.windows(needle.len()).position(|w| w == needle) {
                    Some(n) => return Some(pos + n),
                    None => {
                        let n = bytes.len() - needle.len() + 1;
                        pos += n;
                        &bytes[n..]
                    }
                };
            }
            idx += 1;
            if idx >= self.blocks.len() {
                break;
            }
            for m in 0..bytes.len() {
                let (left, right) = needle.split_at(bytes.len() - m);
                if &bytes[m..] == left {
                    if self.starts_with_internal(right, idx) {
                        return Some(pos + m);
                    }
                }
            }
            pos += bytes.len();
            bytes = unsafe { self.blocks.get_unchecked(idx) }.as_bytes();
        }
        None
    }

    #[inline]
    pub fn find(&self, needle: &[u8]) -> Option<usize> {
        self.find_from(needle, 0)
    }

    pub fn rfind_from(&self, needle: &[u8], mut rpos: usize) -> Option<usize> {
        rpos += needle.len();
        let mut off = rpos;
        let (mut idx, mut bytes) = match self.locate_idx(&mut off) {
            Ok(i) => (i, unsafe { self.blocks.get_unchecked(i) }.as_bytes_to(off)),
            Err(..) => {
                rpos -= off;
                match self.blocks.last() {
                    Some(block) => {
                        off = block.len();
                        (self.blocks.len() - 1, block.as_bytes())
                    }
                    None => (self.idx, EMPTY),
                }
            }
        };
        if needle.is_empty() {
            return Some(rpos);
        }
        rpos -= off;
        loop {
            if bytes.len() >= needle.len() {
                bytes = match bytes.windows(needle.len()).rposition(|w| w == needle) {
                    Some(n) => return Some(rpos + n),
                    None => &bytes[..needle.len() - 1],
                };
            }
            if idx == self.idx {
                break;
            }
            for m in (1..bytes.len() + 1).rev() {
                let (left, right) = needle.split_at(needle.len() - m);
                if &bytes[..m] == right {
                    if self.ends_with_internal(left, idx) {
                        return Some(rpos + m - needle.len());
                    }
                }
            }
            idx -= 1;
            bytes = unsafe { self.blocks.get_unchecked(idx) }.as_bytes();
            rpos -= bytes.len();
        }
        None
    }

    #[inline]
    pub fn rfind(&self, needle: &[u8]) -> Option<usize> {
        self.rfind_from(needle, ::std::usize::MAX)
    }

    pub fn compact(&mut self) {
        while self.idx < self.blocks.len() &&
              unsafe { self.blocks.get_unchecked(self.idx) }.is_empty() {
            self.idx += 1;
        }
        if self.idx > 0 {
            let other_len = self.blocks.len() - self.idx;
            let mut other_blocks = Vec::new();
            other_blocks.reserve(other_len);

            unsafe {
                ptr::copy_nonoverlapping(self.blocks.as_ptr().offset(self.idx as isize),
                                         other_blocks.as_mut_ptr(),
                                         other_len);
                self.blocks.set_len(self.idx);
                other_blocks.set_len(other_len);
            }
            self.blocks = other_blocks;
            self.idx = 0;
        }
    }

    pub fn skip(&mut self, n: usize) -> usize {
        let mut skipped = 0;
        let i = self.idx;
        for block in &mut self.blocks[i..] {
            let m = block.len();
            skipped += m;
            if skipped >= n {
                let read_pos = block.read_pos();
                block.set_read_pos(read_pos + m - (skipped - n));
                return n;
            } else {
                let write_pos = block.write_pos();
                block.set_read_pos(write_pos);
                self.idx += 1;
            }
        }
        skipped
    }

    pub fn is_empty(&self) -> bool {
        for block in &self.blocks[self.idx..] {
            if !block.is_empty() {
                return false;
            }
        }
        true
    }

    pub fn read_in<R>(&mut self, mut r: R) -> Result<usize>
        where R: Read + ReadV
    {
        let n = self.blocks.len() as isize;
        let ptr = self.blocks.as_mut_ptr();
        let last = unsafe { &mut *ptr.offset(n - 1) };
        let off = last.write_pos();
        let size = last.appendable();
        if n > 1 && last.is_empty() {
            let last_2nd = unsafe { &mut *ptr.offset(n - 2) };
            let size2 = last_2nd.appendable();
            if size2 > 0 {
                let off2 = last_2nd.write_pos();
                let iovs = [IoVec::from_mut(unsafe { &mut *last_2nd.mut_ptr_at(off2) }, size2),
                            IoVec::from_mut(unsafe { &mut *last.mut_ptr_at(off) }, size)];
                let read = r.readv(&iovs)?;
                if read <= size2 {
                    last_2nd.set_write_pos(off2 + read);
                } else {
                    let mut len = last_2nd.capacity();
                    last_2nd.set_write_pos(len);
                    len = read - size2;
                    last.set_write_pos(off + len);
                }
                return Ok(read);
            }
        }

        let buf = unsafe { slice::from_raw_parts_mut(last.mut_ptr_at(off), size) };
        let read = r.read(buf)?;
        last.set_write_pos(off + read);
        Ok(read)
    }

    pub fn write_out<W>(&mut self, w: &mut W) -> Result<usize>
        where W: Write + WriteV
    {
        let n = self.blocks.len() - self.idx;
        if n == 1 {
            let block = unsafe { self.blocks.get_unchecked_mut(self.idx) };
            if block.len() < 1 {
                return Ok(0);
            }
            let off = block.read_pos();
            let buf = unsafe { slice::from_raw_parts(block.ptr_at(off), block.len()) };
            let written = w.write(buf)?;
            block.set_read_pos(off + written);
            Ok(written)
        } else if n > 1 {
            let mut iovs = Vec::with_capacity(n);
            for block in &self.blocks[self.idx..] {
                if block.len() > 0 {
                    let off = block.read_pos();
                    iovs.push(IoVec::from(unsafe { &*block.ptr_at(off) }, block.len()));
                }
            }
            let written = w.writev(iovs.as_slice())?;
            self.skip(written);
            Ok(written)
        } else {
            Ok(0)
        }
    }

    #[inline]
    pub fn as_reader(&mut self) -> Reader {
        reader::new(self)
    }

    #[inline]
    pub fn as_writer(&mut self) -> Writer {
        writer::new(self)
    }

    #[inline]
    pub fn as_hex_dump(&self) -> HexDump {
        hex_dump::new(self)
    }

    #[inline]
    pub fn bytes(&self) -> Bytes {
        bytes::new(self)
    }

    #[inline]
    fn pos(&self) -> usize {
        self.idx
    }

    #[inline]
    fn inc_pos(&mut self) {
        self.idx += 1;
    }

    #[inline]
    fn num_of_blocks(&self) -> usize {
        self.blocks.len()
    }

    #[inline]
    unsafe fn mut_ptr_at(&mut self, index: usize) -> *mut Block {
        self.blocks.as_mut_ptr().offset(index as isize)
    }

    #[inline]
    fn read_iter(&mut self) -> ReadIter {
        read::iter(self)
    }

    #[inline]
    fn get_iter(&self, idx: usize, pos: usize) -> GetIter {
        get::iter(&self.blocks[idx..], pos)
    }

    #[inline]
    fn set_iter(&mut self, idx: usize, pos: usize) -> SetIter {
        set::iter(&mut self.blocks[idx..], pos)
    }

    #[inline]
    fn appender(&mut self) -> Appender {
        append::appender(self)
    }

    #[inline]
    fn prepender(&mut self) -> Prepender {
        prepend::prepender(self)
    }

    fn locate_idx(&self, pos: &mut usize) -> Result<usize> {
        let mut i = self.idx;
        for block in &self.blocks[self.idx..] {
            if *pos <= block.len() {
                return Ok(i);
            }
            i += 1;
            *pos -= block.len();
        }
        Err(Error::new(ErrorKind::UnexpectedEof, "Index out of bounds"))
    }

    #[inline]
    fn last(&self) -> Option<&Block> {
        self.blocks.last()
    }

    #[inline]
    fn last_mut(&mut self) -> Option<&mut Block> {
        self.blocks.last_mut()
    }

    #[inline]
    fn first_mut(&mut self) -> Option<&mut Block> {
        self.blocks.first_mut()
    }

    #[inline]
    fn append_block(&mut self, min_capacity: usize) {
        let cap = if min_capacity <= self.growth {
            self.growth
        } else {
            min_capacity
        };
        self.blocks.push(Block::with_capacity(cap));
    }

    #[inline]
    fn prepend_block(&mut self, min_capacity: usize) {
        let cap = if min_capacity <= self.growth {
            self.growth
        } else {
            min_capacity
        };
        self.blocks.insert(0, Block::for_prependable(cap));
    }
}

impl Ord for ByteBuf {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut none1 = false;
        let mut none2 = false;
        let mut iter = self.blocks.iter();
        let mut other_iter = other.blocks.iter();
        let mut slice = EMPTY;
        let mut other_slice = EMPTY;
        loop {
            if slice.len() < other_slice.len() {
                let (s1, s2) = other_slice.split_at(slice.len());
                let ord = slice.cmp(s1);
                if ord != Ordering::Equal {
                    return ord;
                }
                slice = match iter.next() {
                    Some(block) => block.as_bytes(),
                    None => return Ordering::Less,
                };
                other_slice = s2;
            } else if slice.len() > other_slice.len() {
                let (s1, s2) = slice.split_at(other_slice.len());
                let ord = s1.cmp(other_slice);
                if ord != Ordering::Equal {
                    return ord;
                }
                other_slice = match other_iter.next() {
                    Some(block) => block.as_bytes(),
                    None => return Ordering::Greater,
                };
                slice = s2;
            } else {
                let ord = slice.cmp(other_slice);
                if ord != Ordering::Equal {
                    return ord;
                }
                if none1 && none2 {
                    return Ordering::Equal;
                }
                slice = match iter.next() {
                    Some(block) => block.as_bytes(),
                    None => {
                        none1 = true;
                        EMPTY
                    }
                };
                other_slice = match other_iter.next() {
                    Some(block) => block.as_bytes(),
                    None => {
                        none2 = true;
                        EMPTY
                    }
                }
            }
        }
    }
}

impl PartialOrd for ByteBuf {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for ByteBuf {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for ByteBuf {}
