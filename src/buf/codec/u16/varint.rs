use buf::{Appender, Error, GetIter, Prepender, ReadIter, SetIter};

pub fn read(chain: &mut ReadIter) -> Result<u16, Error> {
    let mut v = 0u16;
    let mut shift = 0;
    for mut block in chain {
        let ptr_u8 = block.as_ptr();
        let write_pos = block.write_pos();
        for pos in block.read_pos()..write_pos {
            let b = unsafe { *ptr_u8.offset(pos as isize) } as u16;
            v |= (b & 0x7F).wrapping_shl(shift);
            if b & !0x7F == 0 {
                block.set_read_pos(pos + 1);
                return Ok(v);
            }
            shift = shift.wrapping_add(7);
        }
        block.set_read_pos(write_pos);
    }
    Err(Error::Underflow)
}

pub fn get(chain: &mut GetIter) -> Result<u16, Error> {
    let mut v = 0u16;
    let mut shift = 0;
    for block in chain {
        let ptr_u8 = block.as_ptr();
        for pos in block.read_pos()..block.write_pos() {
            let b = unsafe { *ptr_u8.offset(pos as isize) } as u16;
            v |= (b & 0x7F).wrapping_shl(shift);
            if b & !0x7F == 0 {
                return Ok(v);
            }
            shift = shift.wrapping_add(7);
        }
    }
    Err(Error::IndexOutOfBounds)
}

pub fn set(mut v: u16, chain: &mut SetIter) -> Result<usize, Error> {
    let mut n = 0usize;
    for mut block in chain {
        let ptr_u8 = block.as_mut_ptr();
        for pos in block.read_pos()..block.write_pos() {
            n += 1;
            if v & !0x7F == 0 {
                unsafe { *ptr_u8.offset(pos as isize) = v as u8 };
                return Ok(n);
            }
            let b = (v | 0x80) as u8;
            unsafe { *ptr_u8.offset(pos as isize) = b };
            v >>= 7;
        }
    }
    Err(Error::IndexOutOfBounds)
}

pub fn append(mut v: u16, chain: &mut Appender) -> Result<usize, ()> {
    let mut n = 0usize;
    loop {
        if let Some(mut block) = chain.last_mut() {
            let cap = block.capacity();
            let ptr_u8 = block.as_mut_ptr();
            for pos in block.write_pos()..cap {
                n += 1;
                if v & !0x7F == 0 {
                    unsafe { *ptr_u8.offset(pos as isize) = v as u8 };
                    block.set_write_pos(pos + 1);
                    return Ok(n);
                }
                let b = (v | 0x80) as u8;
                unsafe { *ptr_u8.offset(pos as isize) = b };
                v >>= 7;
            }
            block.set_write_pos(cap);
        }
        chain.append(0);
    }
}

pub fn prepend(v: u16, chain: &mut Prepender) -> Result<usize, ()> {
    let mut shift = 0usize;
    while (v >> shift) > 0x7F {
        shift += 7;
    }
    let mut n = 0usize;
    let mut flag = 0u16;
    loop {
        if let Some(mut block) = chain.first_mut() {
            for pos in (0..block.read_pos()).rev() {
                n += 1;
                let b = (v >> shift | flag) as u8;
                unsafe { *block.as_mut_ptr().offset(pos as isize) = b };
                if shift == 0 {
                    block.set_read_pos(pos);
                    return Ok(n);
                }
                flag = 0x80;
                shift -= 7;
            }
            block.set_read_pos(0);
        }
        chain.prepend(0);
    }
}
