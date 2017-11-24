pub mod net;

mod err;

pub(super) mod nio;

#[cfg(any(target_os = "linux", target_os = "android"))]
mod linux;
#[cfg(any(target_os = "linux", target_os = "android"))]
pub use self::linux::*;

mod awakener;
pub(crate) use self::awakener::*;

mod spsc;
pub(crate) use self::spsc::*;
