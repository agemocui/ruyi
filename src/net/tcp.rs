use std::io;
use std::net::{IpAddr, SocketAddr, Shutdown, ToSocketAddrs};

use net2::TcpBuilder;

use futures::{Poll, Stream, Async};

use super::super::nio;
use super::super::io::{AsyncRead, AsyncWrite};
use super::super::reactor::PollableIo;
use super::super::other_io_err;

#[derive(Debug)]
pub struct TcpStream {
    inner: PollableIo<nio::TcpStream>,
}

impl TcpStream {
    #[inline]
    fn from(sock: nio::TcpStream) -> Self {
        TcpStream { inner: PollableIo::new(sock) }
    }

    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().peer_addr()
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.get_ref().local_addr()
    }

    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.get_ref().shutdown(how)
    }

    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.get_ref().set_nodelay(nodelay)
    }

    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.get_ref().nodelay()
    }

    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.get_ref().set_ttl(ttl)
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.get_ref().ttl()
    }

    #[inline]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.get_ref().take_error()
    }
}

impl io::Read for TcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.get_mut().read(buf)
    }
}

impl nio::ReadV for TcpStream {
    #[inline]
    fn readv(&mut self, iovs: &[nio::IoVec]) -> io::Result<usize> {
        self.inner.get_mut().readv(iovs)
    }
}

impl AsyncRead for TcpStream {
    #[inline]
    fn need_read(&mut self) -> io::Result<()> {
        self.inner.need_read()
    }

    #[inline]
    fn no_need_read(&mut self) -> io::Result<()> {
        self.inner.no_need_read()
    }

    #[inline]
    fn is_readable(&self) -> bool {
        self.inner.is_readable()
    }
}

impl io::Write for TcpStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.get_mut().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.get_mut().flush()
    }
}

impl nio::WriteV for TcpStream {
    #[inline]
    fn writev(&mut self, iovs: &[nio::IoVec]) -> io::Result<usize> {
        self.inner.get_mut().writev(iovs)
    }
}

impl AsyncWrite for TcpStream {
    #[inline]
    fn need_write(&mut self) -> io::Result<()> {
        self.inner.need_write()
    }

    #[inline]
    fn no_need_write(&mut self) -> io::Result<()> {
        self.inner.no_need_write()
    }

    #[inline]
    fn is_writable(&self) -> bool {
        self.inner.is_writable()
    }
}

pub struct Incoming {
    io: PollableIo<nio::TcpListener>,
}

pub struct TcpListenerBuilder {
    addr: String,
    port: u16,
    backlog: i32,
    ttl: Option<u32>,
    only_v6: Option<bool>,
}

pub struct TcpListener {
    inner: nio::TcpListener,
}

impl TcpListener {
    #[inline]
    pub fn builder() -> TcpListenerBuilder {
        Default::default()
    }

    #[inline]
    pub fn incoming(self) -> Incoming {
        Incoming { io: PollableIo::new(self.inner) }
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.set_ttl(ttl)
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.ttl()
    }

    #[inline]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    #[inline]
    fn from(inner: nio::TcpListener) -> Self {
        TcpListener { inner }
    }
}

impl Default for TcpListenerBuilder {
    #[inline]
    fn default() -> Self {
        TcpListenerBuilder {
            addr: "0.0.0.0".to_string(),
            port: 0,
            backlog: 128,
            ttl: None,
            only_v6: None,
        }
    }
}

impl TcpListenerBuilder {
    #[inline]
    pub fn addr<A: Into<String>>(mut self, addr: Option<A>) -> Self {
        self.addr = match addr {
            Some(addr) => addr.into(),
            None => "0.0.0.0".to_string(),
        };
        self
    }

    #[inline]
    pub fn port(mut self, port: u16) -> Self {
        self.port = port;
        self
    }

    #[inline]
    pub fn backlog(mut self, backlog: i32) -> Self {
        self.backlog = backlog;
        self
    }

    #[inline]
    pub fn ttl(mut self, ttl: Option<u32>) -> Self {
        self.ttl = ttl;
        self
    }

    #[inline]
    pub fn only_v6(mut self, only_v6: Option<bool>) -> Self {
        self.only_v6 = only_v6;
        self
    }

    pub fn build(self) -> io::Result<TcpListener> {
        let addr =
            self.addr
                .parse::<IpAddr>()
                .map_err(|_| other_io_err(format!("Error to parse address: {}", self.addr)))?;
        let builder = match addr {
            IpAddr::V4(_) => TcpBuilder::new_v4()?,
            IpAddr::V6(_) => TcpBuilder::new_v6()?,
        };
        if let Some(ttl) = self.ttl {
            builder.ttl(ttl)?;
        }
        if let Some(only_v6) = self.only_v6 {
            builder.only_v6(only_v6)?;
        }
        let bind_addr = SocketAddr::new(addr, self.port);
        let listener = builder
            .reuse_address(true)?
            .bind(bind_addr)?
            .listen(self.backlog)?;
        listener.set_nonblocking(true)?;
        Ok(TcpListener::from(nio::TcpListener::from(listener)))
    }
}

impl Stream for Incoming {
    type Item = (TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.io.get_ref().accept() {
            Ok((s, a)) => Ok(Async::Ready(Some((TcpStream::from(s), a)))),
            Err(e) => {
                match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        self.io.need_read()?;
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                }
            }
        }
    }
}

pub struct TcpConnector {
    _io: PollableIo<nio::TcpStream>,
}

impl TcpConnector {}

pub fn connect<A: ToSocketAddrs>(_addr: A) -> io::Result<TcpConnector> {
    unimplemented!()
}
