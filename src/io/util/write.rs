use std::io;
use std::mem;

use futures::{Poll, Future, Async};

use io::AsyncWrite;
use buf::ByteBuf;

enum State<W> {
    Writing { buf: ByteBuf, w: W },
    Done,
}

pub struct Write<W> {
    state: State<W>,
}

#[inline]
pub fn write<W>(buf: ByteBuf, w: W) -> Write<W>
    where W: AsyncWrite
{
    Write { state: State::Writing { buf, w } }
}

impl<W> Future for Write<W>
    where W: AsyncWrite
{
    type Item = (ByteBuf, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let (mut buf, mut w) = match mem::replace(&mut self.state, State::Done) {
            State::Writing { buf, w } => (buf, w),
            State::Done => ::unreachable(),
        };
        if !w.is_writable() {
            self.state = State::Writing { buf, w };
            return Ok(Async::NotReady);
        }
        match buf.write_out(&mut w) {
            Ok(_) => {
                buf.compact();
                match buf.is_empty() {
                    true => w.no_need_write()?,
                    false => w.need_write()?,
                }
                Ok(Async::Ready((buf, w)))
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        w.need_write()?;
                        self.state = State::Writing { buf, w };
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                }
            }
        }
    }
}
