extern crate futures;
extern crate ruyi;

use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;

use futures::{future, Async, Future, Poll};
use ruyi::IntoTask;
use ruyi::reactor::{self, Gate, Timer};

static COUNT: AtomicUsize = ATOMIC_USIZE_INIT;

struct GateHolder {
    _gate: Option<Gate>,
    timer: Timer,
}

impl GateHolder {
    fn with_gate() -> Self {
        GateHolder {
            _gate: Some(Gate::new().unwrap()),
            timer: Timer::new(Duration::from_millis(200)),
        }
    }

    fn without_gate() -> Self {
        GateHolder {
            _gate: None,
            timer: Timer::new(Duration::from_millis(200)),
        }
    }
}

impl Future for GateHolder {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.timer.poll() {
            Ok(Async::Ready(..)) => {
                COUNT.fetch_add(1, Ordering::Relaxed);
                Ok(Async::Ready(()))
            }
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Err(()) => Err(()),
        }
    }
}

#[test]
fn gate() {
    let task_without_gate = future::ok::<(), ()>(())
        .and_then(|_| Ok(reactor::spawn(GateHolder::without_gate().into_task())));
    reactor::run(task_without_gate).unwrap();
    assert_eq!(COUNT.load(Ordering::Relaxed), 0);

    let task_with_gate = future::ok::<(), ()>(())
        .and_then(|_| Ok(reactor::spawn(GateHolder::with_gate().into_task())));
    reactor::run(task_with_gate).unwrap();
    assert_eq!(COUNT.load(Ordering::Relaxed), 1);
}
