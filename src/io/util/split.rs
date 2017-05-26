use std::cell::UnsafeCell;
use std::io;
use std::rc::Rc;

use super::super::{AsyncRead, AsyncWrite};
use super::super::super::nio;

pub struct ReadHalf<R> {
    inner: Rc<UnsafeCell<R>>,
}

pub struct WriteHalf<W> {
    inner: Rc<UnsafeCell<W>>,
}

#[inline]
pub fn split<T>(t: T) -> (ReadHalf<T>, WriteHalf<T>)
    where T: AsyncRead + AsyncWrite
{
    let io = Rc::new(UnsafeCell::new(t));
    (ReadHalf { inner: io.clone() }, WriteHalf { inner: io })
}

impl<R> ReadHalf<R> {
    #[inline]
    fn get_mut(&self) -> &mut R {
        unsafe { &mut *(&self.inner).get() }
    }
}

impl<W> WriteHalf<W> {
    #[inline]
    fn get_mut(&self) -> &mut W {
        unsafe { &mut *(&self.inner).get() }
    }
}

impl<R: AsyncRead> io::Read for ReadHalf<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.get_mut().read(buf)
    }
}

impl<R: AsyncRead> nio::ReadV for ReadHalf<R> {
    #[inline]
    fn readv(&mut self, iovs: &[nio::IoVec]) -> io::Result<usize> {
        self.get_mut().readv(iovs)
    }
}

impl<R: AsyncRead> AsyncRead for ReadHalf<R> {
    #[inline]
    fn need_read(&mut self) -> io::Result<()> {
        self.get_mut().need_read()
    }

    #[inline]
    fn no_need_read(&mut self) -> io::Result<()> {
        self.get_mut().no_need_read()
    }

    #[inline]
    fn is_readable(&self) -> bool {
        self.get_mut().is_readable()
    }
}

impl<W: AsyncWrite> io::Write for WriteHalf<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.get_mut().write(buf)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.get_mut().flush()
    }
}

impl<W: AsyncWrite> nio::WriteV for WriteHalf<W> {
    #[inline]
    fn writev(&mut self, iovs: &[nio::IoVec]) -> io::Result<usize> {
        self.get_mut().writev(iovs)
    }
}

impl<W: AsyncWrite> AsyncWrite for WriteHalf<W> {
    #[inline]
    fn need_write(&mut self) -> io::Result<()> {
        self.get_mut().need_write()
    }

    #[inline]
    fn no_need_write(&mut self) -> io::Result<()> {
        self.get_mut().no_need_write()
    }

    #[inline]
    fn is_writable(&self) -> bool {
        self.get_mut().is_writable()
    }
}
