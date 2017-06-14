extern crate futures;
extern crate ruyi;

use std::time::{Instant, Duration};
use futures::{Future, Async};

use ruyi::reactor::{self, Timer};

fn into_millis(dur: Duration) -> u64 {
    const NANOS_PER_MILLI: u32 = 1_000_000;
    const MILLIS_PER_SEC: u64 = 1_000;

    let millis = (dur.subsec_nanos() + NANOS_PER_MILLI - 1) / NANOS_PER_MILLI;
    dur.as_secs()
        .wrapping_mul(MILLIS_PER_SEC)
        .wrapping_add(millis as u64)
}

#[test]
#[ignore]
fn sleep() {
    const TIMEOUT: u64 = 10;
    let start = Instant::now();
    let sleep = reactor::sleep(TIMEOUT).then(|e| {
        assert_eq!(e.is_ok(), true);
        let elapsed = Instant::now() - start;
        if elapsed.as_secs() < TIMEOUT - 1 {
            Err(format!("Expect elapsed({:?}) >= {}s", elapsed, TIMEOUT - 1))
        } else if elapsed.as_secs() > TIMEOUT + 1 {
            Err(format!("Expect elapsed({:?}) <= {}s", elapsed, TIMEOUT + 1))
        } else {
            Ok(())
        }
    });
    reactor::run(sleep).unwrap();

    assert_eq!(reactor::sleep(0).poll(), Ok(Async::Ready(())));
}

#[test]
#[ignore]
fn timer() {
    const TIMEOUT: u64 = 100;
    let start = Instant::now();
    let sleep = Timer::new(Duration::from_millis(TIMEOUT)).then(|res| {
        assert_eq!(res.is_ok(), true);
        let elapsed = Instant::now() - start;
        if into_millis(elapsed) < TIMEOUT - 1 {
            Err(format!("Expect elapsed({:?}) >= {}ms", elapsed, TIMEOUT - 1))
        } else if into_millis(elapsed) > TIMEOUT + 1 {
            Err(format!("Expect elapsed({:?}) <= {}ms", elapsed, TIMEOUT + 1))
        } else {
            Ok(())
        }
    });
    reactor::run(sleep).unwrap();
}
