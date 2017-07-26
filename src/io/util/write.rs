use std::io;
use std::mem;

use futures::{Async, Future, Poll};

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

impl<W> Write<W>
where
    W: AsyncWrite,
{
    #[inline]
    fn poll_write(&mut self) -> Result<Option<usize>, io::Error> {
        match self.state {
            State::Writing {
                ref mut w,
                ref mut data,
            } => {
                if !w.is_writable() {
                    return Ok(None);
                }
                match data.write_out(w) {
                    Ok(n) => {
                        data.compact();
                        match data.is_empty() {
                            true => w.no_need_write()?,
                            false => w.need_write()?,
                        }
                        Ok(Some(n))
                    }
                    Err(e) => match e.kind() {
                        io::ErrorKind::WouldBlock => {
                            w.need_write()?;
                            return Ok(None);
                        }
                        _ => return Err(e),
                    },
                }
            }
            State::Done => panic!("Attempted to poll Write after completion"),
        }
    }
}

impl<W> Future for Write<W>
where
    W: AsyncWrite,
{
    type Item = (ByteBuf, usize, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.poll_write() {
            Ok(Some(n)) => match mem::replace(&mut self.state, State::Done) {
                State::Writing { w, data } => Ok(Async::Ready((data, n, w))),
                State::Done => ::unreachable(),
            },
            Ok(None) => Ok(Async::NotReady),
            Err(e) => {
                self.state = State::Done;
                Err(e)
            }
        }
    }
}
