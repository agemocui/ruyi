pub mod net;

mod err;

pub(super) mod nio;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::linux::*;

#[cfg(any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd",
          target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd"))]
mod bsd;
#[cfg(any(target_os = "bitrig", target_os = "dragonfly", target_os = "freebsd",
          target_os = "ios", target_os = "macos", target_os = "netbsd", target_os = "openbsd"))]
pub use self::bsd::*;

mod awakener;
pub(crate) use self::awakener::*;

mod spsc;
pub(crate) use self::spsc::*;
