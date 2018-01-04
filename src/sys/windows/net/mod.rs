pub(crate) mod tcp;

pub fn init() {
    use std::net::UdpSocket;
    use std::sync::{Once, ONCE_INIT};

    static ONCE: Once = ONCE_INIT;

    ONCE.call_once(|| drop(UdpSocket::bind("127.0.0.1:0")));
}

use std::io;
use std::mem;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

use winapi::shared::guiddef::GUID;
use winapi::shared::minwindef::{DWORD, FALSE, TRUE};
use winapi::shared::winerror::ERROR_IO_PENDING;
use winapi::shared::ws2def::SIO_GET_EXTENSION_FUNCTION_POINTER;
use winapi::um::winsock2::{WSAGetLastError, WSAGetOverlappedResult, WSAIoctl, SOCKET, SOCKET_ERROR};

use sys::windows::nio::Overlapped;

#[inline]
fn last_error() -> io::Error {
    let err = unsafe { WSAGetLastError() } as DWORD;
    match err {
        ERROR_IO_PENDING => io::Error::from(io::ErrorKind::WouldBlock),
        _ => io::Error::from_raw_os_error(err as i32),
    }
}

#[inline]
fn get_overlapped_result(socket: SOCKET, overlapped: &mut Overlapped) -> io::Result<usize> {
    let mut transferred = 0;
    let mut flags = 0;
    let success = unsafe {
        WSAGetOverlappedResult(
            socket,
            overlapped.as_mut(),
            &mut transferred,
            FALSE,
            &mut flags,
        )
    };
    match success == TRUE {
        true => Ok(transferred as usize),
        false => Err(last_error()),
    }
}

struct WsaExtFunc {
    guid: GUID,
    func: AtomicUsize,
}

static ACCEPTEX: WsaExtFunc = WsaExtFunc {
    guid: GUID {
        Data1: 0xb5367df1,
        Data2: 0xcbac,
        Data3: 0x11cf,
        Data4: [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
    },
    func: ATOMIC_USIZE_INIT,
};

static GET_ACCEPTEX_SOCKADDRS: WsaExtFunc = WsaExtFunc {
    guid: GUID {
        Data1: 0xb5367df2,
        Data2: 0xcbac,
        Data3: 0x11cf,
        Data4: [0x95, 0xca, 0x00, 0x80, 0x5f, 0x48, 0xa1, 0x92],
    },
    func: ATOMIC_USIZE_INIT,
};

static CONNECTEX: WsaExtFunc = WsaExtFunc {
    guid: GUID {
        Data1: 0x25a207b9,
        Data2: 0xddf3,
        Data3: 0x4660,
        Data4: [0x8e, 0xe9, 0x76, 0xe5, 0x8c, 0x74, 0x06, 0x3e],
    },
    func: ATOMIC_USIZE_INIT,
};

impl WsaExtFunc {
    fn get(&self, socket: SOCKET) -> io::Result<usize> {
        let mut func = self.func.load(Ordering::Relaxed);
        if func != 0 {
            return Ok(func);
        }
        let mut bytes = 0;
        let result = unsafe {
            WSAIoctl(
                socket,
                SIO_GET_EXTENSION_FUNCTION_POINTER,
                &self.guid as *const _ as *mut _,
                mem::size_of_val(&self.guid) as DWORD,
                &mut func as *mut _ as *mut _,
                mem::size_of::<usize>() as DWORD,
                &mut bytes,
                ptr::null_mut(),
                None,
            )
        };
        if result == SOCKET_ERROR {
            let err = unsafe { WSAGetLastError() };
            Err(io::Error::from_raw_os_error(err))
        } else {
            self.func.store(func, Ordering::Relaxed);
            Ok(func)
        }
    }
}
