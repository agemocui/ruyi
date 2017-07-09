use std::fmt;
use std::io;
use std::net::{Shutdown, SocketAddr};
use std::sync::atomic::{AtomicUsize, Ordering};

use io::{AsyncRead, AsyncWrite};
use net::TcpStream;
use nio::{ReadV, WriteV, IoVec};

#[derive(Debug)]
pub struct Session {
    conn: TcpStream,
    peer_addr: Option<SocketAddr>,
    conn_count: &'static AtomicUsize,
}

impl Session {
    #[inline]
    pub fn new(
        conn: TcpStream,
        peer_addr: Option<SocketAddr>,
        conn_count: &'static AtomicUsize,
    ) -> Self {
        Session {
            conn,
            peer_addr,
            conn_count,
        }
    }

    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        match self.peer_addr {
            Some(ref addr) => Ok(*addr),
            None => self.conn.peer_addr(),
        }
    }

    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.conn.local_addr()
    }

    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.conn.shutdown(how)
    }

    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.conn.set_nodelay(nodelay)
    }

    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.conn.nodelay()
    }

    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.conn.set_ttl(ttl)
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.conn.ttl()
    }
}

impl fmt::Display for Session {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.conn.fmt(f)
    }
}

impl io::Read for Session {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.conn.read(buf)
    }
}

impl ReadV for Session {
    #[inline]
    fn readv(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        self.conn.readv(iovs)
    }
}

impl AsyncRead for Session {
    #[inline]
    fn need_read(&mut self) -> io::Result<()> {
        self.conn.need_read()
    }

    #[inline]
    fn no_need_read(&mut self) -> io::Result<()> {
        self.conn.no_need_read()
    }

    #[inline]
    fn is_readable(&self) -> bool {
        self.conn.is_readable()
    }
}

impl io::Write for Session {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.conn.write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.conn.flush()
    }
}

impl WriteV for Session {
    #[inline]
    fn writev(&mut self, iovs: &[IoVec]) -> io::Result<usize> {
        self.conn.writev(iovs)
    }
}

impl AsyncWrite for Session {
    #[inline]
    fn need_write(&mut self) -> io::Result<()> {
        self.conn.need_write()
    }

    #[inline]
    fn no_need_write(&mut self) -> io::Result<()> {
        self.conn.no_need_write()
    }

    #[inline]
    fn is_writable(&self) -> bool {
        self.conn.is_writable()
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.conn_count.fetch_sub(1, Ordering::Relaxed);
    }
}
