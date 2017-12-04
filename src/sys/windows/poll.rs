use std::io;
use std::mem;
use std::ptr;
use std::time::Duration;

use winapi;
use kernel32;

use sys::windows::nio::{AsRawHandle, Overlapped};

bitflags! {
    pub struct Ops: u32 {
        const READ      = 0b_0000_0001;
        const WRITE     = 0b_0000_0010;
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Event {
    inner: winapi::OVERLAPPED_ENTRY,
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
    iocp: winapi::HANDLE,
}

impl Poller {
    #[inline]
    pub fn new() -> io::Result<Self> {
        let iocp = unsafe {
            kernel32::CreateIoCompletionPort(winapi::INVALID_HANDLE_VALUE, ptr::null_mut(), 0, 1)
        };
        match iocp.is_null() {
            false => Ok(Poller { iocp }),
            true => Err(io::Error::last_os_error()),
        }
    }

    #[inline]
    pub fn poll(&self, events: &mut [Event], timeout: Option<Duration>) -> io::Result<usize> {
        let entry_ptr = events.as_mut_ptr() as winapi::LPOVERLAPPED_ENTRY;
        let count = events.len() as winapi::ULONG;
        let mut n: winapi::ULONG = 0;
        let millis = match timeout {
            Some(dur) => ::into_millis(dur) as winapi::DWORD,
            None => winapi::INFINITE,
        };
        let success = unsafe {
            kernel32::GetQueuedCompletionStatusEx(
                self.iocp,
                entry_ptr,
                count,
                &mut n,
                millis,
                winapi::FALSE,
            )
        } == winapi::TRUE;
        match success {
            true => Ok(n as usize),
            false => match io::Error::last_os_error() {
                ref e if e.raw_os_error() == Some(winapi::WAIT_TIMEOUT as i32) => Ok(0),
                e => Err(e),
            },
        }
    }

    #[inline]
    pub(super) fn register<T>(&self, handle: winapi::HANDLE, token: T) -> io::Result<()>
    where
        T: Into<usize>,
    {
        let iocp = unsafe {
            kernel32::CreateIoCompletionPort(
                handle,
                self.iocp,
                token.into() as winapi::ULONG_PTR,
                0,
            )
        };
        match iocp.is_null() {
            false => Ok(()),
            true => Err(io::Error::last_os_error()),
        }
    }
}

impl AsRawHandle for Poller {
    #[inline]
    fn as_raw_handle(&self) -> winapi::HANDLE {
        self.iocp
    }
}

impl Drop for Poller {
    #[inline]
    fn drop(&mut self) {
        let success = unsafe { kernel32::CloseHandle(self.iocp) };
        if success == winapi::FALSE {
            error!("Failed to close {:?}: {}", self, io::Error::last_os_error());
        }
    }
}
