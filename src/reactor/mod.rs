mod eloop;
pub(crate) use self::eloop::EventLoop;

thread_local!(pub(crate) static CURRENT_LOOP: EventLoop = EventLoop::new().unwrap());

mod gate;
pub use self::gate::Gate;

mod timer;
pub use self::timer::{PeriodicTimer, Timer};

mod wheel;
use self::wheel::Wheel;

mod sleep;
pub use self::sleep::*;

mod timeout;
pub use self::timeout::*;

use futures::Future;
use task::Task;

pub fn run<F>(f: F) -> Result<F::Item, F::Error>
where
    F: Future,
{
    CURRENT_LOOP.with(|eloop| {
        debug!("{} started", eloop);
        let res = unsafe { eloop.as_mut() }.run(f);
        debug!("{} stopped", eloop);
        res
    })
}

pub fn spawn(task: Task) {
    CURRENT_LOOP.with(|eloop| unsafe { eloop.as_mut() }.spawn(task))
}
