use std::io::{self, Error, ErrorKind};
use std::ptr;

use super::super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

#[inline]
pub fn read(chain: &mut ReadIter) -> io::Result<String> {
    let len = chain.len();
    read_exact(chain, len)
}

pub fn read_exact(chain: &mut ReadIter, mut n: usize) -> io::Result<String> {
    if n == 0 {
        return Ok(String::new());
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
            return String::from_utf8(buf).map_err(|e| Error::new(ErrorKind::Other, e));
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
        let pos = block.write_pos();
        block.set_read_pos(pos);
    }
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::str::utf8::read_exact"))
}

#[inline]
pub fn get(chain: &mut GetIter) -> io::Result<String> {
    let len = chain.len();
    get_exact(chain, len)
}

pub fn get_exact(chain: &mut GetIter, mut n: usize) -> io::Result<String> {
    if n == 0 {
        return Ok(String::new());
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
            return String::from_utf8(buf).map_err(|e| Error::new(ErrorKind::Other, e));
        }
        n -= len;
        unsafe {
            ptr::copy_nonoverlapping(ptr_src, ptr_dst, len);
            ptr_dst = ptr_dst.offset(len as isize);
        }
    }
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::str::utf8::get_exact"))
}

pub fn set(v: &str, chain: &mut SetIter) -> io::Result<usize> {
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
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::str::utf8::set"))
}

pub fn append(v: &str, chain: &mut Appender) -> io::Result<usize> {
    let mut ptr_src = v.as_ptr();
    let mut n = v.len();
    loop {
        {
            let mut block = chain.last_mut();
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

pub fn prepend(v: &str, chain: &mut Prepender) -> io::Result<usize> {
    let mut ptr_src = v.as_ptr();
    let mut n = v.len();
    unsafe { ptr_src = ptr_src.offset(n as isize) };
    loop {
        {
            let mut block = chain.first_mut();
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
