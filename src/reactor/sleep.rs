use futures::{Future, Poll};

use reactor::wheel::Timer;

#[derive(Debug)]
pub struct Sleep {
    timer: Timer,
}

#[inline]
pub fn sleep(secs: u64) -> Sleep {
    Sleep {
        timer: Timer::new(secs),
    }
}

impl Future for Sleep {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        self.timer.poll()
    }
}
