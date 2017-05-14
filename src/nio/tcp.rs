use std::fmt;
use std::io;
use std::net::{self, IpAddr, SocketAddr, Shutdown};

use net2::TcpBuilder;

use super::{sys, IoVec, ReadV, WriteV};
use super::poll::{Ops, Token, Pollable, Poller};
use super::super::other_io_err;

pub struct TcpStream {
    inner: sys::TcpStream,
}

pub struct TcpListener {
    inner: sys::TcpListener,
}

pub struct TcpListenerBuilder {
    addr: String,
    port: u16,
    backlog: i32,
    ttl: Option<u32>,
    only_v6: Option<bool>,
}

impl TcpListener {
    #[inline]
    pub fn builder() -> TcpListenerBuilder {
        Default::default()
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

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        self.inner
            .accept()
            .map(|(sock, addr)| (TcpStream::from(sock), addr))
    }

    #[inline]
    fn from(listener: net::TcpListener) -> Self {
        TcpListener { inner: sys::TcpListener::from(listener) }
    }
}

impl Pollable for TcpListener {
    #[inline]
    fn register(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.register(poller, interest, token)
    }

    #[inline]
    fn reregister(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.reregister(poller, interest, token)
    }

    #[inline]
    fn deregister(&self, poller: &Poller) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

impl fmt::Debug for TcpListener {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
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
        Ok(TcpListener::from(listener))
    }
}

impl TcpStream {
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.as_inner().peer_addr()
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.as_inner().local_addr()
    }

    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.as_inner().shutdown(how)
    }

    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.as_inner().set_nodelay(nodelay)
    }

    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.as_inner().nodelay()
    }

    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.inner.as_inner().set_ttl(ttl)
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.inner.as_inner().ttl()
    }

    #[inline]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.as_inner().take_error()
    }
}

impl From<sys::TcpStream> for TcpStream {
    #[inline]
    fn from(sock: sys::TcpStream) -> Self {
        TcpStream { inner: sock }
    }
}

impl Pollable for TcpStream {
    #[inline]
    fn register(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.register(poller, interest, token)
    }

    #[inline]
    fn reregister(&self, poller: &Poller, interest: Ops, token: Token) -> io::Result<()> {
        self.inner.reregister(poller, interest, token)
    }

    #[inline]
    fn deregister(&self, poller: &Poller) -> io::Result<()> {
        self.inner.deregister(poller)
    }
}

impl fmt::Debug for TcpStream {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.as_inner().fmt(f)
    }
}

impl io::Read for TcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.as_inner_mut().read(buf)
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.as_inner_mut().read_to_end(buf)
    }
}

impl io::Write for TcpStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.as_inner_mut().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.as_inner_mut().flush()
    }
}

impl ReadV for TcpStream {
    #[inline]
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        self.inner
            .readv(iovs.as_ptr() as *const sys::IoVec, iovs.len())
    }
}

impl WriteV for TcpStream {
    #[inline]
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        self.inner
            .writev(iovs.as_ptr() as *const sys::IoVec, iovs.len())
    }
}
