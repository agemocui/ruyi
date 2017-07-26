use std::mem;
use std::io::{self, Error, ErrorKind};
use std::ptr;

use super::U64_SIZE;
use super::super::reverse;
use super::super::super::{Appender, GetIter, Prepender, ReadIter, SetIter};

pub fn read(chain: &mut ReadIter) -> io::Result<u64> {
    let mut v: u64 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = U64_SIZE;
    for mut block in chain {
        let off = block.read_pos();
        let ptr_src = unsafe { block.as_ptr().offset(off as isize) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "big") {
                reverse(unsafe { mem::transmute(&mut v) }, U64_SIZE);
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
    Err(Error::new(
        ErrorKind::UnexpectedEof,
        "codec::u64::little_endian::read",
    ))
}

pub fn get(chain: &mut GetIter) -> io::Result<u64> {
    let mut v: u64 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = U64_SIZE;
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr_src = unsafe { block.as_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "big") {
                reverse(unsafe { mem::transmute(&mut v) }, U64_SIZE);
            }
            return Ok(v);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
    }
    Err(Error::new(
        ErrorKind::UnexpectedEof,
        "codec::u64::little_endian::get",
    ))
}

pub fn set(mut v: u64, chain: &mut SetIter) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, U64_SIZE);
    }
    let mut n = U64_SIZE;
    for mut block in chain {
        let off = block.read_pos() as isize;
        let ptr_dst = unsafe { block.as_mut_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            return Ok(U64_SIZE);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_src = ptr_src.offset(len as isize);
        }
    }
    Err(Error::new(
        ErrorKind::UnexpectedEof,
        "codec::u64::little_endian::set",
    ))
}

pub fn append(mut v: u64, chain: &mut Appender) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, U64_SIZE);
    }
    let mut n = U64_SIZE;
    loop {
        if let Some(mut block) = chain.last_mut() {
            let off = block.write_pos();
            let ptr_dst = unsafe { block.as_mut_ptr().offset(off as isize) };
            let appendable = block.appendable();
            if appendable >= n {
                unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
                block.set_write_pos(off + n);
                return Ok(U64_SIZE);
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

pub fn prepend(mut v: u64, chain: &mut Prepender) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, U64_SIZE);
    }
    let mut n = U64_SIZE;
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
                return Ok(U64_SIZE);
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
