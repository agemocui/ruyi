use std::cell::UnsafeCell;
use std::io;
use std::mem;
use std::net::{self, SocketAddr};
use std::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use std::rc::Rc;
use std::marker::PhantomData;

use libc;

use futures::{Async, Poll};

use buf::{Block, ByteBuf, GetIter};
use net::{TcpListener, TcpStream};
use sys::nio::BorrowMut;
use sys::unix::err::cvt;
use sys::unix::nio::{IoVec, Nio, Readv, Writev};
use sys::unix::syscall::{accept, socket_v4, socket_v6};

////////////////////////////////////////////////////////////////////////////////
// TcpListener

impl AsRawFd for TcpListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

pub(crate) struct Incoming {
    nio: Nio<TcpListener>,
}

impl Incoming {
    #[inline]
    pub(crate) fn try_from(listener: TcpListener) -> io::Result<Self> {
        Ok(Incoming {
            nio: Nio::try_from(listener)?,
        })
    }

    #[inline]
    pub(crate) fn poll_accept(&mut self) -> Poll<Option<(TcpStream, SocketAddr)>, io::Error> {
        match accept(self.nio.get_ref().as_raw_fd()) {
            Ok((s, a)) => Ok(Async::Ready(Some((
                unsafe { TcpStream::from_raw_fd(s) },
                a,
            )))),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    self.nio.schedule_read()?;
                    Ok(Async::NotReady)
                }
                _ => Err(e),
            },
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// TcpStream

impl AsRawFd for TcpStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

impl FromRawFd for TcpStream {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        TcpStream::from(net::TcpStream::from_raw_fd(fd))
    }
}

// TODO: Make BUF_SIZE configurable
const BUF_SIZE: usize = 128 * 1024;

struct Buffer {
    block1: Block,
    block2: Block,
    reverse: bool,
}

impl Buffer {
    #[inline]
    fn new() -> Self {
        Buffer {
            block1: Block::with_capacity(BUF_SIZE),
            block2: Block::with_capacity(BUF_SIZE),
            reverse: false,
        }
    }

    fn readv<R>(&mut self, r: &mut R) -> io::Result<Option<(ByteBuf, usize)>>
    where
        R: Readv,
    {
        let (block1, block2) = match self.reverse {
            true => (&mut self.block2, &mut self.block1),
            false => (&mut self.block1, &mut self.block2),
        };
        let iovs = [
            IoVec::from((block1.as_ptr(), block1.capacity())),
            IoVec::from((block2.as_ptr(), block2.capacity())),
        ];
        let n = r.readv(&iovs)?;
        if n == 0 {
            return Ok(None);
        }
        let mut buf = ByteBuf::new();
        if n <= block1.appendable() {
            block1.set_write_pos(n);
            let mut block = block1.split_off(n);
            mem::swap(block1, &mut block);
            buf.add_block(block);
            // TODO: min appendable should be configurable
            if block1.appendable() < mem::size_of::<usize>() {
                *block1 = Block::with_capacity(BUF_SIZE);
            }
        } else {
            let len = n - block1.appendable();
            let cap = block1.capacity();
            block1.set_write_pos(cap);
            let mut block = Block::with_capacity(BUF_SIZE);
            mem::swap(block1, &mut block);
            buf.add_block(block);

            block2.set_write_pos(len);
            block = block2.split_off(len);
            mem::swap(block2, &mut block);
            buf.add_block(block);
            // TODO: min appendable should be configurable
            match block2.appendable() < mem::size_of::<usize>() {
                true => *block2 = Block::with_capacity(BUF_SIZE),
                false => self.reverse = !self.reverse,
            }
        }
        Ok(Some((buf, n)))
    }
}

struct IStream<T: AsRef<TcpStream>, B: BorrowMut<Nio<TcpStream, T>> = Nio<TcpStream, T>> {
    nio: B,
    _marker: PhantomData<T>,
}

