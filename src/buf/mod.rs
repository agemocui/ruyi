pub mod codec;

use std::cmp::Ordering;
use std::fmt;
use std::mem;
use std::io::{Result, Read, Write, Error, ErrorKind};
use std::iter::Iterator;
use std::option::Option;
use std::ptr;
use std::rc::Rc;
use std::slice::{self, Iter};
use std::str;

use super::nio::{IoVec, ReadV, WriteV};

#[derive(Debug)]
struct Alloc {
    ptr: *mut u8,
    cap: usize,
}

fn alloc(len: usize) -> Alloc {
    const MASK: usize = word_len!() - 1;
    let n = len + MASK;
    // align = mem::size_of::<usize>()
    let mut buf = Vec::<usize>::with_capacity(n / word_len!());
    let ptr = buf.as_mut_ptr() as *mut u8;
    mem::forget(buf);
    Alloc {
        ptr: ptr,
        cap: n & !MASK,
    }
}

impl Drop for Alloc {
    fn drop(&mut self) {
        drop(unsafe { Vec::from_raw_parts(self.ptr, 0, self.cap) });
    }
}

#[derive(Debug, Clone)]
struct Inner {
    ptr: *mut u8,
    cap: usize,
    read_pos: usize,
    write_pos: usize,
    shared: Rc<Alloc>,
}

impl Inner {
    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        let alloc = alloc(capacity);
        Inner {
            ptr: alloc.ptr,
            cap: alloc.cap,
            read_pos: 0,
            write_pos: 0,
            shared: Rc::new(alloc),
        }
    }

    #[inline]
    fn for_prependable(capacity: usize) -> Self {
        let alloc = alloc(capacity);
        Inner {
            ptr: alloc.ptr,
            cap: alloc.cap,
            read_pos: alloc.cap,
            write_pos: alloc.cap,
            shared: Rc::new(alloc),
        }
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }

    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    #[inline]
    fn mut_ptr_at(&mut self, off: usize) -> *mut u8 {
        unsafe { self.ptr.offset(off as isize) }
    }

    #[inline]
    fn ptr_at(&self, off: usize) -> *const u8 {
        unsafe { self.ptr.offset(off as isize) }
    }

    #[inline]
    fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    #[inline]
    fn capacity(&self) -> usize {
        self.cap
    }

    #[inline]
    fn set_capacity(&mut self, capacity: usize) {
        debug_assert!(capacity >= self.write_pos,
                      "`capacity` out of bounds: capacity={}, write_pos={}",
                      capacity,
                      self.write_pos);
        self.cap = capacity;
    }

    #[inline]
    fn read_pos(&self) -> usize {
        self.read_pos
    }

    #[inline]
    fn set_read_pos(&mut self, read_pos: usize) {
        debug_assert!(read_pos <= self.write_pos,
                      "`read_pos` out of bounds: read_pos={}, write_pos={}",
                      read_pos,
                      self.write_pos);
        self.read_pos = read_pos;
    }

    #[inline]
    fn write_pos(&self) -> usize {
        self.write_pos
    }

    #[inline]
    fn set_write_pos(&mut self, write_pos: usize) {
        debug_assert!(write_pos >= self.read_pos && write_pos <= self.cap,
                      "`write_pos` out of bounds: read_pos={}, write_pos={}, capacity={}",
                      self.read_pos,
                      write_pos,
                      self.cap);
        self.write_pos = write_pos;
    }

    #[inline]
    fn len(&self) -> usize {
        self.write_pos() - self.read_pos()
    }

    #[inline]
    fn appendable(&self) -> usize {
        self.capacity() - self.write_pos()
    }

    #[inline]
    fn prependable(&self) -> usize {
        self.read_pos()
    }

    #[inline]
    fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr_at(self.read_pos()), self.len()) }
    }

    #[inline]
    fn split_off(&mut self, at: usize) -> Self {
        let off = self.read_pos() + at;

        debug_assert!(off <= self.write_pos(),
                      "`at` out of bounds: read_pos={}, at={}, write_pos={}",
                      self.read_pos,
                      at,
                      self.write_pos);

        let other_ptr = unsafe { self.ptr.offset(off as isize) };

        let other_write_pos = self.write_pos() - off;
        self.set_write_pos(off);

        let other_cap = self.cap - off;
        self.set_capacity(off);

        Inner {
            ptr: other_ptr,
            cap: other_cap,
            read_pos: 0,
            write_pos: other_write_pos,
            shared: self.shared.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ByteBuf {
    blocks: Vec<Inner>,
    pos_idx: usize,
    growth: usize,
}

pub struct ReadBlock<'a> {
    inner: &'a mut Inner,
}

impl<'a> ReadBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Inner) -> Self {
        ReadBlock { inner: inner }
    }

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

