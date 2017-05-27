use std::cell::UnsafeCell;
use std::io;

use super::super::buf::{ByteBuf, Appender};
use super::AsyncRead;

const RECV_BUF_SIZE: usize = 128 * 1024;

struct RecvBuf {
    inner: UnsafeCell<ByteBuf>,
}

impl RecvBuf {
    #[inline]
    fn new() -> Self {
        RecvBuf { inner: UnsafeCell::new(ByteBuf::with_capacity(RECV_BUF_SIZE)) }
    }

    #[inline]
    fn append(v: usize, chain: &mut Appender) -> io::Result<usize> {
        if let Some(block) = chain.last_mut() {
            if block.appendable() >= v {
                return Ok(0);
            }
        }
        chain.append(v);
        Ok(0)
    }

    #[inline]
    fn get_mut(&self) -> &mut ByteBuf {
        let buf = unsafe { &mut *self.inner.get() };
        buf.append(RECV_BUF_SIZE, Self::append).unwrap();
        buf
    }
}

thread_local!(static RECV_BUF: RecvBuf = RecvBuf::new());

fn read<R>(r: &mut R) -> io::Result<Option<ByteBuf>>
    where R: AsyncRead
{
    RECV_BUF.with(|recv_buf| {
                      let buf = recv_buf.get_mut();
                      let n = buf.read_in(r)?;
                      let data = if n > 0 { Some(buf.drain_to(n)?) } else { None };
                      Ok(data)
                  })
}

pub mod read;
pub mod write;
pub mod copy;
pub mod split;
