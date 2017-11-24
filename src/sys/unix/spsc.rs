use std::fmt;
use std::io;
use std::sync::Arc;
use std::os::unix::io::{AsRawFd, RawFd};

use futures::{Async, Poll};
use channel::spsc::{ReceiverAwakener, RingBuffer};
use sys::unix::awakener::Awakener;
use sys::unix::nio::Nio;

pub(crate) struct Recv<T> {
    nio: Nio<Awakener, ReceiverAwakener>,
    buffer: Arc<RingBuffer<T>>,
    need_reset: bool,
}

impl<T> Recv<T> {
    #[inline]
    pub(crate) fn try_new(
        buffer: Arc<RingBuffer<T>>,
        awakener: ReceiverAwakener,
    ) -> io::Result<Self> {
        Ok(Recv {
            nio: Nio::try_from(awakener)?,
            buffer,
            need_reset: true,
        })
    }

    #[inline]
    pub(crate) fn poll(&mut self) -> Poll<Option<T>, io::Error> {
        if self.need_reset {
            if let Err(e) = self.nio.get_ref().reset() {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.nio.schedule_read()?;
                    return Ok(Async::NotReady);
                }
                return Err(e);
            }
            self.need_reset = false;
        }
        if let Some(t) = self.buffer.try_pop() {
            Ok(Async::Ready(Some(t)))
        } else if Arc::strong_count(&self.buffer) == 1 {
            self.need_reset = true;
            Ok(Async::Ready(None))
        } else {
            self.need_reset = true;
            self.nio.schedule_read()?;
            Ok(Async::NotReady)
        }
    }
}

impl AsRawFd for ReceiverAwakener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

impl fmt::Debug for ReceiverAwakener {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_inner().fmt(f)
    }
}

impl AsRef<Awakener> for ReceiverAwakener {
    #[inline]
    fn as_ref(&self) -> &Awakener {
        self.as_inner()
    }
}
