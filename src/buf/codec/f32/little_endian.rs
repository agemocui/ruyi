use std::mem;
use std::io::{self, Error, ErrorKind};
use std::ptr;

use super::F32_SIZE;
use super::super::reverse;
use super::super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

pub fn read(chain: &mut ReadIter) -> io::Result<f32> {
    let mut v: f32 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = F32_SIZE;
    for mut block in chain {
        let off = block.read_pos();
        let ptr_src = unsafe { block.as_ptr().offset(off as isize) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "big") {
                reverse(unsafe { mem::transmute(&mut v) }, F32_SIZE);
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
        "codec::f32::little_endian::read",
    ))
}

pub fn get(chain: &mut GetIter) -> io::Result<f32> {
    let mut v: f32 = unsafe { mem::uninitialized() };
    let mut ptr_dst: *mut u8 = unsafe { mem::transmute(&mut v) };
    let mut n = F32_SIZE;
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr_src = unsafe { block.as_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            if cfg!(target_endian = "big") {
                reverse(unsafe { mem::transmute(&mut v) }, F32_SIZE);
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
        "codec::f32::little_endian::get",
    ))
}

pub fn set(mut v: f32, chain: &mut SetIter) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, F32_SIZE);
    }
    let mut n = F32_SIZE;
    for mut block in chain {
        let off = block.read_pos() as isize;
        let ptr_dst = unsafe { block.as_mut_ptr().offset(off) };
        let len = block.len();
        if len >= n {
            unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
            return Ok(F32_SIZE);
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_src = ptr_src.offset(len as isize);
        }
    }
    Err(Error::new(
        ErrorKind::UnexpectedEof,
        "codec::f32::little_endian::set",
    ))
}

pub fn append(mut v: f32, chain: &mut Appender) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, F32_SIZE);
    }
    let mut n = F32_SIZE;
    loop {
        if let Some(mut block) = chain.last_mut() {
            let off = block.write_pos();
            let ptr_dst = unsafe { block.as_mut_ptr().offset(off as isize) };
            let appendable = block.appendable();
            if appendable >= n {
                unsafe { ptr::copy_nonoverlapping(ptr_src, ptr_dst, n) };
                block.set_write_pos(off + n);
                return Ok(F32_SIZE);
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

pub fn prepend(mut v: f32, chain: &mut Prepender) -> io::Result<usize> {
    let mut ptr_src: *mut u8 = unsafe { mem::transmute(&mut v) };
    if cfg!(target_endian = "big") {
        reverse(ptr_src, F32_SIZE);
    }
    let mut n = F32_SIZE;
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
                return Ok(F32_SIZE);
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
