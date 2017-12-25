use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};

const U8_SIZE: usize = 1;

pub fn read(chain: &mut ReadIter) -> Result<u8, Error> {
    if let Some(mut block) = chain.next() {
        let off = block.read_pos();
        block.set_read_pos(off + U8_SIZE);
        return Ok(unsafe { *block.as_ptr().offset(off as isize) });
    }
    Err(Error::Underflow)
}

pub fn get(chain: &mut GetIter) -> Result<u8, Error> {
    if let Some(block) = chain.next() {
        let off = block.read_pos() as isize;
        return Ok(unsafe { *block.as_ptr().offset(off) });
    }
    Err(Error::IndexOutOfBounds)
}

pub fn set(v: u8, chain: &mut SetIter) -> Result<usize, Error> {
    if let Some(mut block) = chain.next() {
        let off = block.read_pos() as isize;
        unsafe { *block.as_mut_ptr().offset(off) = v };
        return Ok(U8_SIZE);
    }
    Err(Error::IndexOutOfBounds)
}

pub fn append(v: u8, chain: &mut Appender) -> Result<usize, ()> {
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

pub fn prepend(v: u8, chain: &mut Prepender) -> Result<usize, ()> {
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
