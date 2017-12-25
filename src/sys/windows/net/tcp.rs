use std::cell::UnsafeCell;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::net::{self, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use std::os::windows::io::{AsRawSocket, FromRawSocket, RawSocket};
use std::ptr;
use std::rc::Rc;

use winapi;
use ws2_32;
use net2::TcpBuilder;
use futures::{Async, Poll};

use buf::{Block, ByteBuf, Error, GetIter};
use net::{TcpListener, TcpStream};
use sys::nio::BorrowMut;
use sys::windows::net::{get_overlapped_result, last_error, ACCEPTEX, CONNECTEX,
                        GET_ACCEPTEX_SOCKADDRS};
use sys::windows::nio::{IoVec, Nio, Overlapped, Readv, Writev};

////////////////////////////////////////////////////////////////////////////////
// TcpListener

type AcceptEx = unsafe extern "system" fn(
    winapi::SOCKET,       // _In_  sListenSocket
    winapi::SOCKET,       // _In_  sAcceptSocket
    winapi::PVOID,        // _In_  lpOutputBuffer
    winapi::DWORD,        // _In_  dwReceiveDataLength
    winapi::DWORD,        // _In_  dwLocalAddressLength
    winapi::DWORD,        // _In_  dwRemoteAddressLength
    winapi::LPDWORD,      // _Out_ lpdwBytesReceived
    winapi::LPOVERLAPPED, // _In_  lpOverlapped
) -> winapi::BOOL;

type GetAcceptExSockaddrs = unsafe extern "system" fn(
    winapi::PVOID,           // _In_ lpOutputBuffer
    winapi::DWORD,           // _In_ dwReceiveDataLength
    winapi::DWORD,           // _In_ dwLocalAddressLength
    winapi::DWORD,           // _In_ dwRemoteAddressLength
    *mut winapi::LPSOCKADDR, // _Out_ *LocalSockaddr
    winapi::LPINT,           // _Out_ LocalSockaddrLength
    *mut winapi::LPSOCKADDR, // _Out_ *RemoteSockaddr
    winapi::LPINT,           // _Out_ RemoteSockaddrLength
);

struct AcceptOutBuf {
    _local_addr: winapi::SOCKADDR_STORAGE,
    _remote_addr: winapi::SOCKADDR_STORAGE,
}

struct Var {
    accept_ex: AcceptEx,
    get_acceptex_sockaddrs: GetAcceptExSockaddrs,
    overlapped: Overlapped,
    accept_out_buf: AcceptOutBuf,
}

impl Var {
    #[inline]
    fn accept(&mut self, listener: &TcpListener, stream: &TcpStream) -> io::Result<SocketAddr> {
        let mut bytes = 0;
        let success = unsafe {
            (self.accept_ex)(
                listener.as_raw_socket(),
                stream.as_raw_socket(),
                &mut self.accept_out_buf as *mut _ as *mut _,
                0,
                mem::size_of::<winapi::SOCKADDR_STORAGE>() as winapi::DWORD,
                mem::size_of::<winapi::SOCKADDR_STORAGE>() as winapi::DWORD,
                &mut bytes,
                self.overlapped.as_mut(),
            )
        };
        match success == winapi::TRUE {
            true => self.finish_accept(listener, stream),
            false => Err(last_error()),
        }
    }

    #[inline]
    fn finish_accept(
        &mut self,
        listener: &TcpListener,
        stream: &TcpStream,
    ) -> io::Result<SocketAddr> {
        const SO_UPDATE_ACCEPT_CONTEXT: winapi::c_int = 0x700B;
        let listen_sock = listener.as_raw_socket();
        let result = unsafe {
            ws2_32::setsockopt(
                stream.as_raw_socket(),
                winapi::SOL_SOCKET,
                SO_UPDATE_ACCEPT_CONTEXT,
                &listen_sock as *const _ as *const _,
                mem::size_of_val(&listen_sock) as winapi::c_int,
            )
        };
        if result != 0 {
            return Err(last_error());
        }
        let remote_addr = self.parse_remote_addr()?;
        Ok(remote_addr)
    }

    #[inline]
    fn parse_remote_addr(&self) -> io::Result<SocketAddr> {
        let mut local_addr = ptr::null_mut();
        let mut local_addr_len = 0;
        let mut remote_addr = ptr::null_mut();
        let mut remote_addr_len = 0;
        unsafe {
            (self.get_acceptex_sockaddrs)(
                &self.accept_out_buf as *const _ as *mut _,
                0,
                mem::size_of::<winapi::SOCKADDR_STORAGE>() as winapi::DWORD,
                mem::size_of::<winapi::SOCKADDR_STORAGE>() as winapi::DWORD,
                &mut local_addr,
                &mut local_addr_len,
                &mut remote_addr,
                &mut remote_addr_len,
            )
        }
        let storage: &winapi::SOCKADDR_STORAGE = unsafe { mem::transmute(remote_addr) };
        let len = remote_addr_len as usize;
        match storage.ss_family as winapi::c_int {
            winapi::AF_INET => {
                debug_assert!(len >= mem::size_of::<winapi::SOCKADDR_IN>());
                let addr = unsafe { *(storage as *const _ as *const winapi::SOCKADDR_IN) };
                Ok(SocketAddr::V4(unsafe { mem::transmute(addr) }))
            }
            winapi::AF_INET6 => {
                debug_assert!(len >= mem::size_of::<winapi::sockaddr_in6>());
                let addr = unsafe { *(storage as *const _ as *const winapi::sockaddr_in6) };
                Ok(SocketAddr::V6(unsafe { mem::transmute(addr) }))
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "unknown address family",
            )),
        }
    }
}

