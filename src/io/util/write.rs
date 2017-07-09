use std::io;
use std::mem;

use futures::{Poll, Future, Async};

use io::AsyncWrite;
use buf::ByteBuf;

enum State<W> {
    Writing { w: W, data: ByteBuf },
    Done,
}

pub struct Write<W> {
    state: State<W>,
}

#[inline]
pub fn write<W>(w: W, data: ByteBuf) -> Write<W>
where
    W: AsyncWrite,
{
    Write {
        state: State::Writing { w, data },
    }
}

impl<W> Future for Write<W>
where
    W: AsyncWrite,
{
    type Item = (W, ByteBuf, usize);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let n = match self.state {
            State::Writing {
                ref mut w,
                ref mut data,
            } => {
                if !w.is_writable() {
                    return Ok(Async::NotReady);
                }
                match data.write_out(w) {
                    Ok(n) => {
                        data.compact();
                        match data.is_empty() {
                            true => w.no_need_write()?,
                            false => w.need_write()?,
                        }
                        n
                    }
                    Err(e) => {
                        match e.kind() {
                            io::ErrorKind::WouldBlock => {
                                w.need_write()?;
                                return Ok(Async::NotReady);
                            }
                            _ => return Err(e),
                        }
                    }
                }
            }
            State::Done => ::unreachable(),
        };
        match mem::replace(&mut self.state, State::Done) {
            State::Writing { w, data } => Ok(Async::Ready((w, data, n))),
            State::Done => ::unreachable(),
        }
    }
}
