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

impl<R> Read<R>
where
    R: AsyncRead,
{
    #[inline]
    fn poll_read(&mut self) -> Poll<Option<(ByteBuf, usize)>, io::Error> {
        match self.state {
            State::Reading { ref mut r } => {
                if !r.is_readable() {
                    return Ok(Async::NotReady);
                }
                match super::read(r) {
                    Ok(Some(data)) => {
                        r.need_read()?;
                        Ok(Async::Ready(Some(data)))
                    }
                    Ok(None) => Ok(Async::Ready(None)),
                    Err(e) => {
                        match e.kind() {
                            io::ErrorKind::WouldBlock => {
                                r.need_read()?;
                                Ok(Async::NotReady)
                            }
                            _ => Err(e),
                        }
                    }
                }
            }
            State::Done => panic!("Attempted to poll Read after completion"),
        }
    }
}

impl<R> Future for Read<R>
where
    R: AsyncRead,
{
    type Item = (Option<(ByteBuf, usize)>, R);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.poll_read() {
            Ok(Async::Ready(data)) => {
                match mem::replace(&mut self.state, State::Done) {
                    State::Reading { r } => Ok(Async::Ready((data, r))),
                    State::Done => ::unreachable(),
                }
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => {
                self.state = State::Done;
                Err(e)
            }
        }
    }
}
