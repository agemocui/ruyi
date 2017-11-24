use std::fmt;
use std::sync::atomic::{AtomicUsize, Ordering};

use net::TcpStream;

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
}

impl AsRef<TcpStream> for Session {
    #[inline]
    fn as_ref(&self) -> &TcpStream {
        &self.conn
    }
}

impl AsMut<TcpStream> for Session {
    #[inline]
    fn as_mut(&mut self) -> &mut TcpStream {
        &mut self.conn
    }
}

impl fmt::Display for Session {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.conn, f)
    }
}

impl Drop for Session {
    #[inline]
    fn drop(&mut self) {
        self.conn_count.fetch_sub(1, Ordering::Relaxed);
    }
}