pub struct ReadIter<'a> {
    inner: &'a mut ByteBuf,
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
        while self.inner.pos_idx() < self.inner.num_of_blocks() {
            let i = self.inner.pos_idx();
            let inner = unsafe { &mut *self.inner.mut_ptr_at(i) };
            if !inner.is_empty() {
                return Some(ReadBlock::new(inner));
            }
            self.inner.inc_pos_idx();
        }
        None
    }
}

pub struct GetBlock<'a> {
    inner: &'a Inner,
    get_pos: usize,
}

impl<'a> GetBlock<'a> {
    #[inline]
    fn new(inner: &'a Inner) -> Self {
        GetBlock {
            inner: inner,
            get_pos: inner.read_pos(),
        }
    }

    #[inline]
    fn with_index(inner: &'a Inner, index: usize) -> Self {
        GetBlock {
            inner: inner,
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

pub struct GetIter<'a> {
    blocks: &'a [Inner],
    pos_idx: usize,
    // offset of the first u8 in the first block
    init_index: usize,
}

impl<'a> GetIter<'a> {
    pub fn len(&self) -> usize {
        let init_val = unsafe { self.blocks.get_unchecked(self.pos_idx).len() - self.init_index };
        self.blocks[self.pos_idx + 1..]
            .iter()
            .fold(0, |remaining, b| remaining + b.len()) + init_val
    }
}

impl<'a> Iterator for GetIter<'a> {
    type Item = GetBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos_idx < self.blocks.len() {
            let inner = unsafe { self.blocks.get_unchecked(self.pos_idx) };
            self.pos_idx += 1;
            if self.init_index > 0 {
                let index = self.init_index;
                self.init_index = 0;
                if inner.len() > index {
                    return Some(GetBlock::with_index(inner, index));
                }
            } else if !inner.is_empty() {
                return Some(GetBlock::new(inner));
            }
        }
        None
    }
}

pub struct SetBlock<'a> {
    inner: &'a mut Inner,
    set_pos: usize,
}

impl<'a> SetBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Inner) -> Self {
        let set_pos = inner.read_pos();
        SetBlock {
            inner: inner,
            set_pos: set_pos,
        }
    }

    #[inline]
    fn with_index(inner: &'a mut Inner, index: usize) -> Self {
        let set_pos = inner.read_pos() + index;
        SetBlock {
            inner: inner,
            set_pos: set_pos,
        }
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

pub struct SetIter<'a> {
    blocks: &'a mut [Inner],
    pos_idx: usize,
    // offset of the first u8 in the first block
    init_index: usize,
}

impl<'a> Iterator for SetIter<'a> {
    type Item = SetBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.pos_idx < self.blocks.len() {
            let inner = unsafe { &mut *self.blocks.as_mut_ptr().offset(self.pos_idx as isize) };
            self.pos_idx += 1;
            if self.init_index > 0 {
                let index = self.init_index;
                self.init_index = 0;
                if inner.len() > index {
                    return Some(SetBlock::with_index(inner, index));
                }
            } else if !inner.is_empty() {
                return Some(SetBlock::new(inner));
            }
        }
        None
    }
}

pub struct AppendBlock<'a> {
    inner: &'a mut Inner,
}

impl<'a> AppendBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Inner) -> Self {
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

pub struct Appender<'a> {
    inner: &'a mut ByteBuf,
}

impl<'a> Appender<'a> {
    #[inline]
    pub fn last_mut(&mut self) -> AppendBlock {
        AppendBlock::new(self.inner.last_mut())
    }

    pub fn append(&mut self, min_capacity: usize) {
        self.inner.append_block(min_capacity)
    }
}

pub struct PrependBlock<'a> {
    inner: &'a mut Inner,
}

