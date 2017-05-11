use std::time::Duration;

fn into_millis(dur: Duration) -> u64 {
    const NANOS_PER_MILLI: u32 = 1_000_000;
    const MILLIS_PER_SEC: u64 = 1_000;

    let millis = (dur.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI;
    dur.as_secs()
        .wrapping_mul(MILLIS_PER_SEC)
        .wrapping_add(millis as u64)
}

#[cfg(unix)]
mod unix;

#[cfg(unix)]
pub use self::unix::*;

#[cfg(windows)]
mod windows;

#[cfg(windows)]
pub use self::windows::*;
