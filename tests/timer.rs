extern crate futures;
extern crate ruyi;

use std::time::{Instant, Duration};
use futures::Future;

use ruyi::reactor;

#[test]
#[ignore]
fn sleep() {
    const TIMEOUT: u64 = 10;
    let start = Instant::now();
    let sleep = reactor::sleep(Duration::from_secs(TIMEOUT)).then(|_| {
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
}
