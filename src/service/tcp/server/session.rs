use std::fmt;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering};

use io::{AsyncRead, AsyncWrite};
use net::TcpStream;
use nio::{ReadV, WriteV, IoVec};

#[derive(Debug)]
pub struct Session {
    conn: TcpStream,
    conn_count: &'static AtomicUsize,
}

impl Session {
    #[inline]
    pub fn new(conn: TcpStream, conn_count: &'static AtomicUsize) -> Self {
        Session { conn, conn_count }
    }

    #[inline]
    pub fn as_tcp_stream(&self) -> &TcpStream {
        &self.conn
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
