use std::cell::Cell;
use std::fmt;
use std::mem;
use std::io;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::stream::Stream;
use futures::{Async, Poll};

use super::err::{RecvError, SendError, TryRecvError, TrySendError};
use nio::{Awakener, Ops, Pollable, Poller, Token};
use stream::IntoStream;
use reactor::PollableIo;

struct Inner<T> {
    buf_ptr: *mut T,
    alloc_cap: usize,
    cap: usize,
    idx_mask: usize,
    _padding0: [usize; cache_line_pad!(4)],

    front: AtomicUsize,
    shadow_rear: Cell<usize>,
    _padding1: [usize; cache_line_pad!(1)],

    shadow_front: Cell<usize>,
    rear: AtomicUsize,
}

unsafe impl<T: Send> Send for Inner<T> {}
unsafe impl<T> Sync for Inner<T> {}

impl<T> Inner<T> {
    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        let n = capacity.next_power_of_two();
        let mut buf = Vec::with_capacity(n);
        let buf_ptr = buf.as_mut_ptr();
        let alloc_cap = buf.capacity();
        mem::forget(buf);
        Inner {
            buf_ptr: buf_ptr,
            alloc_cap: alloc_cap,
            cap: capacity,
            idx_mask: n.wrapping_sub(1),
            _padding0: [0; cache_line_pad!(4)],

            front: AtomicUsize::new(0),
            shadow_rear: Cell::new(0),
            _padding1: [0; cache_line_pad!(1)],

            shadow_front: Cell::new(0),
            rear: AtomicUsize::new(0),
        }
    }

    #[inline]
    fn effective_index(&self, index: usize) -> usize {
        index & self.idx_mask
    }

    fn try_push(&self, t: T) -> Option<T> {
        let rear = self.rear.load(Ordering::Relaxed);
        if self.shadow_front.get() + self.cap <= rear {
            self.shadow_front.set(self.front.load(Ordering::Acquire));
            if self.shadow_front.get() + self.cap <= rear {
                return Some(t);
            }
        }
        let i = self.effective_index(rear) as isize;
        unsafe { ptr::write(self.buf_ptr.offset(i), t) };
        self.rear.store(rear.wrapping_add(1), Ordering::Release);
        None
    }

    fn try_pop(&self) -> Option<T> {
        let front = self.front.load(Ordering::Relaxed);
        if front == self.shadow_rear.get() {
            self.shadow_rear.set(self.rear.load(Ordering::Acquire));
            if front == self.shadow_rear.get() {
                return None;
            }
        }

        let i = self.effective_index(front) as isize;
        let v = unsafe { ptr::read(self.buf_ptr.offset(i)) };
        self.front.store(front.wrapping_add(1), Ordering::Release);
        Some(v)
    }
}

impl<T> fmt::Debug for Inner<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Channel {{ ")?;
        write!(
            f,
            "cap: {}, front: {:?}, shadow_rear: {}, shadow_front: {}, rear: {:?}",
            self.cap,
            self.front,
            self.shadow_rear.get(),
            self.shadow_front.get(),
            self.rear
        )?;
        write!(f, " }}")
    }
}

impl<T> Drop for Inner<T> {
    #[inline]
    fn drop(&mut self) {
        while let Some(t) = self.try_pop() {
            drop(t);
        }
        drop(unsafe {
            Vec::from_raw_parts(self.buf_ptr, 0, self.alloc_cap)
        });
    }
}

#[derive(Debug)]
struct SenderAwakener(Arc<Awakener>);

impl Drop for SenderAwakener {
    fn drop(&mut self) {
        if let Err(e) = (&self.0).wakeup() {
            if e.kind() != io::ErrorKind::WouldBlock {
                error!("Failed to wakeup, {:?}: {}", &self.0, e);
            }
        }
    }
}

#[derive(Debug)]
pub struct SyncSender<T> {
    inner: Arc<Inner<T>>,
    awakener: SenderAwakener,
}

