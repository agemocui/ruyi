use std::io;
use std::mem;
use std::ptr;
use std::time::Duration;

use winapi::shared::basetsd::ULONG_PTR;
use winapi::shared::minwindef::{DWORD, FALSE, TRUE, ULONG};
use winapi::shared::winerror::WAIT_TIMEOUT;
use winapi::um::handleapi::{CloseHandle, INVALID_HANDLE_VALUE};
use winapi::um::ioapiset::{CreateIoCompletionPort, GetQueuedCompletionStatusEx};
use winapi::um::minwinbase::{LPOVERLAPPED_ENTRY, OVERLAPPED_ENTRY};
use winapi::um::winbase::INFINITE;
use winapi::um::winnt::HANDLE;

use sys::windows::nio::{AsRawHandle, Overlapped};

bitflags! {
    pub struct Ops: u32 {
        const READ      = 0b_0000_0001;
        const WRITE     = 0b_0000_0010;
    }
}

#[repr(C)]
pub struct Event {
    inner: OVERLAPPED_ENTRY,
}

impl Event {
    #[inline]
    pub fn token(&self) -> usize {
        self.inner.lpCompletionKey as usize
    }

    #[inline]
    pub(super) fn is_read(&self) -> bool {
        let overlapped: &Overlapped = unsafe { mem::transmute(self.inner.lpOverlapped) };
        overlapped.is_read()
    }
}

#[derive(Debug)]
pub struct Poller {
    iocp: HANDLE,
}

impl Poller {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let iocp = unsafe { CreateIoCompletionPort(INVALID_HANDLE_VALUE, ptr::null_mut(), 0, 1) };
        match iocp.is_null() {
            false => Ok(Poller { iocp }),
            true => Err(io::Error::last_os_error()),
        }
    }

    #[inline]
    pub fn poll(&self, events: &mut [Event], timeout: Option<Duration>) -> io::Result<usize> {
        let entry_ptr = events.as_mut_ptr() as LPOVERLAPPED_ENTRY;
        let count = events.len() as ULONG;
        let mut n: ULONG = 0;
        let millis = match timeout {
            Some(dur) => ::into_millis(dur) as DWORD,
            None => INFINITE,
        };
        let success = unsafe {
            GetQueuedCompletionStatusEx(self.iocp, entry_ptr, count, &mut n, millis, FALSE)
        } == TRUE;
        match success {
            true => Ok(n as usize),
            false => match io::Error::last_os_error() {
                ref e if e.raw_os_error() == Some(WAIT_TIMEOUT as i32) => Ok(0),
                e => Err(e),
            },
        }
    }

    #[inline]
    pub(super) fn register<T>(&self, handle: HANDLE, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let iocp =
            unsafe { CreateIoCompletionPort(handle, self.iocp, token.into() as ULONG_PTR, 0) };
        match iocp.is_null() {
            false => Ok(()),
            true => Err(io::Error::last_os_error()),
        }
    }
}

impl AsRawHandle for Poller {
    #[inline]
    fn as_raw_handle(&self) -> HANDLE {
        self.iocp
    }
}

impl Drop for Poller {
    #[inline]
    fn drop(&mut self) {
        let success = unsafe { CloseHandle(self.iocp) };
        if success == FALSE {
            error!("Failed to close {:?}: {}", self, io::Error::last_os_error());
        }
    }
}
