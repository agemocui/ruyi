use std::mem;
use std::ptr;

use buf::{Appender, BufError, GetIter, Prepender, ReadIter, SetIter};
use buf::codec::reverse;
use buf::codec::u16::U16_SIZE;

#[inline]
pub fn read(chain: &mut ReadIter) -> Result<u16, BufError> {
    let mut v: u16 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = U16_SIZE;
    for mut block in chain {
        let off = block.read_pos();
        let ptr_src = unsafe { block.as_ptr().offset(off as isize) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "little") {
                reverse(unsafe { mem::transmute(&mut v) }, U16_SIZE);
            }
            block.set_read_pos(off + n);
            return Ok(v);
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

pub fn get(chain: &mut GetIter) -> Result<u16, BufError> {
    let mut v: u16 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = U16_SIZE;
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr_src = unsafe { block.as_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "little") {
                reverse(unsafe { mem::transmute(&mut v) }, U16_SIZE);
            }
            return Ok(v);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
    }
    Err(BufError::IndexOutOfBounds)
}

pub fn set(mut v: u16, chain: &mut SetIter) -> Result<usize, BufError> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "little") {
        reverse(ptr_src, U16_SIZE);
    }
    let mut n = U16_SIZE;
    for mut block in chain {
        let off = block.read_pos() as isize;
        let ptr_dst = unsafe { block.as_mut_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            return Ok(U16_SIZE);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_src = ptr_src.offset(len as isize);
        }
    }
    Err(BufError::IndexOutOfBounds)
}

pub fn append(mut v: u16, chain: &mut Appender) -> Result<usize, ()> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "little") {
        reverse(ptr_src, U16_SIZE);
    }
    let mut n = U16_SIZE;
    loop {
        if let Some(mut block) = chain.last_mut() {
            let off = block.write_pos();
            let ptr_dst = unsafe { block.as_mut_ptr().offset(off as isize) };
            let appendable = block.appendable();
            if appendable >= n {
                unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
                block.set_write_pos(off + n);
                return Ok(U16_SIZE);
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

pub fn prepend(mut v: u16, chain: &mut Prepender) -> Result<usize, ()> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "little") {
        reverse(ptr_src, U16_SIZE);
    }
    let mut n = U16_SIZE;
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
                return Ok(U16_SIZE);
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
