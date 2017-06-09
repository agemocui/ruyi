use std::fmt;
use std::mem;
use std::str;

use super::ByteBuf;

pub struct HexDump<'a> {
    inner: &'a ByteBuf,
}

#[inline]
pub(super) fn new<'a>(inner: &'a ByteBuf) -> HexDump<'a> {
    HexDump { inner }
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