impl AsRawSocket for TcpListener {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.as_inner().as_raw_socket()
    }
}

pub struct Incoming {
    nio: Nio<TcpListener>,
    var: Box<Var>,
    family: winapi::c_int,
    stream: Option<TcpStream>,
}

impl Incoming {
    #[inline]
    pub fn try_from(listener: TcpListener) -> io::Result<Self> {
        let accept_ex = ACCEPTEX.get(listener.as_raw_socket())?;
        let get_acceptex_sockaddrs = GET_ACCEPTEX_SOCKADDRS.get(listener.as_raw_socket())?;
        let family = match listener.as_inner().local_addr()?.is_ipv6() {
            true => winapi::AF_INET6,
            false => winapi::AF_INET,
        };
        Ok(Incoming {
            nio: Nio::try_from(listener)?,
            var: Box::new(Var {
                accept_ex: unsafe { mem::transmute(accept_ex) },
                get_acceptex_sockaddrs: unsafe { mem::transmute(get_acceptex_sockaddrs) },
                overlapped: Overlapped::for_read(),
                accept_out_buf: unsafe { mem::uninitialized() },
            }),
            family,
            stream: None,
        })
    }

    #[inline]
    pub fn poll_accept(&mut self) -> Poll<Option<(TcpStream, SocketAddr)>, io::Error> {
        if self.nio.is_read_ready() {
            if let Some(stream) = self.stream.take() {
                match get_overlapped_result(
                    self.nio.get_ref().as_raw_socket(),
                    &mut self.var.overlapped,
                ).and_then(|_| self.var.finish_accept(self.nio.get_ref(), &stream))
                {
                    Ok(addr) => return Ok(Async::Ready(Some((stream, addr)))),
                    Err(e) => warn!("{:?} failed to accept: {}", self.nio.get_ref(), e),
                }
            }
        } else {
            return Ok(Async::NotReady);
        }

        let stream = self.new_sock()?;
        match self.var.accept(self.nio.get_ref(), &stream) {
            Ok(addr) => Ok(Async::Ready(Some((stream, addr)))),
            Err(e) => match e.kind() {
                io::ErrorKind::WouldBlock => {
                    self.nio.schedule_read();
                    self.stream = Some(stream);
                    Ok(Async::NotReady)
                }
                _ => Err(e),
            },
        }
    }

    #[inline]
    fn new_sock(&self) -> io::Result<TcpStream> {
        const IPPROTO_TCP: winapi::c_int = 6;
        let accept_sock = unsafe { ws2_32::socket(self.family, winapi::SOCK_STREAM, IPPROTO_TCP) };
        if accept_sock == winapi::INVALID_SOCKET {
            return Err(last_error());
        }
        let stream = unsafe { TcpStream::from(net::TcpStream::from_raw_socket(accept_sock)) };
        Ok(stream)
    }
}

