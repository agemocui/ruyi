use std::io;
use std::mem;

use futures::{Poll, Future, Async};

use io::{AsyncRead, AsyncWrite};
use buf::ByteBuf;

enum ReadState<R> {
    Reading(R),
    Done(R),
    Dead,
}

pub struct Copy<R, W> {
    r: ReadState<R>,
    w: Option<W>,
    buf: Option<ByteBuf>,
    len: u64,
}

#[inline]
pub fn copy<R, W>(r: R, w: W) -> Copy<R, W>
    where R: AsyncRead,
          W: AsyncWrite
{
    Copy {
        r: ReadState::Reading(r),
        w: Some(w),
        buf: None,
        len: 0,
    }
}

impl<R, W> Future for Copy<R, W>
    where R: AsyncRead,
          W: AsyncWrite
{
    type Item = (u64, R, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match mem::replace(&mut self.r, ReadState::Dead) {
            ReadState::Reading(mut r) => {
                if r.is_readable() {
                    match super::read(&mut r) {
                        Ok(data) => {
                            match data {
                                None => self.r = ReadState::Done(r),
                                Some(bytes) => {
                                    self.buf = match self.buf.take() {
                                        Some(mut buf) => {
                                            buf.extend(bytes);
                                            Some(buf)
                                        }
                                        None => Some(bytes),
                                    };
                                    r.need_read()?;
                                    self.r = ReadState::Reading(r);
                                }
                            }
                        }
                        Err(e) => {
                            match e.kind() {
                                io::ErrorKind::WouldBlock => r.need_read()?,
                                _ => return Err(e),
                            }
                            self.r = ReadState::Reading(r)
                        }
                    }
                } else {
                    self.r = ReadState::Reading(r)
                }
            }
            ReadState::Done(r) => self.r = ReadState::Done(r),
            _ => ::unreachable(),
        }

        match self.w.take() {
            Some(mut w) => {
                if !w.is_writable() {
                    return Ok(Async::NotReady);
                }
                if let Some(mut buf) = self.buf.take() {
                    self.len += buf.write_out(&mut w)? as u64;
                    buf.compact();
                    if !buf.is_empty() {
                        w.need_write()?;
                        self.buf = Some(buf);
                        self.w = Some(w);
                        return Ok(Async::NotReady);
                    } else {
                        w.no_need_write()?;
                    }
                }
                match mem::replace(&mut self.r, ReadState::Dead) {
                    ReadState::Reading(r) => {
                        self.w = Some(w);
                        self.r = ReadState::Reading(r);
                        Ok(Async::NotReady)
                    }
                    ReadState::Done(r) => Ok(Async::Ready((self.len, r, w))),
                    _ => ::unreachable(),
                }
            }
            None => ::unreachable(),
        }
    }
}
