use std::fmt;
use std::io;
use std::net::{self, IpAddr, Ipv4Addr, Shutdown, SocketAddr};
use std::time::Duration;

use net2::{TcpBuilder, TcpStreamExt};
use futures::{Async, AsyncSink, Future, Poll, Sink, StartSend, Stream};
use futures::stream::StreamFuture;
use futures::sink::{Send, SendAll};

use buf::ByteBuf;
use sys::net::tcp;

////////////////////////////////////////////////////////////////////////////////
// TcpListener

pub struct Incoming {
    inner: tcp::Incoming,
}

#[derive(Debug, Clone, Copy)]
pub struct TcpListenerBuilder {
    addr: SocketAddr,
    backlog: i32,
    ttl: Option<u32>,
    only_v6: Option<bool>,
}

pub struct TcpListener {
    inner: net::TcpListener,
}

impl TcpListener {
    #[inline]
    pub fn builder() -> TcpListenerBuilder {
        Default::default()
    }

    #[inline]
    pub fn incoming(self) -> io::Result<Incoming> {
        Ok(Incoming {
            inner: tcp::Incoming::try_from(self)?,
        })
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
    pub(crate) fn as_inner(&self) -> &net::TcpListener {
        &self.inner
    }

    #[inline]
    fn from(inner: net::TcpListener) -> Self {
        TcpListener { inner }
    }
}

impl AsRef<Self> for TcpListener {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl fmt::Debug for TcpListener {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl Default for TcpListenerBuilder {
    #[inline]
    fn default() -> Self {
        TcpListenerBuilder {
            addr: SocketAddr::new(IpAddr::from(Ipv4Addr::from(0)), 0),
            backlog: 128,
            ttl: None,
            only_v6: None,
        }
    }
}

impl TcpListenerBuilder {
    #[inline]
    pub fn addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.addr = addr;
        self
    }

    #[inline]
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.addr.set_port(port);
        self
    }

    #[inline]
    pub fn backlog(&mut self, backlog: i32) -> &mut Self {
        self.backlog = backlog;
        self
    }

    #[inline]
    pub fn ttl(&mut self, ttl: Option<u32>) -> &mut Self {
        self.ttl = ttl;
        self
    }

    #[inline]
    pub fn only_v6(&mut self, only_v6: Option<bool>) -> &mut Self {
        self.only_v6 = only_v6;
        self
    }

    pub fn build(&self) -> io::Result<TcpListener> {
        let builder = match self.addr {
            SocketAddr::V4(..) => TcpBuilder::new_v4()?,
            SocketAddr::V6(..) => TcpBuilder::new_v6()?,
        };
        if let Some(ttl) = self.ttl {
            builder.ttl(ttl)?;
        }
        if let Some(only_v6) = self.only_v6 {
            builder.only_v6(only_v6)?;
        }
        let listener = builder
            .reuse_address(true)?
            .bind(self.addr)?
            .listen(self.backlog)?;
        listener.set_nonblocking(true)?;
        Ok(TcpListener::from(listener))
    }
}

impl Stream for Incoming {
    type Item = (TcpStream, SocketAddr);
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll_accept()
    }
}

///////////////////////////////////////////////////////////////////////////////
// TcpStream

#[derive(Debug)]
pub struct TcpStream {
    inner: net::TcpStream,
}

impl TcpStream {
    /// Returns the local address.
    #[inline]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.as_inner().local_addr()
    }

    /// Returns the remote address.
    #[inline]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.as_inner().peer_addr()
    }

    #[inline]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.as_inner().shutdown(how)
    }

    #[inline]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.as_inner().set_nodelay(nodelay)
    }

    #[inline]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.as_inner().nodelay()
    }

    #[inline]
    pub fn set_recv_buffer_size(&self, size: usize) -> io::Result<()> {
        self.as_inner().set_recv_buffer_size(size)
    }

    #[inline]
    pub fn recv_buffer_size(&self) -> io::Result<usize> {
        self.as_inner().recv_buffer_size()
    }

    #[inline]
    pub fn set_send_buffer_size(&self, size: usize) -> io::Result<()> {
        self.as_inner().set_send_buffer_size(size)
    }

    #[inline]
    pub fn send_buffer_size(&self) -> io::Result<usize> {
        self.as_inner().send_buffer_size()
    }

    #[inline]
    pub fn set_keepalive(&self, keepalive: Option<Duration>) -> io::Result<()> {
        self.as_inner().set_keepalive(keepalive)
    }

    #[inline]
    pub fn keepalive(&self) -> io::Result<Option<Duration>> {
        self.as_inner().keepalive()
    }

    #[inline]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.as_inner().set_ttl(ttl)
    }

    #[inline]
    pub fn ttl(&self) -> io::Result<u32> {
        self.as_inner().ttl()
    }

    #[inline]
    pub fn set_linger(&self, dur: Option<Duration>) -> io::Result<()> {
        self.as_inner().set_linger(dur)
    }

    #[inline]
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.as_inner().linger()
    }

    #[inline]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.as_inner().set_only_v6(only_v6)
    }

    #[inline]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.as_inner().only_v6()
    }

    #[inline]
    pub(crate) fn as_inner(&self) -> &net::TcpStream {
        &self.inner
    }

    #[inline]
    pub(crate) fn from(inner: net::TcpStream) -> Self {
        TcpStream { inner }
    }
}

