use std::cell::UnsafeCell;
use std::io;
use std::sync::atomic::Ordering;

use winapi;
use kernel32;
use reactor::CURRENT_LOOP;

use sys::{AtomicBool, Token};
use sys::windows::nio::{AsRawHandle, Overlapped};
use sys::windows::poll::Ops;

pub(crate) struct Awakener {
    need_wakeup: AtomicBool,
    nio: UnsafeCell<Option<(winapi::HANDLE, Token)>>,
    overlapped: UnsafeCell<Overlapped>,
}

impl Awakener {
    #[inline]
    pub(crate) fn new() -> io::Result<Self> {
        Ok(Awakener {
            need_wakeup: AtomicBool::new(false),
            nio: UnsafeCell::new(None),
            overlapped: UnsafeCell::new(Overlapped::for_read()),
        })
    }

    #[inline]
    pub(crate) fn wakeup(&self) -> io::Result<()> {
        // Ordering::SeqCst ensures all the write-ops before this
        // need_wakeup.load since last wakeup will be seen by all
        // the read-ops after the need_wakeup.store in reset thread.
        if self.need_wakeup.load(Ordering::SeqCst) == true {
            self.need_wakeup.store(false, Ordering::SeqCst);
            self.post_queued_completion_status()?;
        }
        Ok(())
    }

    #[inline]
    pub(crate) fn reset(&self) -> io::Result<()> {
        self.need_wakeup.store(true, Ordering::SeqCst);
        Ok(())
    }

    // register has to be called before 1st reset.
    #[inline]
    pub(super) fn register(&self) {
        debug_assert!(self.as_nio().is_none());
        let (iocp, token) = CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            let iocp = eloop.as_poller().as_raw_handle();
            let token = eloop.schedule(Ops::empty());
            (iocp, token)
        });
        *self.as_mut_nio() = Some((iocp, token));
    }

    #[inline]
    fn post_queued_completion_status(&self) -> io::Result<()> {
        if let Some(ref nio) = (*self.as_nio()).as_ref() {
            let result = unsafe {
                kernel32::PostQueuedCompletionStatus(
                    nio.0,
                    0,
                    nio.1.as_inner() as winapi::ULONG_PTR,
                    self.as_mut_overlapped().as_mut(),
                )
            };
            if result == winapi::FALSE {
                return Err(io::Error::last_os_error());
            }
        }
        Ok(())
    }

    #[inline]
    fn as_nio(&self) -> &Option<(winapi::HANDLE, Token)> {
        unsafe { &*self.nio.get() }
    }

    #[inline]
    fn as_mut_overlapped(&self) -> &mut Overlapped {
        unsafe { &mut *self.overlapped.get() }
    }

    #[inline]
    fn as_mut_nio(&self) -> &mut Option<(winapi::HANDLE, Token)> {
        unsafe { &mut *self.nio.get() }
    }
}

impl Drop for Awakener {
    fn drop(&mut self) {
        if let Some(ref nio) = (*self.as_nio()).as_ref() {
            CURRENT_LOOP.with(|eloop| {
                unsafe { eloop.as_mut() }.as_mut_inner().cancel(&nio.1)
            })
        }
    }
}
