use std::cell::UnsafeCell;
use std::io;

use buf::{ByteBuf, Appender};
use io::{AsyncRead, AsyncWrite};

const RECV_BUF_SIZE: usize = 128 * 1024;

struct RecvBuf {
    inner: UnsafeCell<ByteBuf>,
}

impl RecvBuf {
    #[inline]
    fn new() -> Self {
        RecvBuf {
            inner: UnsafeCell::new(ByteBuf::with_capacity(RECV_BUF_SIZE)),
        }
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

#[inline]
fn read<R>(r: &mut R) -> io::Result<Option<(ByteBuf, usize)>>
where
    R: AsyncRead,
{
    RECV_BUF.with(|recv_buf| {
        let buf = recv_buf.get_mut();
        let n = buf.read_in(r)?;
        let data = if n > 0 {
            Some((buf.drain_to(n)?, n))
        } else {
            None
        };
        Ok(data)
    })
}

pub mod read;
pub mod write;
pub mod copy;
pub mod split;

use futures::{Stream, Sink, Poll, Async, StartSend, AsyncSink};
use sink::IntoSink;
use stream::IntoStream;

#[derive(Debug)]
pub struct IStream<R> {
    r: Option<R>,
    would_block: bool,
}

impl<R> IStream<R>
where
    R: AsyncRead,
{
    #[inline]
    pub fn get_ref(&self) -> &R {
        self.r
            .as_ref()
            .expect("Attempted IStream::get_ref after completion")
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut R {
        self.r
            .as_mut()
            .expect("Attempted IStream::get_mut after completion")
    }

    #[inline]
    pub fn into_inner(self) -> R {
        self.r
            .expect("Attempted IStream::into_inner after completion")
    }

    #[inline]
    fn poll_read(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        match self.r {
            Some(ref mut r) => {
                if self.would_block {
                    self.would_block = false;
                    r.need_read()?;
                    return Ok(Async::NotReady);
                }
                if !r.is_readable() {
                    return Ok(Async::NotReady);
                }
                match read(r) {
                    Ok(Some((data, _))) => {
                        self.would_block = true;
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
            None => panic!("Attempted to poll IStream after completion"),
        }
    }
}

impl<R> Stream for IStream<R>
where
    R: AsyncRead,
{
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.poll_read() {
            Ok(o) => Ok(o),
            Err(e) => {
                self.r = None;
                Err(e)
            }
        }
    }
}

#[derive(Debug)]
pub struct OStream<W> {
    w: Option<W>,
    buf: Option<ByteBuf>,
}

impl<W> OStream<W>
where
    W: AsyncWrite,
{
    #[inline]
    pub fn get_ref(&self) -> &W {
        self.w
            .as_ref()
            .expect("Attempted OStream::get_ref after completion")
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut W {
        self.w
            .as_mut()
            .expect("Attempted OStream::get_mut after completion")
    }

    #[inline]
    pub fn into_inner(self) -> W {
        self.w
            .expect("Attempted OStream::into_inner after completion")
    }

    #[inline]
    fn poll_write(&mut self) -> Poll<(), io::Error> {
        match self.w {
            Some(ref mut w) => {
                if w.is_writable() {
                    if let Some(ref mut data) = self.buf {
                        if let Err(e) = data.write_out(w) {
                            return match e.kind() {
                                io::ErrorKind::WouldBlock => {
                                    w.need_write()?;
                                    Ok(Async::NotReady)
                                }
                                _ => Err(e),
                            };
                        }
                        if !data.is_empty() {
                            w.need_write()?;
                            data.compact();
                            return Ok(Async::NotReady);
                        }
                    }
                    w.no_need_write()?;
                    Ok(Async::Ready(()))
                } else {
                    Ok(Async::NotReady)
                }
            }
            None => panic!("Attempted to poll OStream after completion"),
        }
    }
}

impl<W> Sink for OStream<W>
where
    W: AsyncWrite,
{
    type SinkItem = ByteBuf;
    type SinkError = io::Error;

    fn start_send(&mut self, bytes: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        let buf = match self.buf.take() {
            Some(mut buf) => {
                buf.extend(bytes);
                buf
            }
            None => bytes,
        };
        self.buf = Some(buf);
        self.poll_complete()?;
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        match self.poll_write() {
            Ok(Async::Ready(())) => {
                self.buf = None;
                Ok(Async::Ready(()))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(e) => {
                self.w = None;
                Err(e)
            }
        }
    }

    #[inline]
    fn close(&mut self) -> Poll<(), Self::SinkError> {
        self.poll_complete()
    }
}

impl<R: AsyncRead> IntoStream for R {
    type Stream = IStream<Self>;

    #[inline]
    fn into_stream(self) -> Self::Stream {
        IStream {
            r: Some(self),
            would_block: false,
        }
    }
}

impl<W: AsyncWrite> IntoSink for W {
    type Sink = OStream<Self>;

    #[inline]
    fn into_sink(self) -> Self::Sink {
        OStream {
            w: Some(self),
            buf: None,
        }
    }
}
