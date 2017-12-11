use std::ptr;

use buf::{Appender, BufError, GetIter, Prepender, ReadIter, SetIter};

#[derive(Copy, Clone)]
pub struct Filling {
    v: u8,
    n: usize,
}

#[inline]
pub fn filling(val: u8, count: usize) -> Filling {
    Filling { v: val, n: count }
}

impl Filling {
    #[inline]
    pub fn new(val: u8, count: usize) -> Self {
        Filling { v: val, n: count }
    }

    #[inline]
    pub fn val(&self) -> u8 {
        self.v
    }

    #[inline]
    pub fn count(&self) -> usize {
        self.n
    }
}

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<Vec<u8>, BufError> {
    let len = chain.len();
    read_exact(chain, len)
}

pub fn read_exact(chain: &mut ReadIter, mut n: usize) -> Result<Vec<u8>, BufError> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut buf = Vec::with_capacity(n);
    unsafe { buf.set_len(n) };
    let mut ptr_dst = buf.as_mut_ptr();
    for mut block in chain {
        let off = block.read_pos();
        let ptr_src = unsafe { block.as_ptr().offset(off as isize) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            block.set_read_pos(off + n);
            return Ok(buf);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
        let pos = block.write_pos();
        block.set_read_pos(pos);
    }
    Err(BufError::Underflow)
}

#[inline]
pub fn get(chain: &mut GetIter) -> Result<Vec<u8>, BufError> {
    let len = chain.len();
    get_exact(chain, len)
}

pub fn get_exact(chain: &mut GetIter, mut n: usize) -> Result<Vec<u8>, BufError> {
    if n == 0 {
        return Ok(Vec::new());
    }
    let mut buf = Vec::with_capacity(n);
    unsafe { buf.set_len(n) };
    let mut ptr_dst = buf.as_mut_ptr();
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr_src = unsafe { block.as_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            return Ok(buf);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
    }
    Err(BufError::IndexOutOfBounds)
}

pub fn set(v: &[u8], chain: &mut SetIter) -> Result<usize, BufError> {
    let mut ptr_src = v.as_ptr();
    let mut n = v.len();
    for mut block in chain {
        let off = block.read_pos() as isize;
        let ptr_dst = unsafe { block.as_mut_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            return Ok(v.len());
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_src = ptr_src.offset(len as isize);
        }
    }
    Err(BufError::IndexOutOfBounds)
}

pub fn append(v: &[u8], chain: &mut Appender) -> Result<usize, ()> {
    let mut ptr_src = v.as_ptr();
    let mut n = v.len();
    loop {
        if let Some(mut block) = chain.last_mut() {
            let off = block.write_pos();
            let ptr_dst = unsafe { block.as_mut_ptr().offset(off as isize) };
            let appendable = block.appendable();
            if appendable >= n {
                unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
                block.set_write_pos(off + n);
                return Ok(v.len());
            }
            n -= appendable;
            unsafe {
                ptr::copy_nonoverlapping(ptr_src, ptr_dst, appendable);
                ptr_src = ptr_src.offset(appendable as isize);
            }
            let cap = block.capacity();
            block.set_write_pos(cap);
        }
        chain.append(0);
    }
}

pub fn append_bytes(v: Vec<u8>, chain: &mut Appender) -> Result<usize, ()> {
    let n = v.len();
    if n < 6 * 1024 {
        append(&v, chain)
    } else {
        chain.append_bytes(v);
        Ok(n)
    }
}

pub fn prepend(v: &[u8], chain: &mut Prepender) -> Result<usize, ()> {
    let mut ptr_src = v.as_ptr();
    let mut n = v.len();
    unsafe { ptr_src = ptr_src.offset(n as isize) };
    loop {
        if let Some(mut block) = chain.first_mut() {
            let prependable = block.prependable();
            let mut ptr_dst = block.as_mut_ptr();
            if prependable >= n {
                let off = prependable - n;
                unsafe {
                    ptr_dst = ptr_dst.offset(off as isize);
                    ptr_src = ptr_src.offset(-(n as isize));
                    ptr::copy_nonoverlapping(ptr_src, ptr_dst, n);
                }
                block.set_read_pos(off);
                return Ok(v.len());
            }
            n -= prependable;
            unsafe {
                ptr_src = ptr_src.offset(-(prependable as isize));
                ptr::copy_nonoverlapping(ptr_src, ptr_dst, prependable);
            }
            block.set_read_pos(0);
        }
        chain.prepend(0);
    }
}

pub fn prepend_bytes(v: Vec<u8>, chain: &mut Prepender) -> Result<usize, ()> {
    let n = v.len();
    if n < 6 * 1024 {
        prepend(&v, chain)
    } else {
        chain.prepend_bytes(v);
        Ok(n)
    }
}

pub fn set_fill(filling: Filling, chain: &mut SetIter) -> Result<usize, BufError> {
    let mut n = filling.count();
    for mut block in chain {
        let off = block.read_pos() as isize;
        let ptr_dst = unsafe { block.as_mut_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::write_bytes(ptr_dst, filling.val(), n) };
            return Ok(filling.count());
        }
        n -= len;
        unsafe { ptr::write_bytes(ptr_dst, filling.val(), len) };
    }
    Err(BufError::IndexOutOfBounds)
}

pub fn append_fill(filling: Filling, chain: &mut Appender) -> Result<usize, ()> {
    let mut n = filling.count();
    loop {
        if let Some(mut block) = chain.last_mut() {
            let off = block.write_pos();
            let ptr_dst = unsafe { block.as_mut_ptr().offset(off as isize) };
            let appendable = block.appendable();
            if appendable >= n {
                unsafe { ptr::write_bytes(ptr_dst, filling.val(), n) };
                block.set_write_pos(off + n);
                return Ok(filling.count());
            }
            n -= appendable;
            unsafe { ptr::write_bytes(ptr_dst, filling.val(), appendable) };
            let cap = block.capacity();
            block.set_write_pos(cap);
        }
        chain.append(0);
    }
}

pub fn prepend_fill(filling: Filling, chain: &mut Prepender) -> Result<usize, ()> {
    let mut n = filling.count();
    loop {
        if let Some(mut block) = chain.first_mut() {
            let prependable = block.prependable();
            let mut ptr_dst = block.as_mut_ptr();
            if prependable >= n {
                let off = prependable - n;
                unsafe {
                    ptr_dst = ptr_dst.offset(off as isize);
                    ptr::write_bytes(ptr_dst, filling.val(), n);
                }
                block.set_read_pos(off);
                return Ok(filling.count());
            }
            n -= prependable;
            unsafe { ptr::write_bytes(ptr_dst, filling.val(), prependable) };
            block.set_read_pos(0);
        }
        chain.prepend(0);
    }
}
