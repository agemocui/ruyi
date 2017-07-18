use std::error::Error;
use std::fmt;
use std::io;

use futures::{Future, Poll, Async, Stream};

use future;
use stream;
use reactor::wheel::Timer;

pub struct TimeoutError;

impl fmt::Display for TimeoutError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl fmt::Debug for TimeoutError {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.description())
    }
}

impl Error for TimeoutError {
    fn description(&self) -> &str {
        "timed out"
    }
}

impl From<TimeoutError> for io::Error {
    fn from(_: TimeoutError) -> Self {
        io::Error::from(io::ErrorKind::TimedOut)
    }
}

impl From<TimeoutError> for () {
    fn from(_: TimeoutError) -> Self {}
}

pub struct TimeoutFuture<F> {
    timer: Timer,
    future: F,
    timed_out: bool,
}

impl<F> TimeoutFuture<F> {
    #[inline]
    pub fn new(future: F, secs: u64) -> Self {
        TimeoutFuture {
            timer: Timer::new(secs),
            future,
            timed_out: false,
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &F {
        &self.future
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut F {
        &mut self.future
    }

    #[inline]
    pub fn into_inner(self) -> F {
        self.future
    }
}

impl<F, E> Future for TimeoutFuture<F>
where
    F: Future<Error = E>,
    E: From<TimeoutError>,
{
    type Item = F::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        assert!(
            !self.timed_out,
            "This TimeoutFuture has been consumed already"
        );

        match self.future.poll() {
            Ok(Async::NotReady) => {}
            ready => return ready,
        }

        match self.timer.poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(())) => {
                self.timed_out = true;
                Err(TimeoutError.into())
            }
            _ => ::unreachable(),
        }
    }
}

pub struct TimeoutStream<S> {
    timer: Timer,
    secs: u64,
    stream: S,
    timed_out: bool,
}

impl<S> TimeoutStream<S> {
    #[inline]
    pub fn new(stream: S, secs: u64) -> Self {
        TimeoutStream {
            timer: Timer::new(secs),
            secs,
            stream,
            timed_out: false,
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &S {
        &self.stream
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut S {
        &mut self.stream
    }

    #[inline]
    pub fn into_inner(self) -> S {
        self.stream
    }
}

impl<S, E> Stream for TimeoutStream<S>
where
    S: Stream<Error = E>,
    E: From<TimeoutError>,
{
    type Item = S::Item;
    type Error = E;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        assert!(
            !self.timed_out,
            "This TimeoutStream has been consumed already"
        );

        match self.stream.poll() {
            Ok(Async::NotReady) => {}
            Ok(Async::Ready(Some(v))) => {
                return match self.timer.reschedule(self.secs) {
                    true => Ok(Async::Ready(Some(v))),
                    false => {
                        self.timed_out = true;
                        Err(TimeoutError.into())
                    }
                };
            }
            ready_none => return ready_none,
        }

        match self.timer.poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(())) => {
                self.timed_out = true;
                Err(TimeoutError.into())
            }
            _ => ::unreachable(),
        }
    }
}

impl<F, E> future::Timeout for F
where
    F: Future<Error = E>,
    E: From<TimeoutError>,
{
    type Future = TimeoutFuture<F>;

    fn timeout(self, secs: u64) -> Self::Future {
        TimeoutFuture::new(self, secs)
    }
}

impl<S, E> stream::Timeout for S
where
    S: Stream<Error = E>,
    E: From<TimeoutError>,
{
    type Stream = TimeoutStream<S>;

    fn timeout(self, secs: u64) -> Self::Stream {
        TimeoutStream::new(self, secs)
    }
}