impl<T, B> IStream<T, B>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
    B: BorrowMut<Nio<TcpStream, T>>,
{
    #[inline]
    pub(super) fn from(b: B) -> Self {
        IStream {
            nio: b,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub(super) fn get_ref(&self) -> &T {
        self.nio.borrow().get_ref()
    }

    #[inline]
    pub(super) fn get_mut(&mut self) -> &mut T {
        self.nio.borrow_mut().get_mut()
    }

    #[inline]
    pub(super) fn poll_recv(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        let nio = self.nio.borrow_mut();
        if !nio.is_read_ready() {
            return Ok(Async::NotReady);
        }
        match Self::readv(nio.get_mut().as_mut()) {
            Ok(Some((data, _))) => {
                nio.schedule_read()?;
                Ok(Async::Ready(Some(data)))
            }
            Ok(None) => Ok(Async::Ready(None)),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    nio.schedule_read()?;
                    Ok(Async::NotReady)
                }
                _ => Err(e),
            },
        }
    }

    #[inline]
    fn readv(stream: &mut TcpStream) -> io::Result<Option<(ByteBuf, usize)>> {
        thread_local!(static BUFFER: UnsafeCell<Buffer> = UnsafeCell::new(Buffer::new()));

        BUFFER.with(|buf| unsafe { (*buf.get()).readv(stream) })
    }
}


#[inline]
fn get_iovs(chain: &mut GetIter) -> io::Result<Vec<IoVec>> {
    let mut iovecs = Vec::new();
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr = unsafe { block.as_ptr().offset(off) };
        iovecs.push(IoVec::from((ptr, block.len())));
    }
    Ok(iovecs)
}

struct OStream<T: AsRef<TcpStream>, B: BorrowMut<Nio<TcpStream, T>> = Nio<TcpStream, T>> {
    nio: B,
    _marker: PhantomData<T>,
}

impl<T, B> OStream<T, B>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
    B: BorrowMut<Nio<TcpStream, T>>,
{
    #[inline]
    pub(super) fn from(b: B) -> Self {
        OStream {
            nio: b,
            _marker: PhantomData,
        }
    }

    #[inline]
    pub(super) fn get_ref(&self) -> &T {
        self.nio.borrow().get_ref()
    }

    #[inline]
    pub(super) fn get_mut(&mut self) -> &mut T {
        self.nio.borrow_mut().get_mut()
    }

    #[inline]
    pub fn poll_send(&mut self, data: &mut ByteBuf) -> Poll<(), io::Error> {
        if data.is_empty() {
            return Ok(Async::Ready(()));
        }
        let nio = self.nio.borrow_mut();
        if !nio.is_write_ready() {
            return Ok(Async::NotReady);
        }
        match Self::writev(nio.get_mut().as_mut(), data) {
            Ok(_) => {
                data.compact();
                if data.is_empty() {
                    nio.cancel_write()?;
                    Ok(Async::Ready(()))
                } else {
                    nio.schedule_write()?;
                    Ok(Async::NotReady)
                }
            }
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    nio.schedule_write()?;
                    Ok(Async::NotReady)
                }
                _ => Err(e),
            },
        }
    }

    #[inline]
    fn writev(stream: &mut TcpStream, data: &mut ByteBuf) -> io::Result<()> {
        let iovs = data.get(0, get_iovs)?;
        let n = stream.writev(iovs.as_slice())?;
        data.skip(n);
        Ok(())
    }
}

pub struct Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: IStream<T>,
}

impl<T> Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        Ok(Recv {
            inner: IStream::from(Nio::try_from(io)?),
        })
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    #[inline]
    pub fn poll_recv(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        self.inner.poll_recv()
    }

    #[inline]
    pub fn into_2way(self) -> (RecvHalf<T>, SendHalf<T>) {
        let nio_r = Rc::new(UnsafeCell::new(self.inner.nio));
        let nio_s = nio_r.clone();
        (
            RecvHalf {
                inner: IStream::from(nio_r),
            },
            SendHalf {
                inner: OStream::from(nio_s),
            },
        )
    }
}

pub struct Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: OStream<T>,
}

