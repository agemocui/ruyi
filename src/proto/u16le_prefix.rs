use std::io;

use futures::{Async, Poll, Stream};

use buf::ByteBuf;
use buf::codec::u16::little_endian as u16le;

const U16_SIZE: usize = 2;

enum State {
    More(usize),
    Pending,
    Done,
}

pub struct U16lePrefix<S> {
    stream: S,
    state: State,
    data: ByteBuf,
}

impl<S> U16lePrefix<S> {
    #[inline]
    fn new(stream: S) -> Self {
        U16lePrefix {
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

impl<S> U16lePrefix<S>
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
                let len = self.data.len();
                if len < U16_SIZE {
                    return Ok(Async::NotReady);
                }
                match self.data.read(u16le::read) {
                    Ok(n) => match len >= n as usize {
                        true => match self.data.drain_to(n as usize) {
                            Ok(t) => Ok(Async::Ready(Some(t))),
                            _ => ::unreachable(),
                        },
                        false => {
                            self.state = State::More(n as usize);
                            Ok(Async::NotReady)
                        }
                    },
                    _ => ::unreachable(),
                }
            }
            None => {
                self.state = State::Done;
                Ok(Async::Ready(None))
            }
        }
    }
}

impl<S> Stream for U16lePrefix<S>
where
    S: Stream<Item = ByteBuf, Error = io::Error>,
{
    type Item = ByteBuf;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.state {
            State::Pending => {
                let len = self.data.len();
                if len < U16_SIZE {
                    return self.poll_pending();
                }
                match self.data.read(u16le::read) {
                    Ok(n) => match len >= n as usize {
                        true => match self.data.drain_to(n as usize) {
                            Ok(t) => Ok(Async::Ready(Some(t))),
                            _ => ::unreachable(),
                        },
                        false => {
                            self.state = State::More(n as usize);
                            self.poll_more(n as usize)
                        }
                    },
                    _ => ::unreachable(),
                }
            }
            State::More(n) => self.poll_more(n),
            State::Done => Ok(Async::Ready(None)),
        }
    }
}

impl<S> From<S> for U16lePrefix<S>
where
    S: Stream<Item = ByteBuf, Error = io::Error>,
{
    #[inline]
    fn from(stream: S) -> Self {
        Self::new(stream)
    }
}
