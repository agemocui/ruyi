use std::io;

use nio::{ReadV, WriteV};

pub trait AsyncRead: io::Read + ReadV {
    fn need_read(&mut self) -> io::Result<()>;

    fn no_need_read(&mut self) -> io::Result<()>;

    fn is_readable(&self) -> bool;
}

pub trait AsyncWrite: io::Write + WriteV {
    fn need_write(&mut self) -> io::Result<()>;

    fn no_need_write(&mut self) -> io::Result<()>;

    fn is_writable(&self) -> bool;
}

impl<'a, R: AsyncRead> AsyncRead for &'a mut R {
    #[inline]
    fn need_read(&mut self) -> io::Result<()> {
        AsyncRead::need_read(*self)
    }

    #[inline]
    fn no_need_read(&mut self) -> io::Result<()> {
        AsyncRead::no_need_read(*self)
    }

    #[inline]
    fn is_readable(&self) -> bool {
        AsyncRead::is_readable(*self)
    }
}

impl<'a, W: AsyncWrite> AsyncWrite for &'a mut W {
    #[inline]
    fn need_write(&mut self) -> io::Result<()> {
        AsyncWrite::need_write(*self)
    }

    #[inline]
    fn no_need_write(&mut self) -> io::Result<()> {
        AsyncWrite::no_need_write(*self)
    }

    #[inline]
    fn is_writable(&self) -> bool {
        AsyncWrite::is_writable(*self)
    }
}

mod util;

pub use self::util::{IStream, OStream};
pub use self::util::read::*;
pub use self::util::write::*;
pub use self::util::copy::*;
pub use self::util::split::*;
