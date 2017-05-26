use std::fmt;
use std::io;
use std::mem;
use std::net::{self, SocketAddr, Shutdown};
use std::os::unix::io::{RawFd, AsRawFd, FromRawFd};

use libc;

use super::{io_result, readv, writev, IoVec};

pub struct TcpStream {
    inner: net::TcpStream,
}

impl From<net::TcpStream> for TcpStream {
    #[inline]
    fn from(stream: net::TcpStream) -> Self {
        TcpStream { inner: stream }
    }
}

impl AsRawFd for TcpStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
    }
}

impl fmt::Debug for TcpStream {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl TcpStream {
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

    #[inline]
    pub fn readv(&mut self, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
        readv(self.as_raw_fd(), iov_ptr, len)
    }

    #[inline]
    pub fn writev(&mut self, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
        writev(self.as_raw_fd(), iov_ptr, len)
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

pub struct TcpListener {
    inner: net::TcpListener,
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl From<net::TcpListener> for TcpListener {
    #[inline]
    fn from(listener: net::TcpListener) -> Self {
        TcpListener { inner: listener }
    }
}

impl AsRawFd for TcpListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.inner.as_raw_fd()
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

    #[inline]
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut storage: libc::sockaddr_storage = unsafe { mem::uninitialized() };
        let mut len = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let res = unsafe {
            libc::accept4(self.as_raw_fd(),
                          &mut storage as *mut _ as *mut _,
                          &mut len,
                          libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC)
        };
        let fd = io_result(res)?;
        let sock = unsafe { net::TcpStream::from_raw_fd(fd) };
        let addr = sockaddr_to_addr(&storage, len as usize)?;
        Ok((TcpStream::from(sock), addr))
    }
}

#[inline]
fn sockaddr_to_addr(storage: &libc::sockaddr_storage, len: usize) -> io::Result<SocketAddr> {
    match storage.ss_family as libc::c_int {
        libc::AF_INET => {
            assert!(len >= mem::size_of::<libc::sockaddr_in>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in) };
            Ok(SocketAddr::V4(unsafe { mem::transmute(addr) }))
        }
        libc::AF_INET6 => {
            assert!(len >= mem::size_of::<libc::sockaddr_in6>());
            let addr = unsafe { *(storage as *const _ as *const libc::sockaddr_in6) };
            Ok(SocketAddr::V6(unsafe { mem::transmute(addr) }))
        }
        _ => Err(io::Error::new(io::ErrorKind::InvalidInput, "invalid argument")),
    }
}