////////////////////////////////////////////////////////////////////////////////
// TcpStream

impl AsRawSocket for TcpStream {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.as_inner().as_raw_socket()
    }
}

impl Readv for TcpStream {
    unsafe fn readv(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize> {
        let iov_ptr = iovs.as_ptr() as *mut IoVec as winapi::LPWSABUF;
        let mut bytes = 0;
        let mut flags = 0;
        let ret = ws2_32::WSARecv(
            self.as_raw_socket(),
            iov_ptr,
            iovs.len() as winapi::DWORD,
            &mut bytes,
            &mut flags,
            overlapped.as_mut(),
            None,
        );
        match ret {
            winapi::SOCKET_ERROR => Err(last_error()),
            _ => Ok(bytes as usize),
        }
    }
}

impl Writev for TcpStream {
    unsafe fn writev(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize> {
        let iov_ptr = iovs.as_ptr() as *mut IoVec as winapi::LPWSABUF;
        let mut bytes = 0;
        let ret = ws2_32::WSASend(
            self.as_raw_socket(),
            iov_ptr,
            iovs.len() as winapi::DWORD,
            &mut bytes,
            0,
            overlapped.as_mut(),
            None,
        );
        match ret {
            winapi::SOCKET_ERROR => Err(last_error()),
            _ => Ok(bytes as usize),
        }
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

    unsafe fn readv<R>(
        &mut self,
        r: &mut R,
        overlapped: &mut Overlapped,
    ) -> io::Result<Option<(ByteBuf, usize)>>
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
        let n = r.readv(&iovs, overlapped)?;
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

thread_local!(static BUFFER: UnsafeCell<Buffer> = UnsafeCell::new(Buffer::new()));

#[inline]
unsafe fn readv<R>(r: &mut R, overlapped: &mut Overlapped) -> io::Result<Option<(ByteBuf, usize)>>
where
    R: Readv,
{
    BUFFER.with(|buf| (*buf.get()).readv(r, overlapped))
}

struct IStream<T, B = Nio<TcpStream, T>> {
    nio: B,
    overlapped: Box<Overlapped>,
    pending: bool,
    _marker: PhantomData<T>,
}

impl<T, B> IStream<T, B> {
    #[inline]
    pub(super) fn from(b: B) -> Self {
        IStream {
            nio: b,
            overlapped: Box::new(Overlapped::for_read()),
            pending: false,
            _marker: PhantomData,
        }
    }
}

impl<T, B> IStream<T, B>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
    B: BorrowMut<Nio<TcpStream, T>>,
{
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
        if self.pending {
            match nio.is_read_ready() {
                true => {
                    self.pending = false;
                    get_overlapped_result(
                        nio.get_ref().as_ref().as_raw_socket(),
                        &mut self.overlapped,
                    )?;
                }
                false => return Ok(Async::NotReady),
            }
        } else {
            if let Err(e) = unsafe {
                nio.get_mut()
                    .as_mut()
                    .readv(IoVec::empty(), &mut self.overlapped)
            } {
                return match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        nio.schedule_read();
                        self.pending = true;
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                };
            }
        }
        match unsafe { readv(nio.get_mut().as_mut(), &mut self.overlapped)? } {
            Some((data, _)) => Ok(Async::Ready(Some(data))),
            None => Ok(Async::Ready(None)),
        }
    }
}

#[inline]
fn get_iovs(chain: &mut GetIter) -> Result<Vec<IoVec>, Error> {
    let mut iovecs = Vec::new();
    for block in chain {
        let off = block.read_pos() as isize;
        let ptr = unsafe { block.as_ptr().offset(off) };
        iovecs.push(IoVec::from((ptr, block.len())));
    }
    Ok(iovecs)
}

struct OStream<T, B = Nio<TcpStream, T>> {
    nio: B,
    overlapped: Box<Overlapped>,
    pending: bool,
    _marker: PhantomData<T>,
}

impl<T, B> OStream<T, B> {
    #[inline]
    pub(super) fn from(b: B) -> Self {
        OStream {
            nio: b,
            overlapped: Box::new(Overlapped::for_write()),
            pending: false,
            _marker: PhantomData,
        }
    }
}

impl<T, B> OStream<T, B>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
    B: BorrowMut<Nio<TcpStream, T>>,
{
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
        if nio.is_write_ready() {
            if self.pending {
                self.pending = false;
                let socket = nio.get_ref().as_ref().as_raw_socket();
                let n = get_overlapped_result(socket, &mut self.overlapped)?;
                data.skip(n);
                data.compact();
                if data.is_empty() {
                    return Ok(Async::Ready(()));
                }
            }
            if let Err(e) = Self::writev(nio.get_mut().as_mut(), data, &mut self.overlapped) {
                return match e.kind() {
                    io::ErrorKind::WouldBlock => {
                        self.pending = true;
                        nio.schedule_write();
                        Ok(Async::NotReady)
                    }
                    _ => Err(e),
                };
            }
            data.compact();
            if data.is_empty() {
                return Ok(Async::Ready(()));
            }
        }
        Ok(Async::NotReady)
    }

