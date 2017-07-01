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
where
    R: AsyncRead,
    W: AsyncWrite,
{
    Copy {
        r: ReadState::Reading(r),
        w: Some(w),
        buf: None,
        len: 0,
    }
}

impl<R, W> Future for Copy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    type Item = (R, W, u64);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut done_read = false;
        let mut eof = false;
        match self.r {
            ReadState::Reading(ref mut r) => {
                if r.is_readable() {
                    match super::read(r) {
                        Ok((data, _)) => {
                            match data {
                                None => eof = true,
                                Some(bytes) => {
                                    match self.buf {
                                        Some(ref mut buf) => buf.extend(bytes),
                                        None => self.buf = Some(bytes),
                                    };
                                    r.need_read()?;
                                }
                            }
                        }
                        Err(e) => {
                            match e.kind() {
                                io::ErrorKind::WouldBlock => r.need_read()?,
                                _ => return Err(e),
                            }
                        }
                    }
                }
            }
            ReadState::Done(..) => done_read = true,
            _ => ::unreachable(),
        }
        if eof {
            match mem::replace(&mut self.r, ReadState::Dead) {
                ReadState::Reading(r) => {
                    self.r = ReadState::Done(r);
                    done_read = true;
                }
                _ => ::unreachable(),
            }
        }

        match self.w {
            Some(ref mut w) => {
                if !w.is_writable() {
                    return Ok(Async::NotReady);
                }
                if let Some(ref mut buf) = self.buf {
                    self.len += buf.write_out(w)? as u64;
                    buf.compact();
                    match buf.is_empty() {
                        false => {
                            w.need_write()?;
                            return Ok(Async::NotReady);
                        }
                        true => w.no_need_write()?,
                    }
                }
            }
            None => ::unreachable(),
        }
        match done_read {
            true => {
                match mem::replace(&mut self.r, ReadState::Dead) {
                    ReadState::Done(r) => {
                        match mem::replace(&mut self.w, None) {
                            Some(w) => Ok(Async::Ready((r, w, self.len))),
                            _ => ::unreachable(),
                        }
                    }
                    _ => ::unreachable(),
                }
            }
            false => Ok(Async::NotReady),
        }
    }
}
