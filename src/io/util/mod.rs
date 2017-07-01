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
        RecvBuf { inner: UnsafeCell::new(ByteBuf::with_capacity(RECV_BUF_SIZE)) }
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

fn read<R>(r: &mut R) -> io::Result<(Option<ByteBuf>, usize)>
where
    R: AsyncRead,
{
    RECV_BUF.with(|recv_buf| {
        let buf = recv_buf.get_mut();
        let n = buf.read_in(r)?;
        let data = if n > 0 { Some(buf.drain_to(n)?) } else { None };
        Ok((data, n))
    })
}

pub mod read;
pub mod write;
pub mod copy;
pub mod split;

use futures::{Stream, Sink, Poll, Async, StartSend, AsyncSink};
use reactor::{IntoStream, IntoSink};

#[derive(Debug)]
pub struct IStream<R> {
    r: R,
    would_block: bool,
}

impl<R: AsyncRead> Stream for IStream<R> {
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.would_block {
            self.would_block = false;
            self.r.need_read()?;
            return Ok(Async::NotReady);
        }
        if !self.r.is_readable() {
            return Ok(Async::NotReady);
        }
        match read(&mut self.r) {
            Ok((data, _)) => {
                match data {
                    Some(bytes) => {
                        self.would_block = true;
                        Ok(Async::Ready(Some(bytes)))
                    }
                    None => Ok(Async::Ready(None)),
                }
            }
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        self.r.need_read()?;
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct OStream<W> {
    w: W,
    buf: Option<ByteBuf>,
}

impl<W: AsyncWrite> Sink for OStream<W> {
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
        if !self.w.is_writable() {
            return Ok(Async::NotReady);
        }
        if let Some(mut buf) = self.buf.take() {
            buf.write_out(&mut self.w)?;
            if !buf.is_empty() {
                self.w.need_write()?;
                buf.compact();
                self.buf = Some(buf);
                return Ok(Async::NotReady);
            }
        }
        self.w.no_need_write()?;
        return Ok(Async::Ready(()));
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
            r: self,
            would_block: false,
        }
    }
}

impl<W: AsyncWrite> IntoSink for W {
    type Sink = OStream<Self>;

    #[inline]
    fn into_sink(self) -> Self::Sink {
        OStream { w: self, buf: None }
    }
}