    #[inline]
    fn writev(
        w: &mut TcpStream,
        data: &mut ByteBuf,
        overlapped: &mut Overlapped,
    ) -> io::Result<()> {
        let iovs = data.get(0, get_iovs).unwrap();
        let n = unsafe { w.writev(iovs.as_slice(), overlapped)? };
        data.skip(n);
        Ok(())
    }
}

pub struct Recv<T>(IStream<T>);

impl<T> Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        Ok(Recv(IStream::from(Nio::try_from(io)?)))
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    #[inline]
    pub fn poll_recv(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        self.0.poll_recv()
    }

    #[inline]
    pub fn into_twoway(self) -> (RecvHalf<T>, SendHalf<T>) {
        let nio_r = Rc::new(UnsafeCell::new(self.0.nio));
        let nio_s = nio_r.clone();
        (
            RecvHalf(IStream {
                nio: nio_r,
                overlapped: self.0.overlapped,
                pending: self.0.pending,
                _marker: PhantomData,
            }),
            SendHalf(OStream::from(nio_s)),
        )
    }
}

pub struct Sender<T>(OStream<T>);

impl<T> Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        Ok(Sender(OStream::from(Nio::try_from(io)?)))
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    #[inline]
    pub fn poll_send(&mut self, data: &mut ByteBuf) -> Poll<(), io::Error> {
        self.0.poll_send(data)
    }

    #[inline]
    pub fn into_twoway(self) -> (RecvHalf<T>, SendHalf<T>) {
        let nio_s = Rc::new(UnsafeCell::new(self.0.nio));
        let nio_r = nio_s.clone();
        (
            RecvHalf(IStream::from(nio_r)),
            SendHalf(OStream {
                nio: nio_s,
                overlapped: self.0.overlapped,
                pending: self.0.pending,
                _marker: PhantomData,
            }),
        )
    }
}

pub struct RecvHalf<T>(IStream<T, Rc<UnsafeCell<Nio<TcpStream, T>>>>);

impl<T> RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    #[inline]
    pub fn poll_recv(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        self.0.poll_recv()
    }
}

pub struct SendHalf<T>(OStream<T, Rc<UnsafeCell<Nio<TcpStream, T>>>>);

impl<T> SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn get_ref(&self) -> &T {
        self.0.get_ref()
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        self.0.get_mut()
    }

    #[inline]
    pub fn poll_send(&mut self, data: &mut ByteBuf) -> Poll<(), io::Error> {
        self.0.poll_send(data)
    }
}

#[inline]
pub fn split<T>(io: T) -> io::Result<(RecvHalf<T>, SendHalf<T>)>
where
    T: AsRef<TcpStream>,
{
    let nio_r = Rc::new(UnsafeCell::new(Nio::try_from(io)?));
    let nio_s = nio_r.clone();
    Ok((
        RecvHalf(IStream::from(nio_r)),
        SendHalf(OStream::from(nio_s)),
    ))
}

type ConnectEx = unsafe extern "system" fn(
    winapi::SOCKET,          // _In_    s,
    *const winapi::SOCKADDR, // _in_    name,
    winapi::c_int,           // _In_    namelen,
    winapi::PVOID,           // _In_opt lpSendBuffer,
    winapi::DWORD,           // _In_    dwSendDataLength,
    winapi::LPDWORD,         // _Out_   lpdwBytesSent,
    winapi::LPOVERLAPPED,    // _In_    lpOverlapped,
) -> winapi::BOOL;

