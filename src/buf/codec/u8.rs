use std::io::{self, Error, ErrorKind};

use super::super::{ReadIter, GetIter, SetIter, Appender, Prepender};

const U8_SIZE: usize = 1;

pub fn read(chain: &mut ReadIter) -> io::Result<u8> {
    if let Some(mut block) = chain.next() {
        let off = block.read_pos();
        block.set_read_pos(off + U8_SIZE);
        return Ok(unsafe { *block.as_ptr().offset(off as isize) });
    }
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::u8::read"))
}

pub fn get(chain: &mut GetIter) -> io::Result<u8> {
    if let Some(block) = chain.next() {
        let off = block.read_pos() as isize;
        return Ok(unsafe { *block.as_ptr().offset(off) });
    }
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::u8::get"))
}

pub fn set(v: u8, chain: &mut SetIter) -> io::Result<usize> {
    if let Some(mut block) = chain.next() {
        let off = block.read_pos() as isize;
        unsafe { *block.as_mut_ptr().offset(off) = v };
        return Ok(U8_SIZE);
    }
    Err(Error::new(ErrorKind::UnexpectedEof, "codec::u8::set"))
}

pub fn append(v: u8, chain: &mut Appender) -> io::Result<usize> {
    loop {
        if let Some(mut block) = chain.last_mut() {
            if block.appendable() > 0 {
                let off = block.write_pos();
                unsafe { *block.as_mut_ptr().offset(off as isize) = v };
                block.set_write_pos(off + U8_SIZE);
                return Ok(U8_SIZE);
            }
        }
        chain.append(0);
    }
}

pub fn prepend(v: u8, chain: &mut Prepender) -> io::Result<usize> {
    loop {
        if let Some(mut block) = chain.first_mut() {
            if block.prependable() > 0 {
                let off = block.read_pos() - U8_SIZE;
                block.set_read_pos(off);
                unsafe { *block.as_mut_ptr().offset(off as isize) = v };
                return Ok(U8_SIZE);
            }
        }
        chain.prepend(0);
    }
}