impl<'a> PrependBlock<'a> {
    #[inline]
    fn new(inner: &'a mut Inner) -> Self {
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

pub struct Prepender<'a> {
    inner: &'a mut ByteBuf,
}

impl<'a> Prepender<'a> {
    #[inline]
    pub fn first_mut(&mut self) -> PrependBlock {
        PrependBlock::new(self.inner.first_mut())
    }

    #[inline]
    pub fn prepend(&mut self, min_capacity: usize) {
        self.inner.prepend_block(min_capacity)
    }
}

impl ByteBuf {
    #[inline]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let cap = if capacity == 0 { 8 * 1024 } else { capacity };
        ByteBuf {
            blocks: vec![Inner::with_capacity(cap)],
            pos_idx: 0,
            growth: cap,
        }
    }

    #[inline]
    pub fn set_growth(&mut self, growth: usize) {
        self.growth = growth;
    }

    pub fn len(&self) -> usize {
        (&self.blocks[self.pos_idx..])
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
        let pos_idx = self.locate_pos_idx(&mut index)?;
        get(&mut self.get_iter(pos_idx, index))
    }

    #[inline]
    pub fn get_exact<T, G>(&mut self, mut index: usize, len: usize, get_exact: G) -> Result<T>
        where G: Fn(&mut GetIter, usize) -> Result<T>
    {
        let pos_idx = self.locate_pos_idx(&mut index)?;
        get_exact(&mut self.get_iter(pos_idx, index), len)
    }

    #[inline]
    pub fn set<T, S>(&mut self, mut index: usize, t: T, set: S) -> Result<usize>
        where S: Fn(T, &mut SetIter) -> Result<usize>
    {
        let pos_idx = self.locate_pos_idx(&mut index)?;
        set(t, &mut self.set_iter(pos_idx, index))
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
        let first = self.first_mut();
        if first.is_empty() {
            let reserved = if len > first.capacity() {
                first.capacity()
            } else {
                len
            };

            #[cfg(debug_assertions)]
            first.set_read_pos(0);

            first.set_write_pos(reserved);
            first.set_read_pos(reserved);
            reserved
        } else {
            first.read_pos()
        }
    }

    /// Reserves the minimum capacity for exactly additional more bytes to be appended to
    /// the given `ByteBuf`.  Does nothing if the capacity is already sufficient.
    pub fn reserve(&mut self, additional: usize) {
        let appendable = self.last_mut().appendable();
        if appendable < additional {
            self.append_block(additional - appendable);
        }
    }

    pub fn extend(&mut self, mut other: Self) {
        let pos = other.pos_idx;
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
        let pos_idx = self.locate_pos_idx(&mut at)?;
        let len = pos_idx + 1;
        let n = self.blocks.len() - len;
        let mut other_blocks = Vec::new();
        let mut other_pos_idx = 0;
        let other_len;
        let mut dst_ptr;
        {
            let block = unsafe { self.blocks.get_unchecked_mut(pos_idx) };
            let other_block = block.split_off(at);
            if !other_block.is_empty() || other_block.appendable() >= 8 {
                other_len = n + 1;
                other_blocks.reserve(other_len);
                dst_ptr = other_blocks.as_mut_ptr();
                unsafe {
                    ptr::write(dst_ptr, other_block);
                    dst_ptr = dst_ptr.offset(1);
                }
                if self.pos_idx > pos_idx {
                    other_pos_idx = self.pos_idx - pos_idx;
                    self.pos_idx = pos_idx;
                }
            } else if n > 0 {
                other_len = n;
                other_blocks.reserve(other_len);
                dst_ptr = other_blocks.as_mut_ptr();
                if self.pos_idx > pos_idx {
                    other_pos_idx = self.pos_idx - len;
                    self.pos_idx = pos_idx;
                }
            } else {
                return Ok(Self::with_capacity(self.growth));
            }
        }

        unsafe {
            self.blocks.set_len(len);
            other_blocks.set_len(other_len);
            ptr::copy_nonoverlapping(self.blocks.as_ptr().offset(len as isize), dst_ptr, n);
        }

        Ok(ByteBuf {
               blocks: other_blocks,
               pos_idx: other_pos_idx,
               growth: self.growth,
           })
    }

