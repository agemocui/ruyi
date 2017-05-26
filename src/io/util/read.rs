use std::io;
use std::mem;

use futures::{Poll, Future, Async};

use super::super::AsyncRead;
use super::super::super::buf::ByteBuf;
use super::super::super::unreachable;

enum State<R> {
    Reading { r: R },
    Done,
}

pub struct Read<R> {
    state: State<R>,
}

#[inline]
pub fn read<R>(r: R) -> Read<R>
    where R: AsyncRead
{
    Read { state: State::Reading { r } }
}

impl<R> Future for Read<R>
    where R: AsyncRead
{
    type Item = (Option<ByteBuf>, R);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut r = match mem::replace(&mut self.state, State::Done) {
            State::Reading { r } => r,
            State::Done => unreachable(),
        };
        if !r.is_readable() {
            self.state = State::Reading { r };
            return Ok(Async::NotReady);
        }
        match super::read(&mut r) {
            Ok(data) => {
                match data {
                    Some(bytes) => {
                        r.need_read()?;
                        Ok(Async::Ready((Some(bytes), r)))
                    }
                    None => Ok(Async::Ready((None, r))),
                }
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        r.need_read()?;
                        self.state = State::Reading { r };
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                }
            }
        }
    }
}
