pub mod tcp;
pub use self::tcp::{TcpListener, TcpListenerBuilder, TcpStream};

#[inline]
pub fn init() {
    ::sys::net::init()
}
