use std::io;

use futures::{Async, Poll, Stream};

use buf::ByteBuf;
use buf::codec::u8;

enum State {
    More(usize),
    Pending,
    Done,
}

pub struct U8Prefix<S> {
    stream: S,
    state: State,
    data: ByteBuf,
}

impl<S> U8Prefix<S> {
    #[inline]
    fn new(stream: S) -> Self {
        U8Prefix {
            stream,
            state: State::Pending,
            data: ByteBuf::new(),
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

impl<S> U8Prefix<S>
where
    S: Stream<Item = ByteBuf, Error = io::Error>,
{
    #[inline]
    fn poll_more(&mut self, n: usize) -> Poll<Option<ByteBuf>, io::Error> {
        match try_ready!(self.stream.poll()) {
            Some(pulled) => {
                self.data.extend(pulled);
                match self.data.drain_to(n) {
                    Ok(t) => {
                        self.state = State::Pending;
                        Ok(Async::Ready(Some(t)))
                    }
                    Err(..) => Ok(Async::NotReady),
                }
            }
            None => {
                self.state = State::Done;
                Ok(Async::Ready(None))
            }
        }
    }

    #[inline]
    fn poll_pending(&mut self) -> Poll<Option<ByteBuf>, io::Error> {
        match try_ready!(self.stream.poll()) {
            Some(pulled) => {
                self.data = pulled;
                match self.data.read(u8::read) {
                    Ok(n) => {
                        match self.data.drain_to(n as usize) {
                            Ok(t) => Ok(Async::Ready(Some(t))),
                            Err(..) => {
                                // data.len() < n
                                self.state = State::More(n as usize);
                                Ok(Async::NotReady)
                            }
                        }
                    }
                    Err(..) => Ok(Async::NotReady),
                }
            }
            None => {
                self.state = State::Done;
                Ok(Async::Ready(None))
            }
        }
    }
}

impl<S> Stream for U8Prefix<S>
where
    S: Stream<Item = ByteBuf, Error = io::Error>,
{
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.state {
            State::Pending => {
                match self.data.read(u8::read) {
                    Ok(n) => {
                        match self.data.drain_to(n as usize) {
                            Ok(t) => Ok(Async::Ready(Some(t))),
                            Err(..) => {
                                // data.len() < n
                                self.state = State::More(n as usize);
                                self.poll_more(n as usize)
                            }
                        }
                    }
                    Err(..) => self.poll_pending(),
                }
            }
            State::More(n) => self.poll_more(n),
            State::Done => Ok(Async::Ready(None)),
        }
    }
}

impl<S> From<S> for U8Prefix<S>
where
    S: Stream<Item = ByteBuf, Error = io::Error>,
{
    #[inline]
    fn from(stream: S) -> Self {
        Self::new(stream)
    }
}
