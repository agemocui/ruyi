use std::io;
use std::mem;

use futures::{Async, Future, Poll};

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

impl<R, W> Copy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    #[inline]
    fn poll_copy(&mut self) -> Result<bool, io::Error> {
        let mut done_read = false;
        let mut eof = false;
        match self.r {
            ReadState::Reading(ref mut r) => if r.is_readable() {
                match super::read(r) {
                    Ok(Some((data, _))) => {
                        match self.buf {
                            Some(ref mut buf) => buf.extend(data),
                            None => self.buf = Some(data),
                        };
                        r.need_read()?;
                    }
                    Ok(None) => eof = true,
                    Err(e) => match e.kind() {
                        io::ErrorKind::WouldBlock => r.need_read()?,
                        _ => return Err(e),
                    },
                }
            },
            ReadState::Done(..) => done_read = true,
            _ => panic!("Attempted to poll Copy after completion"),
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
                    return Ok(false);
                }
                if let Some(ref mut buf) = self.buf {
                    match buf.write_out(w) {
                        Ok(n) => self.len += n as u64,
                        Err(e) => {
                            return match e.kind() {
                                io::ErrorKind::WouldBlock => {
                                    w.need_write()?;
                                    Ok(false)
                                }
                                _ => Err(e),
                            };
                        }
                    }
                    buf.compact();
                    if buf.is_empty() {
                        w.no_need_write()?;
                    } else {
                        w.need_write()?;
                        return Ok(false);
                    }
                }
            }
            None => ::unreachable(),
        }

        Ok(done_read)
    }
}

impl<R, W> Future for Copy<R, W>
where
    R: AsyncRead,
    W: AsyncWrite,
{
    type Item = (u64, R, W);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.poll_copy() {
            Ok(false) => Ok(Async::NotReady),
            Ok(true) => match mem::replace(&mut self.r, ReadState::Dead) {
                ReadState::Done(r) => match mem::replace(&mut self.w, None) {
                    Some(w) => Ok(Async::Ready((self.len, r, w))),
                    _ => ::unreachable(),
                },
                _ => ::unreachable(),
            },
            Err(e) => {
                self.r = ReadState::Dead;
                Err(e)
            }
        }
    }
}
