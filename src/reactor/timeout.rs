use std::error::Error;
use std::fmt;
use std::io;

use futures::{Async, Future, Poll, Stream};

use future;
use stream;
use reactor::wheel::Timer;

pub struct TimeoutError<T> {
    inner: T,
}

impl<T> TimeoutError<T> {
    #[inline]
    fn new(inner: T) -> Self {
        TimeoutError { inner }
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    #[inline]
    pub fn into_inner(self) -> T {
        self.inner
    }
}

impl<T> fmt::Display for TimeoutError<T> {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl<T> fmt::Debug for TimeoutError<T> {
    #[inline]
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl<T> Error for TimeoutError<T> {
    #[inline]
    fn description(&self) -> &str {
        "timed out"
    }
}

impl<T> From<TimeoutError<T>> for io::Error {
    #[inline]
    fn from(_: TimeoutError<T>) -> Self {
        io::Error::from(io::ErrorKind::TimedOut)
    }
}

impl<T> From<TimeoutError<T>> for () {
    fn from(_: TimeoutError<T>) -> Self {}
}

pub struct TimeoutFuture<F> {
    future: Option<F>,
    timer: Timer,
}

impl<F> TimeoutFuture<F> {
    #[inline]
    pub fn new(future: F, secs: u64) -> Self {
        TimeoutFuture {
            future: Some(future),
            timer: Timer::new(secs),
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &F {
        self.future
            .as_ref()
            .expect("Attempted TimeoutFuture::get_ref after completion")
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut F {
        self.future
            .as_mut()
            .expect("Attempted TimeoutFuture::get_mut after completion")
    }

    #[inline]
    pub fn into_inner(self) -> F {
        self.future
            .expect("Attempted TimeoutFuture::into_inner after completion")
    }

    #[inline]
    fn future_mut(&mut self) -> &mut F {
        self.future
            .as_mut()
            .expect("Attempted to poll TimeoutFuture after completion")
    }
}

impl<F, E> Future for TimeoutFuture<F>
where
    F: Future<Error = E>,
    E: From<TimeoutError<F>>,
{
    type Item = F::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.future_mut().poll() {
            Ok(Async::NotReady) => match self.timer.poll() {
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Ok(Async::Ready(())) => match self.future.take() {
                    Some(f) => Err(TimeoutError::new(f).into()),
                    None => ::unreachable(),
                },
                _ => ::unreachable(),
            },
            ready => ready,
        }
    }
}

pub struct TimeoutStream<S> {
    stream: Option<S>,
    secs: u64,
    timer: Timer,
}

impl<S> TimeoutStream<S> {
    #[inline]
    pub fn new(stream: S, secs: u64) -> Self {
        TimeoutStream {
            stream: Some(stream),
            secs,
            timer: Timer::new(secs),
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &S {
        self.stream
            .as_ref()
            .expect("Attempted TimeoutStream::get_ref after completion")
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut S {
        self.stream
            .as_mut()
            .expect("Attempted TimeoutStream::get_mut after completion")
    }

    #[inline]
    pub fn into_inner(self) -> S {
        self.stream
            .expect("Attempted TimeoutStream::into_inner after completion")
    }

    #[inline]
    pub fn stream_mut(&mut self) -> &mut S {
        self.stream
            .as_mut()
            .expect("Attempted to poll TimeoutStream after completion")
    }
}

impl<S, E> Stream for TimeoutStream<S>
where
    S: Stream<Error = E>,
    E: From<TimeoutError<S>>,
{
    type Item = S::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.stream_mut().poll() {
            Ok(Async::NotReady) => match self.timer.poll() {
                Ok(Async::NotReady) => Ok(Async::NotReady),
                Ok(Async::Ready(())) => match self.stream.take() {
                    Some(s) => Err(TimeoutError::new(s).into()),
                    None => ::unreachable(),
                },
                _ => ::unreachable(),
            },
            Ok(Async::Ready(Some(v))) => match self.timer.reschedule(self.secs) {
                true => Ok(Async::Ready(Some(v))),
                false => match self.stream.take() {
                    Some(s) => Err(TimeoutError::new(s).into()),
                    None => ::unreachable(),
                },
            },
            ready_none => ready_none,
        }
    }
}

impl<F> future::Timeout for F
where
    F: Future,
    F::Error: From<TimeoutError<F>>,
{
    type Future = TimeoutFuture<F>;

    fn timeout(self, secs: u64) -> Self::Future {
        TimeoutFuture::new(self, secs)
    }
}

impl<S> stream::Timeout for S
where
    S: Stream,
    S::Error: From<TimeoutError<S>>,
{
    type Stream = TimeoutStream<S>;

    fn timeout(self, secs: u64) -> Self::Stream {
        TimeoutStream::new(self, secs)
    }
}
