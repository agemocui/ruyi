use std::fmt;
use std::io;
use std::net::{self, Shutdown, SocketAddr};

use super::{sys, IoVec, ReadV, WriteV};
use super::poll::{Ops, Pollable, Poller, Token};

pub struct TcpStream {
    inner: net::TcpStream,
}

pub struct TcpListener {
    inner: net::TcpListener,
}

impl From<net::TcpListener> for TcpListener {
    #[inline]
    fn from(inner: net::TcpListener) -> Self {
        TcpListener { inner }
    }
}

impl TcpListener {
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
        sys::accept(&self.inner).map(|(sock, addr)| (TcpStream::from(sock), addr))
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

impl TcpStream {
    #[inline]
    fn from(sock: net::TcpStream) -> Self {
        TcpStream { inner: sock }
    }

    #[inline]
    pub fn connect(addr: &SocketAddr) -> io::Result<(Self, bool)> {
        let (sock, connected) = sys::connect(addr)?;
        Ok((Self::from(sock), connected))
    }

    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.inner.peer_addr()
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.inner.local_addr()
    }

    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
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
        self.inner.fmt(f)
    }
}

impl io::Read for TcpStream {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }
}

impl io::Write for TcpStream {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

impl ReadV for TcpStream {
    #[inline]
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        sys::readv(&self.inner, iovs.as_ptr() as *const sys::IoVec, iovs.len())
    }
}

impl WriteV for TcpStream {
    #[inline]
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        sys::writev(&self.inner, iovs.as_ptr() as *const sys::IoVec, iovs.len())
    }
}