impl<T> SyncSender<T> {
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        if Arc::strong_count(&self.inner) == 1 {
            return Err(TrySendError::Disconnected(t));
        }
        if let Some(t) = (&self.inner).try_push(t) {
            return Err(TrySendError::Full(t));
        }
        (&self.awakener.0).wakeup()?;
        Ok(())
    }

    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        let mut msg = t;
        loop {
            if Arc::strong_count(&self.inner) == 1 {
                return Err(SendError::Disconnected(msg));
            }
            match (&self.inner).try_push(msg) {
                Some(t) => msg = t,
                None => break,
            }
        }
        (&self.awakener.0).wakeup()?;
        Ok(())
    }
}

struct ReceiverAwakener(Arc<Awakener>);

impl Drop for ReceiverAwakener {
    fn drop(&mut self) {
        if let Err(e) = (&self.0).reset() {
            if e.kind() != io::ErrorKind::WouldBlock {
                error!("Failed to reset, {:?}: {}", &self.0, e);
            }
        }
    }
}

pub struct Receiver<T> {
    inner: Arc<Inner<T>>,
    awakener: ReceiverAwakener,
}

pub struct Receiving<T> {
    io: PollableIo<Receiver<T>>,
    need_reset: bool,
}

impl<T> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.reset()?;
        if let Some(t) = self.try_pop() {
            Ok(t)
        } else if Arc::strong_count(&self.inner) == 1 {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    pub fn recv(&self) -> Result<T, RecvError> {
        loop {
            if let Some(t) = self.try_pop() {
                (&self.awakener.0).reset()?;
                return Ok(t);
            }
            if Arc::strong_count(&self.inner) == 1 {
                return Err(RecvError::Disconnected);
            }
        }
    }

    #[inline]
    fn reset(&self) -> io::Result<()> {
        (&self.awakener.0).reset()
    }

    #[inline]
    fn try_pop(&self) -> Option<T> {
        (&self.inner).try_pop()
    }
}

impl<T> Pollable for Receiver<T> {
    #[inline]
    fn register(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()> {
        (&self.awakener.0).register(poller, interested_ops, token)
    }

    #[inline]
    fn reregister(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()> {
        (&self.awakener.0).reregister(poller, interested_ops, token)
    }

    #[inline]
    fn deregister(&self, poller: &Poller) -> io::Result<()> {
        (&self.awakener.0).deregister(poller)
    }
}

impl<T> IntoStream for Receiver<T> {
    type Stream = Receiving<T>;

    #[inline]
    fn into_stream(self) -> Self::Stream {
        Receiving {
            io: PollableIo::new(self),
            need_reset: true,
        }
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Receiver {{ inner: {:?}, awakener: {:?} }}",
            &self.inner,
            &self.awakener.0
        )
    }
}

impl<T> Stream for Receiving<T> {
    type Item = T;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        if self.need_reset {
            if let Err(e) = self.io.get_ref().reset() {
                if e.kind() == io::ErrorKind::WouldBlock {
                    self.io.need_read()?;
                    return Ok(Async::NotReady);
                }
                return Err(e);
            }
            self.need_reset = false;
        }
        if let Some(t) = self.io.get_ref().try_pop() {
            Ok(Async::Ready(Some(t)))
        } else if Arc::strong_count(&self.io.get_ref().inner) == 1 {
            self.need_reset = true;
            Ok(Async::Ready(None))
        } else {
            self.need_reset = true;
            self.io.need_read()?;
            Ok(Async::NotReady)
        }
    }
}

pub fn sync_channel<T>(capacity: usize) -> io::Result<(SyncSender<T>, Receiver<T>)> {
    let inner = Arc::new(Inner::with_capacity(capacity));
    let awakener = Arc::new(Awakener::new()?);
    let tx = SyncSender {
        inner: inner.clone(),
        awakener: SenderAwakener(awakener.clone()),
    };
    let rx = Receiver {
        inner: inner,
        awakener: ReceiverAwakener(awakener),
    };
    Ok((tx, rx))
}