#[derive(Debug)]
enum ConnectState<T> {
    Connecting(Nio<TcpStream, T>, Box<Overlapped>),
    Finishing(Nio<TcpStream, T>, Box<Overlapped>),
    Connected(Nio<TcpStream, T>),
    Error(io::Error),
    Done,
}

#[derive(Debug)]
pub struct Connect<T> {
    state: ConnectState<T>,
}

impl<T> Connect<T>
where
    T: AsRef<TcpStream> + From<TcpStream>,
{
    #[inline]
    pub fn from(addr: &SocketAddr) -> Self {
        let state = match Self::connect(addr) {
            Ok(s) => s,
            Err(e) => ConnectState::Error(e),
        };
        Connect { state }
    }

    #[inline]
    fn inaddr_any() -> SocketAddr {
        SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(0, 0, 0, 0), 0))
    }

    #[inline]
    fn inaddr_any6() -> SocketAddr {
        SocketAddr::V6(SocketAddrV6::new(
            Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0),
            0,
            0,
            0,
        ))
    }

    #[inline]
    fn connect(addr: &SocketAddr) -> io::Result<ConnectState<T>> {
        let (builder, inaddr_any, name, len) = match *addr {
            SocketAddr::V4(ref a) => (
                TcpBuilder::new_v4()?,
                Self::inaddr_any(),
                a as *const _ as *const _,
                mem::size_of_val(a),
            ),
            SocketAddr::V6(ref a) => (
                TcpBuilder::new_v6()?,
                Self::inaddr_any6(),
                a as *const _ as *const _,
                mem::size_of_val(a),
            ),
        };
        let stream = TcpStream::from(builder.bind(inaddr_any)?.to_tcp_stream()?);
        stream.as_inner().set_nonblocking(true)?;
        let nio = Nio::try_from(T::from(stream))?;

        let connect_ex = CONNECTEX.get(nio.get_ref().as_ref().as_raw_socket())?;
        let mut overlapped = Box::new(Overlapped::for_write());
        let success = unsafe {
            (mem::transmute::<_, ConnectEx>(connect_ex))(
                nio.get_ref().as_ref().as_raw_socket(),
                name,
                len as winapi::c_int,
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                overlapped.as_mut().as_mut(),
            )
        };
        use self::ConnectState::*;
        match success == winapi::TRUE {
            false => {
                let e = last_error();
                match e.kind() {
                    io::ErrorKind::WouldBlock => Ok(Connecting(nio, overlapped)),
                    _ => Err(e),
                }
            }
            true => Ok(Connected(nio)),
        }
    }
}

impl<T> Connect<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn poll_connect(&mut self) -> Poll<Sender<T>, io::Error> {
        use self::ConnectState::*;
        match mem::replace(&mut self.state, Done) {
            Connecting(nio, overlapped) => {
                nio.schedule_write();
                self.state = Finishing(nio, overlapped);
                Ok(Async::NotReady)
            }
            Finishing(nio, mut overlapped) => if nio.is_write_ready() {
                get_overlapped_result(nio.get_ref().as_ref().as_raw_socket(), &mut overlapped)?;
                Self::finish_connect(nio.get_ref().as_ref())?;
                Ok(Async::Ready(Sender(OStream::from(nio))))
            } else {
                self.state = Finishing(nio, overlapped);
                Ok(Async::NotReady)
            },
            Connected(nio) => Ok(Async::Ready(Sender(OStream::from(nio)))),
            Error(e) => Err(e),
            Done => panic!("Attempted to poll Connect after completion"),
        }
    }

    #[inline]
    fn finish_connect(stream: &TcpStream) -> io::Result<()> {
        const SO_UPDATE_CONNECT_CONTEXT: winapi::c_int = 0x7010;
        let result = unsafe {
            ws2_32::setsockopt(
                stream.as_raw_socket(),
                winapi::SOL_SOCKET,
                SO_UPDATE_CONNECT_CONTEXT,
                ptr::null(),
                0,
            )
        };
        match result == 0 {
            true => Ok(()),
            false => Err(last_error()),
        }
    }
}
