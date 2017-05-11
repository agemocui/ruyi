use std::fmt;
use std::io;
use std::mem;
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd};

use libc;

use super::Selector;
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

impl TcpStream {
    #[inline]
    pub fn register<I, T>(&self, selector: &Selector, interest: I, token: T) -> io::Result<()>
        where I: Into<usize>,
              T: Into<usize>
    {
        selector.register(self.inner.as_raw_fd(), interest, token)
    }

    #[inline]
    pub fn reregister<I, T>(&self, selector: &Selector, interest: I, token: T) -> io::Result<()>
        where I: Into<usize>,
              T: Into<usize>
    {
        selector.reregister(self.inner.as_raw_fd(), interest, token)
    }

    #[inline]
    pub fn deregister(&self, selector: &Selector) -> io::Result<()> {
        selector.deregister(self.inner.as_raw_fd())
    }

    #[inline]
    pub fn as_inner(&self) -> &net::TcpStream {
        &self.inner
    }

    #[inline]
    pub fn as_inner_mut(&mut self) -> &mut net::TcpStream {
        &mut self.inner
    }

    #[inline]
    pub fn readv(&mut self, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
        readv(self.inner.as_raw_fd(), iov_ptr, len)
    }

    #[inline]
    pub fn writev(&mut self, iov_ptr: *const IoVec, len: usize) -> io::Result<usize> {
        writev(self.inner.as_raw_fd(), iov_ptr, len)
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

impl TcpListener {
    #[inline]
    pub fn register<I, T>(&self, selector: &Selector, interest: I, token: T) -> io::Result<()>
        where I: Into<usize>,
              T: Into<usize>
    {
        selector.register(self.inner.as_raw_fd(), interest, token)
    }

    #[inline]
    pub fn reregister<I, T>(&self, selector: &Selector, interest: I, token: T) -> io::Result<()>
        where I: Into<usize>,
              T: Into<usize>
    {
        selector.reregister(self.inner.as_raw_fd(), interest, token)
    }

    #[inline]
    pub fn deregister(&self, selector: &Selector) -> io::Result<()> {
        selector.deregister(self.inner.as_raw_fd())
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
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut storage: libc::sockaddr_storage = unsafe { mem::uninitialized() };
        let mut len = mem::size_of::<libc::sockaddr_storage>() as libc::socklen_t;
        let res = unsafe {
            libc::accept4(self.inner.as_raw_fd(),
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