    #[inline]
    pub fn drain_to(&mut self, at: usize) -> Result<Self> {
        let mut other = self.split_off(at)?;
        mem::swap(self, &mut other);
        Ok(other)
    }

    pub fn compact(&mut self) {
        if self.pos_idx > 0 {
            let other_len = self.blocks.len() - self.pos_idx;
            let mut other_blocks = Vec::new();
            other_blocks.reserve(other_len);

            unsafe {
                ptr::copy_nonoverlapping(self.blocks.as_ptr().offset(self.pos_idx as isize),
                                         other_blocks.as_mut_ptr(),
                                         other_len);
                self.blocks.set_len(self.pos_idx);
                other_blocks.set_len(other_len);
            }
            self.blocks = other_blocks;
            self.pos_idx = 0;
        }
    }

    pub fn skip(&mut self, n: usize) -> usize {
        let mut skipped = 0;
        let i = self.pos_idx;
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
                self.pos_idx += 1;
            }
        }
        skipped
    }

    pub fn is_empty(&self) -> bool {
        for block in &self.blocks[self.pos_idx..] {
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

    pub fn write_out<W>(&mut self, mut w: W) -> Result<usize>
        where W: Write + WriteV
    {
        let n = self.blocks.len() - self.pos_idx;
        if n == 1 {
            let block = unsafe { self.blocks.get_unchecked_mut(self.pos_idx) };
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
            for block in &self.blocks[self.pos_idx..] {
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
        Reader { inner: self }
    }

    #[inline]
    pub fn as_writer(&mut self) -> Writer {
        Writer { inner: self }
    }

    #[inline]
    pub fn as_hex_dump(&self) -> HexDump {
        HexDump { inner: self }
    }

    #[inline]
    pub fn bytes(&self) -> Bytes {
        Bytes::new(self)
    }

    #[inline]
    fn pos_idx(&self) -> usize {
        self.pos_idx
    }

    #[inline]
    fn inc_pos_idx(&mut self) {
        self.pos_idx += 1;
    }

    #[inline]
    fn num_of_blocks(&self) -> usize {
        self.blocks.len()
    }

    #[inline]
    unsafe fn mut_ptr_at(&mut self, index: usize) -> *mut Inner {
        self.blocks.as_mut_ptr().offset(index as isize)
    }

    #[inline]
    fn read_iter(&mut self) -> ReadIter {
        ReadIter { inner: self }
    }

    #[inline]
    fn get_iter(&self, pos_idx: usize, index: usize) -> GetIter {
        GetIter {
            blocks: &self.blocks[pos_idx..],
            pos_idx: 0,
            init_index: index,
        }
    }

    #[inline]
    fn set_iter(&mut self, pos_idx: usize, index: usize) -> SetIter {
        SetIter {
            blocks: &mut self.blocks[pos_idx..],
            pos_idx: 0,
            init_index: index,
        }
    }

    #[inline]
    fn appender(&mut self) -> Appender {
        Appender { inner: self }
    }

    #[inline]
    fn prepender(&mut self) -> Prepender {
        Prepender { inner: self }
    }

    fn locate_pos_idx(&self, index: &mut usize) -> Result<usize> {
        let blocks = &self.blocks;
        let mut i = self.pos_idx;
        loop {
            if i == blocks.len() {
                return Err(Error::new(ErrorKind::UnexpectedEof, "Index out of bounds"));
            }
            let block_len = unsafe { blocks.get_unchecked(i) }.len();
            if *index <= block_len {
                return Ok(i);
            }
            *index -= block_len;
            i += 1;
        }
    }

    #[inline]
    fn last_mut(&mut self) -> &mut Inner {
        let n = self.blocks.len() - 1;
        unsafe { self.blocks.get_unchecked_mut(n) }
    }

    #[inline]
    fn first_mut(&mut self) -> &mut Inner {
        unsafe { self.blocks.get_unchecked_mut(0) }
    }

    #[inline]
    fn append_block(&mut self, min_capacity: usize) {
        let cap = if min_capacity == 0 {
            self.growth
        } else {
            min_capacity
        };
        self.blocks.push(Inner::with_capacity(cap));
    }

    #[inline]
    fn prepend_block(&mut self, min_capacity: usize) {
        let cap = if min_capacity == 0 {
            self.growth
        } else {
            min_capacity
        };
        self.blocks.insert(0, Inner::for_prependable(cap));
    }
}

impl Ord for ByteBuf {
    fn cmp(&self, other: &Self) -> Ordering {
        const EMPTY: &[u8] = &[0; 0];

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
                    Some(block) => block.as_slice(),
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
                    Some(block) => block.as_slice(),
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
                    Some(block) => block.as_slice(),
                    None => {
                        none1 = true;
                        EMPTY
                    }
                };
                other_slice = match other_iter.next() {
                    Some(block) => block.as_slice(),
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

pub struct Bytes<'a> {
    iter_inner: Iter<'a, Inner>,
    iter_u8: Option<Iter<'a, u8>>,
}

impl<'a> Bytes<'a> {
    fn new(buf: &'a ByteBuf) -> Self {
        let mut iter_inner = (&buf.blocks[buf.pos_idx..]).iter();
        let iter_u8 = match iter_inner.next() {
            Some(inner) => {
                let ptr = inner.ptr_at(inner.read_pos());
                Some(unsafe { slice::from_raw_parts(ptr, inner.len()) }.iter())
            }
            None => None,
        };
        Bytes {
            iter_inner,
            iter_u8,
        }
    }
}

impl<'a> Iterator for Bytes<'a> {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.iter_u8.as_mut() {
                Some(iter_u8) => {
                    if let Some(b) = iter_u8.next() {
                        return Some(*b);
                    }
                }
                None => return None,
            }
            self.iter_u8 = match self.iter_inner.next() {
                Some(inner) => Some(inner.as_slice().iter()),
                None => None,
            };
        }
    }
}

pub struct Reader<'a> {
    inner: &'a mut ByteBuf,
}

impl<'a> Iterator for Reader<'a> {
    type Item = ReadBlock<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        while self.inner.pos_idx() < self.inner.num_of_blocks() {
            let i = self.inner.pos_idx();
            let block = unsafe { &mut *self.inner.mut_ptr_at(i) };
            if !block.is_empty() {
                return Some(ReadBlock::new(block));
            }
            self.inner.inc_pos_idx();
        }
        None
    }
}

