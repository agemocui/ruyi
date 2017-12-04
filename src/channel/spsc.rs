use std::cell::Cell;
use std::fmt;
use std::io;
use std::mem;
use std::ptr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use futures::stream::Stream;
use futures::Poll;

use sys::{self, Awakener};
use channel::err::{SendError, TrySendError};

#[cfg_attr(nightly, repr(align(64)))]
#[derive(Debug)]
struct Pointer(AtomicUsize);

impl Pointer {
    #[inline]
    fn load(&self, order: Ordering) -> usize {
        self.0.load(order)
    }

    #[inline]
    fn store(&self, val: usize, order: Ordering) {
        self.0.store(val, order);
    }
}

pub(crate) struct RingBuffer<T> {
    buf_ptr: *mut T,
    alloc_cap: usize,
    cap: usize,
    idx_mask: usize,

    front: Pointer,
    shadow_rear: Cell<usize>,

    rear: Pointer,
    shadow_front: Cell<usize>,
}

unsafe impl<T: Send> Send for RingBuffer<T> {}
unsafe impl<T> Sync for RingBuffer<T> {}

impl<T> RingBuffer<T> {
    #[inline]
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        let n = capacity.next_power_of_two();
        let mut buf = Vec::with_capacity(n);
        let buf_ptr = buf.as_mut_ptr();
        let alloc_cap = buf.capacity();
        mem::forget(buf);
        RingBuffer {
            buf_ptr,
            alloc_cap,
            cap: capacity,
            idx_mask: n.wrapping_sub(1),

            front: Pointer(AtomicUsize::new(0)),
            shadow_rear: Cell::new(0),

            rear: Pointer(AtomicUsize::new(0)),
            shadow_front: Cell::new(0),
        }
    }

    #[inline]
    fn effective_index(&self, index: usize) -> usize {
        index & self.idx_mask
    }

    pub(crate) fn try_push(&self, t: T) -> Option<T> {
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

    pub(crate) fn try_pop(&self) -> Option<T> {
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

impl<T> fmt::Debug for RingBuffer<T> {
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

impl<T> Drop for RingBuffer<T> {
    #[inline]
    fn drop(&mut self) {
        while let Some(t) = self.try_pop() {
            drop(t);
        }
        drop(unsafe { Vec::from_raw_parts(self.buf_ptr, 0, self.alloc_cap) });
    }
}

struct SenderAwakener(Arc<Awakener>);

impl SenderAwakener {
    #[inline]
    pub fn wakeup(&self) -> io::Result<()> {
        self.0.as_ref().wakeup()
    }
}

impl Drop for SenderAwakener {
    fn drop(&mut self) {
        if let Err(e) = self.wakeup() {
            if e.kind() != io::ErrorKind::WouldBlock {
                error!("Failed to wakeup: {}", e);
            }
        }
    }
}

pub(crate) struct ReceiverAwakener(Arc<Awakener>);

impl ReceiverAwakener {
    #[inline]
    pub(crate) fn reset(&self) -> io::Result<()> {
        self.0.as_ref().reset()
    }

    #[inline]
    pub(crate) fn as_inner(&self) -> &Awakener {
        self.0.as_ref()
    }
}

impl Drop for ReceiverAwakener {
    fn drop(&mut self) {
        if let Err(e) = self.reset() {
            if e.kind() != io::ErrorKind::WouldBlock {
                error!("Failed to reset: {}", e);
            }
        }
    }
}

pub struct SyncSender<T> {
    buffer: Arc<RingBuffer<T>>,
    awakener: SenderAwakener,
}

unsafe impl<T: Send> Send for SyncSender<T> {}
unsafe impl<T> Sync for SyncSender<T> {}

impl<T> SyncSender<T> {
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        if Arc::strong_count(&self.buffer) == 1 {
            return Err(TrySendError::Disconnected(t));
        }
        if let Some(t) = (&self.buffer).try_push(t) {
            return Err(TrySendError::Full(t));
        }
        self.awakener.wakeup()?;
        Ok(())
    }

    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        let mut msg = t;
        loop {
            if Arc::strong_count(&self.buffer) == 1 {
                return Err(SendError::Disconnected(msg));
            }
            match (&self.buffer).try_push(msg) {
                Some(t) => msg = t,
                None => break,
            }
        }
        self.awakener.wakeup()?;
        Ok(())
    }
}

impl<T> fmt::Debug for SyncSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "SyncSender {{ buffer: {:?} }}", &self.buffer)
    }
}

pub struct Receiver<T> {
    buffer: Arc<RingBuffer<T>>,
    awakener: ReceiverAwakener,
}

unsafe impl<T: Send> Send for Receiver<T> {}
unsafe impl<T> Sync for Receiver<T> {}

pub struct Recv<T>(sys::Recv<T>);

impl<T> Receiver<T> {
    #[inline]
    pub fn recv(self) -> io::Result<Recv<T>> {
        Ok(Recv(sys::Recv::try_new(self.buffer, self.awakener)?))
    }
}

impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Receiver {{ buffer: {:?} }}", &self.buffer)
    }
}

impl<T> Stream for Recv<T> {
    type Item = T;
    type Error = io::Error;

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        self.0.poll()
    }
}

pub fn sync_channel<T>(capacity: usize) -> io::Result<(SyncSender<T>, Receiver<T>)> {
    let buffer = Arc::new(RingBuffer::with_capacity(capacity));
    let awakener = Arc::new(Awakener::new()?);
    let tx = SyncSender {
        buffer: buffer.clone(),
        awakener: SenderAwakener(awakener.clone()),
    };
    let rx = Receiver {
        buffer,
        awakener: ReceiverAwakener(awakener),
    };
    Ok((tx, rx))
}