impl<T> Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        Ok(Sender {
            inner: OStream::from(Nio::try_from(io)?),
        })
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    #[inline]
    pub fn poll_send(&mut self, data: &mut ByteBuf) -> Poll<(), io::Error> {
        self.inner.poll_send(data)
    }

    #[inline]
    pub fn into_2way(self) -> (RecvHalf<T>, SendHalf<T>) {
        let nio_s = Rc::new(UnsafeCell::new(self.inner.nio));
        let nio_r = nio_s.clone();
        (
            RecvHalf {
                inner: IStream::from(nio_r),
            },
            SendHalf {
                inner: OStream::from(nio_s),
            },
        )
    }
}

pub struct RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: IStream<T, Rc<UnsafeCell<Nio<TcpStream, T>>>>,
}

impl<T> RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    #[inline]
    pub fn poll_recv(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        self.inner.poll_recv()
    }
}

pub struct SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: OStream<T, Rc<UnsafeCell<Nio<TcpStream, T>>>>,
}

impl<T> SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.inner.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }

    #[inline]
    pub fn poll_send(&mut self, data: &mut ByteBuf) -> Poll<(), io::Error> {
        self.inner.poll_send(data)
    }
}

#[inline]
pub fn split<T>(io: T) -> io::Result<(RecvHalf<T>, SendHalf<T>)>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    let nio_r = Rc::new(UnsafeCell::new(Nio::try_from(io)?));
    let nio_s = nio_r.clone();
    Ok((
        RecvHalf {
            inner: IStream::from(nio_r),
        },
        SendHalf {
            inner: OStream::from(nio_s),
        },
    ))
}

#[derive(Debug)]
enum ConnectState<T: AsRef<TcpStream>> {
    Connecting(Nio<TcpStream, T>),
    Finishing(Nio<TcpStream, T>),
    Connected(Nio<TcpStream, T>),
    Error(io::Error),
    Done,
}

#[derive(Debug)]
pub struct Connect<T: AsRef<TcpStream>> {
    state: ConnectState<T>,
}

impl<T> Connect<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn poll_connect(&mut self) -> Poll<Sender<T>, io::Error> {
        use self::ConnectState::*;
        match mem::replace(&mut self.state, Done) {
            Connecting(mut nio) => {
                nio.schedule_write()?;
                self.state = Finishing(nio);
                Ok(Async::NotReady)
            }
            Finishing(mut nio) => if nio.is_write_ready() {
                match nio.get_ref().as_ref().as_inner().take_error()? {
                    None => {
                        nio.cancel_write()?;
                        Ok(Async::Ready(Sender {
                            inner: OStream::from(nio),
                        }))
                    }
                    Some(e) => Err(e),
                }
            } else {
                self.state = Finishing(nio);
                Ok(Async::NotReady)
            },
            Connected(nio) => Ok(Async::Ready(Sender {
                inner: OStream::from(nio),
            })),
            Error(e) => Err(e),
            Done => panic!("Attempted to poll Connect after completion"),
        }
    }
}

impl<T> Connect<T>
where
    T: AsRef<TcpStream> + From<TcpStream>,
{
    pub fn from(addr: &SocketAddr) -> Self {
        let state = match Self::connect(addr) {
            Ok(s) => s,
            Err(e) => ConnectState::Error(e),
        };
        Connect { state }
    }

    #[inline]
    fn connect(addr: &SocketAddr) -> io::Result<ConnectState<T>> {
        let (stream, addr, len) = match *addr {
            SocketAddr::V4(ref a) => (
                unsafe { TcpStream::from_raw_fd(socket_v4()?) },
                a as *const _ as *const _,
                mem::size_of_val(a) as libc::socklen_t,
            ),
            SocketAddr::V6(ref a) => (
                unsafe { TcpStream::from_raw_fd(socket_v6()?) },
                a as *const _ as *const _,
                mem::size_of_val(a) as libc::socklen_t,
            ),
        };
        let nio = Nio::try_from(T::from(stream))?;
        let res = unsafe { libc::connect(nio.get_ref().as_ref().as_raw_fd(), addr, len) };
        use self::ConnectState::*;
        match cvt(res) {
            Err(e) => match e.raw_os_error() {
                Some(libc::EINPROGRESS) => Ok(Connecting(nio)),
                _ => Err(e),
            },
            Ok(..) => Ok(Connected(nio)),
        }
    }
}