impl<'a> Read for Reader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
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

pub struct Writer<'a> {
    inner: &'a mut ByteBuf,
}

impl<'a> Write for Writer<'a> {
    fn write(&mut self, buf: &[u8]) -> Result<usize> {
        let mut n = buf.len();
        let mut src_dst = buf.as_ptr();
        loop {
            {
                let block = self.inner.last_mut();
                let dst_off = block.write_pos();
                let appendable = block.appendable();
                if appendable >= n {
                    unsafe {
                        ptr::copy_nonoverlapping(src_dst,
                                                 block.as_mut_ptr().offset(dst_off as isize),
                                                 n);
                    }
                    block.set_write_pos(dst_off + n);
                    return Ok(buf.len());
                } else {
                    unsafe {
                        ptr::copy_nonoverlapping(src_dst,
                                                 block.as_mut_ptr().offset(dst_off as isize),
                                                 appendable);
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
    fn flush(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct HexDump<'a> {
    inner: &'a ByteBuf,
}

impl<'a> fmt::Display for HexDump<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut bytes = self.inner.bytes();
        let mut next = bytes.next();
        if next.is_none() {
            return Ok(());
        }

        const BYTES_PER_ROW: usize = 16;

        let mut addr = 0;
        let mut asc: [u8; BYTES_PER_ROW] = unsafe { mem::uninitialized() };
        loop {
            write!(f, "{:08X}h:", addr)?;
            // dump one row
            let mut i = 0;
            while i < BYTES_PER_ROW {
                match next {
                    Some(b) => {
                        write!(f, " {:02X}", b)?;
                        asc[i] = if (b as char).is_control() { b'.' } else { b };
                        i += 1;
                        next = bytes.next();
                    }
                    None => {
                        for _ in i..BYTES_PER_ROW {
                            write!(f, "   ")?;
                        }
                        writeln!(f, "   {}", unsafe { str::from_utf8_unchecked(&asc[0..i]) })?;
                        return Ok(());
                    }
                }
            }
            writeln!(f, "   {}", unsafe { str::from_utf8_unchecked(&asc[0..i]) })?;
            addr += BYTES_PER_ROW;
        }
    }
}
