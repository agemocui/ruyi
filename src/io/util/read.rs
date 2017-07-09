use std::io;
use std::mem;

use futures::{Poll, Future, Async};

use io::AsyncRead;
use buf::ByteBuf;

enum State<R> {
    Reading { r: R },
    Done,
}

pub struct Read<R> {
    state: State<R>,
}

#[inline]
pub fn read<R>(r: R) -> Read<R>
where
    R: AsyncRead,
{
    Read {
        state: State::Reading { r },
    }
}

impl<R> Future for Read<R>
where
    R: AsyncRead,
{
    type Item = (R, Option<ByteBuf>, usize);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (data, n) = match self.state {
            State::Reading { ref mut r } => {
                if !r.is_readable() {
                    return Ok(Async::NotReady);
                }
                match super::read(r) {
                    Ok((data, n)) => {
                        r.need_read()?;
                        (data, n)
                    }
                    Err(e) => {
                        match e.kind() {
                            io::ErrorKind::WouldBlock => {
                                r.need_read()?;
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
            State::Reading { r } => Ok(Async::Ready((r, data, n))),
            State::Done => ::unreachable(),
        }
    }
}
