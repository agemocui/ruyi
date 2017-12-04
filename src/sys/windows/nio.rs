use std::cell::UnsafeCell;
use std::io;
use std::marker::PhantomData;
use std::mem;
use std::os::windows::io::AsRawSocket;
use std::rc::Rc;

use winapi;
use kernel32;

use reactor::CURRENT_LOOP;
use sys::{ReadyTasks, Schedule, Token};
use sys::nio::{Borrow, BorrowMut};
use sys::windows::poll::{Event, Ops};

pub trait AsRawHandle {
    fn as_raw_handle(&self) -> winapi::HANDLE;
}

impl<T: AsRawSocket> AsRawHandle for T {
    #[inline]
    fn as_raw_handle(&self) -> winapi::HANDLE {
        unsafe { mem::transmute(self.as_raw_socket()) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IoVec {
    inner: winapi::WSABUF,
}

unsafe impl Sync for IoVec {}

impl IoVec {
    #[inline]
    pub fn empty() -> &'static [Self] {
        static EMPTY: [IoVec; 1] = [
            IoVec {
                inner: winapi::WSABUF {
                    len: 0,
                    buf: 0 as *mut i8, // FIXME: ptr::null_mut()
                },
            },
        ];
        &EMPTY
    }
}

impl From<(*const u8, usize)> for IoVec {
    #[inline]
    fn from(slice: (*const u8, usize)) -> Self {
        IoVec {
            inner: winapi::WSABUF {
                len: slice.1 as winapi::ULONG,
                buf: unsafe { mem::transmute(slice.0) },
            },
        }
    }
}

impl<'a> From<&'a [u8]> for IoVec {
    #[inline]
    fn from(slice: &[u8]) -> Self {
        IoVec {
            inner: winapi::WSABUF {
                len: slice.len() as winapi::ULONG,
                buf: slice.as_ptr() as *mut _,
            },
        }
    }
}

impl<'a> From<&'a mut [u8]> for IoVec {
    #[inline]
    fn from(slice: &mut [u8]) -> Self {
        IoVec {
            inner: winapi::WSABUF {
                len: slice.len() as winapi::ULONG,
                buf: slice.as_ptr() as *mut _,
            },
        }
    }
}

impl AsMut<winapi::WSABUF> for IoVec {
    #[inline]
    fn as_mut(&mut self) -> &mut winapi::WSABUF {
        &mut self.inner
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct Overlapped {
    inner: winapi::OVERLAPPED,
    ops: Ops,
}

impl Overlapped {
    #[inline]
    pub fn for_read() -> Self {
        Overlapped {
            inner: unsafe { mem::zeroed() },
            ops: Ops::READ,
        }
    }

    #[inline]
    pub fn for_write() -> Self {
        Overlapped {
            inner: unsafe { mem::zeroed() },
            ops: Ops::WRITE,
        }
    }

    #[inline]
    pub fn is_read(&self) -> bool {
        self.ops == Ops::READ
    }
}

impl AsMut<winapi::OVERLAPPED> for Overlapped {
    #[inline]
    fn as_mut(&mut self) -> &mut winapi::OVERLAPPED {
        &mut self.inner
    }
}

pub trait Read {
    unsafe fn read(&mut self, buf: &mut [u8], overlapped: &mut Overlapped) -> io::Result<usize>;
}

pub trait Readv {
    unsafe fn readv(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize>;
}

pub trait Writev {
    unsafe fn writev(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize>;
}

impl<'a, R: Readv> Readv for &'a mut R {
    #[inline]
    unsafe fn readv(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize> {
        Readv::readv(*self, iovs, overlapped)
    }
}

impl<'a, W: Writev> Writev for &'a mut W {
    #[inline]
    unsafe fn writev(&mut self, iovs: &[IoVec], overlapped: &mut Overlapped) -> io::Result<usize> {
        Writev::writev(*self, iovs, overlapped)
    }
}

#[derive(Debug)]
pub struct Nio<H, T = H> {
    io: T,
    token: Token,
    _marker: PhantomData<H>,
}

impl<H, T> Nio<H, T> {
    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.io
    }

    #[inline]
    pub fn is_read_ready(&self) -> bool {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = current_loop.as_inner();
            let schedule = unsafe { eloop.get_schedule_unchecked(self.token) };
            schedule.is_read_ready()
        })
    }

    #[inline]
    pub fn is_write_ready(&self) -> bool {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = current_loop.as_inner();
            let schedule = unsafe { eloop.get_schedule_unchecked(self.token) };
            schedule.is_write_ready()
        })
    }

    #[inline]
    pub fn schedule_read(&self) {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() };
            eloop.as_mut_inner().schedule_read(self.token);
        })
    }

    #[inline]
    pub fn schedule_write(&self) {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() };
            eloop.as_mut_inner().schedule_write(self.token);
        })
    }
}

impl<H, T> Nio<H, T>
where
    H: AsRawHandle,
    T: AsRef<H>,
{
    pub fn try_from(io: T) -> io::Result<Self> {
        let token = Self::register(io.as_ref().as_raw_handle())?;
        Ok(Nio {
            io,
            token,
            _marker: PhantomData,
        })
    }

    #[inline]
    fn register(handle: winapi::HANDLE) -> io::Result<Token> {
        const FILE_SKIP_COMPLETION_PORT_ON_SUCCESS: winapi::UCHAR = 0x1;
        const FILE_SKIP_SET_EVENT_ON_HANDLE: winapi::UCHAR = 0x2;
        const FLAGS: winapi::UCHAR =
            FILE_SKIP_COMPLETION_PORT_ON_SUCCESS | FILE_SKIP_SET_EVENT_ON_HANDLE;

        let success = unsafe { kernel32::SetFileCompletionNotificationModes(handle, FLAGS) };
        if success == winapi::FALSE {
            return Err(io::Error::last_os_error());
        }

        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            let token = eloop.schedule();
            match eloop.as_poller().register(handle, token) {
                Ok(()) => Ok(token),
                Err(e) => {
                    eloop.cancel(token);
                    Err(e)
                }
            }
        })
    }
}

impl<H, T> Drop for Nio<H, T> {
    #[inline]
    fn drop(&mut self) {
        CURRENT_LOOP.with(|eloop| {
            unsafe { eloop.as_mut() }.as_mut_inner().cancel(self.token);
        })
    }
}

impl<H, T> Borrow<Nio<H, T>> for Rc<UnsafeCell<Nio<H, T>>> {
    #[inline]
    fn borrow(&self) -> &Nio<H, T> {
        unsafe { &*self.as_ref().get() }
    }
}

impl<H, T> BorrowMut<Nio<H, T>> for Rc<UnsafeCell<Nio<H, T>>> {
    #[inline]
    fn borrow_mut(&mut self) -> &mut Nio<H, T> {
        unsafe { &mut *self.as_ref().get() }
    }
}

#[inline]
pub(in sys) fn get_ready_tasks(event: &Event, schedule: &mut Schedule) -> ReadyTasks {
    match event.is_read() {
        true => {
            schedule.set_read_ready(true);
            ReadyTasks::Single(schedule.read_task())
        }
        false => {
            schedule.set_write_ready(true);
            ReadyTasks::Single(schedule.write_task())
        }
    }
}
