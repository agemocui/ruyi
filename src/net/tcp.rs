use std::cell::UnsafeCell;
use std::io;
use std::net::{SocketAddr, Shutdown};
use std::rc::Rc;

use futures::{Poll, Stream, Sink, Async, StartSend, AsyncSink};

use super::super::buf::{ByteBuf, Appender};
use super::super::nio::{self, Ops};
use super::super::reactor::{self, IntoStream, Split, PollableIo};

pub struct Incoming {
    io: PollableIo<nio::TcpListener>,
}

impl IntoStream for nio::TcpListener {
    type Stream = Incoming;

    #[inline]
    fn into_stream(self) -> Self::Stream {
        Incoming { io: reactor::register(self) }
    }
}

impl Stream for Incoming {
    type Item = (nio::TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.io.get_ref().accept() {
            Ok((stream, addr)) => Ok(Async::Ready(Some((stream, addr)))),
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        self.io.interest_ops(Ops::read())?;
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                }
            }
        }
    }
}

#[derive(Debug)]
pub struct IStream {
    io: Rc<UnsafeCell<PollableIo<nio::TcpStream>>>,
    would_block: bool,
}

#[derive(Debug)]
pub struct OStream {
    io: Rc<UnsafeCell<PollableIo<nio::TcpStream>>>,
    buf: Option<ByteBuf>,
}

impl IStream {
    #[inline]
    fn new(io: Rc<UnsafeCell<PollableIo<nio::TcpStream>>>) -> Self {
        IStream {
            io: io,
            would_block: false,
        }
    }
}

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
        if chain.last_mut().appendable() < v {
            chain.append(v);
        }
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

impl Stream for IStream {
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.would_block {
            self.would_block = false;
            let io = unsafe { &mut *(&self.io).get() };
            let ops = io.interested_ops() | Ops::read();
            io.interest_ops(ops)?;
            return Ok(Async::NotReady);
        }
        RECV_BUF.with(|recv_buf| {
            let buf = recv_buf.get_mut();
            let io = unsafe { &mut *(&self.io).get() };
            match buf.read_in(io.get_mut()) {
                Ok(n) => {
                    match n {
                        0 => Ok(Async::Ready(None)),
                        _ => {
                            self.would_block = true;
                            Ok(Async::Ready(Some(buf.drain_to(n)?)))
                        }
                    }
                }
                Err(e) => {
                    match e.kind() {
                        io::ErrorKind::WouldBlock => {
                            let ops = io.interested_ops() | Ops::read();
                            io.interest_ops(ops)?;
                            Ok(Async::NotReady)
                        }
                        _ => Err(e),
                    }
                }
            }
        })
    }
}

impl OStream {
    #[inline]
    fn new(io: Rc<UnsafeCell<PollableIo<nio::TcpStream>>>) -> Self {
        OStream { io: io, buf: None }
    }
}

impl Sink for OStream {
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
        let io = unsafe { &mut *(&self.io).get() };
        if let Some(mut buf) = self.buf.take() {
            buf.write_out(io.get_mut())?;
            if !buf.is_empty() {
                let ops = io.interested_ops() | Ops::write();
                io.interest_ops(ops)?;
                buf.compact();
                self.buf = Some(buf);
                return Ok(Async::NotReady);
            }
        }
        let ops = io.interested_ops() - Ops::write();
        io.interest_ops(ops)?;
        return Ok(Async::Ready(()));
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        match self.poll_complete()? {
            Async::Ready(()) => {
                let sock = unsafe { &*(&self.io).get() };
                sock.get_ref().shutdown(Shutdown::Write)?;
                Ok(Async::Ready(()))
            }
            Async::NotReady => Ok(Async::NotReady),
        }
    }
}

impl Split for nio::TcpStream {
    type Stream = IStream;
    type Sink = OStream;

    fn split(self) -> (Self::Stream, Self::Sink) {
        let io = Rc::new(UnsafeCell::new(reactor::register(self)));
        (IStream::new(io.clone()), OStream::new(io))
    }
}
