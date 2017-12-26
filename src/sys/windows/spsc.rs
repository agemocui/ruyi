use std::io;
use std::sync::Arc;

use futures::{Async, Poll};
use sync::spsc::{ReceiverAwakener, RingBuffer};

pub(crate) struct Recv<T> {
    awakener: ReceiverAwakener,
    buffer: Arc<RingBuffer<T>>,
    need_reset: bool,
}

impl<T> Recv<T> {
    #[inline]
    pub(crate) fn try_new(
        buffer: Arc<RingBuffer<T>>,
        awakener: ReceiverAwakener,
    ) -> io::Result<Self> {
        awakener.as_inner().register();
        Ok(Recv {
            awakener,
            buffer,
            need_reset: true,
        })
    }

    #[inline]
    pub(crate) fn poll(&mut self) -> Poll<Option<T>, io::Error> {
        if self.need_reset {
            self.awakener.reset()?;
            self.need_reset = false;
        }
        if let Some(t) = self.buffer.try_pop() {
            Ok(Async::Ready(Some(t)))
        } else if Arc::strong_count(&self.buffer) == 1 {
            self.need_reset = true;
            Ok(Async::Ready(None))
        } else {
            self.need_reset = true;
            Ok(Async::NotReady)
        }
    }
}