impl AsRef<Self> for TcpStream {
    #[inline]
    fn as_ref(&self) -> &Self {
        self
    }
}

impl AsMut<Self> for TcpStream {
    #[inline]
    fn as_mut(&mut self) -> &mut Self {
        self
    }
}

impl fmt::Display for TcpStream {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

pub struct Connect<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: tcp::Connect<T>,
}

impl<T> Future for Connect<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    type Item = Sender<T>;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let inner = try_ready!(self.inner.poll_connect());
        Ok(Async::Ready(Sender {
            inner,
            buf: ByteBuf::new(),
        }))
    }
}

pub struct Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: tcp::Recv<T>,
}

impl<T> AsRef<T> for Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner.get_ref()
    }
}

impl<T> AsMut<T> for Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Stream for Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll_recv()
    }
}

impl<T> Recv<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn next(self) -> StreamFuture<Self> {
        self.into_future()
    }

    #[inline]
    pub fn into_2way(self) -> (RecvHalf<T>, SendHalf<T>) {
        let (r, s) = self.inner.into_2way();
        (
            RecvHalf { inner: r },
            SendHalf {
                inner: s,
                buf: ByteBuf::new(),
            },
        )
    }
}

pub struct Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: tcp::Sender<T>,
    buf: ByteBuf,
}

impl<T> AsRef<T> for Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner.get_ref()
    }
}

impl<T> AsMut<T> for Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn try_from(io: T) -> io::Result<Self> {
        Ok(Sender {
            inner: tcp::Sender::try_from(io)?,
            buf: ByteBuf::new(),
        })
    }

    #[inline]
    pub fn into_2way(self) -> (RecvHalf<T>, SendHalf<T>) {
        let (r, s) = self.inner.into_2way();
        (
            RecvHalf { inner: r },
            SendHalf {
                inner: s,
                buf: self.buf,
            },
        )
    }
}

impl<T> Sink for Sender<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    type SinkItem = ByteBuf;
    type SinkError = io::Error;

    fn start_send(&mut self, data: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.buf.is_empty() {
            true => self.buf = data,
            false => self.buf.extend(data),
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_send(&mut self.buf)
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.inner.poll_send(&mut self.buf));
        self.as_ref().as_ref().shutdown(Shutdown::Write)?;
        Ok(Async::Ready(()))
    }
}

pub struct RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: tcp::RecvHalf<T>,
}

impl<T> AsRef<T> for RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner.get_ref()
    }
}

impl<T> AsMut<T> for RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Stream for RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.inner.poll_recv()
    }
}

impl<T> RecvHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    pub fn next(self) -> StreamFuture<Self> {
        self.into_future()
    }
}

pub struct SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    inner: tcp::SendHalf<T>,
    buf: ByteBuf,
}

impl<T> AsRef<T> for SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_ref(&self) -> &T {
        self.inner.get_ref()
    }
}

impl<T> AsMut<T> for SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut T {
        self.inner.get_mut()
    }
}

impl<T> Sink for SendHalf<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    type SinkItem = ByteBuf;
    type SinkError = io::Error;

    fn start_send(&mut self, data: Self::SinkItem) -> StartSend<Self::SinkItem, Self::SinkError> {
        match self.buf.is_empty() {
            true => self.buf = data,
            false => self.buf.extend(data),
        }
        Ok(AsyncSink::Ready)
    }

    fn poll_complete(&mut self) -> Poll<(), Self::SinkError> {
        self.inner.poll_send(&mut self.buf)
    }

    fn close(&mut self) -> Poll<(), Self::SinkError> {
        try_ready!(self.inner.poll_send(&mut self.buf));
        self.as_ref().as_ref().shutdown(Shutdown::Write)?;
        Ok(Async::Ready(()))
    }
}

#[inline]
pub fn connect<T>(addr: &SocketAddr) -> Connect<T>
where
    T: AsRef<TcpStream> + AsMut<TcpStream> + From<TcpStream>,
{
    Connect {
        inner: tcp::Connect::from(addr),
    }
}

#[inline]
pub fn recv<T>(io: T) -> io::Result<Recv<T>>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    Ok(Recv {
        inner: tcp::Recv::try_from(io)?,
    })
}

#[inline]
pub fn send<T>(io: T, data: ByteBuf) -> io::Result<Send<Sender<T>>>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    Ok(Sender::try_from(io)?.send(data))
}

#[inline]
pub fn send_all<T, S>(io: T, s: S) -> io::Result<SendAll<Sender<T>, S>>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
    S: Stream<Item = ByteBuf>,
    io::Error: From<S::Error>,
{
    Ok(Sender::try_from(io)?.send_all(s))
}

#[inline]
pub fn split<T>(io: T) -> io::Result<(RecvHalf<T>, SendHalf<T>)>
where
    T: AsRef<TcpStream> + AsMut<TcpStream>,
{
    let (r, s) = tcp::split(io)?;
    Ok((
        RecvHalf { inner: r },
        SendHalf {
            inner: s,
            buf: ByteBuf::new(),
        },
    ))
}
