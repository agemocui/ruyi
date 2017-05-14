mod event_loop;
use self::event_loop::{TaskId, TimerTaskId, EventLoop};

mod pollable_io;
pub use self::pollable_io::PollableIo;

mod timer;
pub use self::timer::{Timer, PeriodicTimer};

mod wheel;
pub use self::wheel::{Sleep, Timeout};

use std::borrow::Borrow;
use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::time::{Instant, Duration};

use futures::{Future, Stream, Sink};

use super::nio::{Pollable, Ops};

pub trait IntoStream {
    type Stream: Stream;

    fn into_stream(self) -> Self::Stream;
}

pub trait IntoSink {
    type Sink: Sink;

    fn into_sink(self) -> Self::Sink;
}

pub trait Split {
    type Stream: Stream;
    type Sink: Sink;

    fn split(self) -> (Self::Stream, Self::Sink);
}

pub type Task = Box<Future<Item = (), Error = ()>>;

pub trait IntoTask {
    fn into_task(self) -> Task;
}

impl<F: Future<Error = ()> + 'static> IntoTask for F {
    #[inline]
    fn into_task(self) -> Task {
        Box::new(self.map(drop))
    }
}

thread_local!(static CURRENT_LOOP: EventLoop = event_loop::new().unwrap());

pub struct Gate {
    _marker: PhantomData<()>,
}

impl Gate {
    #[inline]
    fn new() -> Option<Self> {
        match CURRENT_LOOP.with(|eloop| eloop.enter_gate()) {
            true => Some(Gate { _marker: PhantomData }),
            false => None,
        }
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        CURRENT_LOOP.with(|eloop| eloop.leave_gate());
    }
}

pub fn gate() -> Option<Gate> {
    Gate::new()
}

pub fn run<F>(f: F) -> Result<F::Item, F::Error>
    where F: Future
{
    CURRENT_LOOP.with(|eloop| {
                          info!("{} started", eloop);
                          let res = eloop.run(f);
                          info!("{} stopped", eloop);
                          res
                      })
}

pub fn spawn(f: Task) {
    CURRENT_LOOP.with(|eloop| { eloop.spawn(f); });
}

#[inline]
pub fn register<P>(io: P) -> PollableIo<P>
    where P: Pollable + fmt::Debug
{
    pollable_io::new(io)
}

#[inline]
pub fn sleep(dur: Duration) -> Sleep {
    wheel::sleep(dur)
}

#[inline]
fn schedule_at(at: Instant) -> TimerTaskId {
    CURRENT_LOOP.with(|eloop| eloop.schedule_at(at))
}

#[inline]
fn schedule(at: Instant, period: Duration) -> TimerTaskId {
    CURRENT_LOOP.with(|eloop| eloop.schedule(at, period))
}

#[inline]
fn register_io<P, B>(pollable: B, interested_ops: Ops) -> io::Result<usize>
    where P: Pollable,
          B: Borrow<P>
{
    CURRENT_LOOP.with(|eloop| eloop.register_io(pollable, interested_ops))
}

#[inline]
fn reregister_io<P, B>(pollable: B,
                       interested_ops: Ops,
                       sched_idx: usize,
                       sched_io_ops: Ops)
                       -> io::Result<()>
    where P: Pollable,
          B: Borrow<P>
{
    CURRENT_LOOP.with(|eloop| {
                          eloop.reregister_io(pollable, interested_ops, sched_idx, sched_io_ops)
                      })
}

#[inline]
fn deregister_io<P, B>(pollable: B, sched_idx: usize) -> io::Result<()>
    where P: Pollable,
          B: Borrow<P>
{
    CURRENT_LOOP.with(|eloop| eloop.deregister_io(pollable, sched_idx))
}

#[inline]
fn is_timer_task_expired(timer_task_id: TimerTaskId) -> bool {
    CURRENT_LOOP.with(|eloop| eloop.is_timer_task_expired(timer_task_id))
}

#[inline]
fn cancel_timer_task(timer_task_id: TimerTaskId) -> bool {
    CURRENT_LOOP.with(|eloop| eloop.cancel_timer_task(timer_task_id))
}

#[inline]
fn run_expired_task(task: TaskId) -> bool {
    CURRENT_LOOP.with(|eloop| eloop.run_task(task))
}

#[inline]
fn wt_schedule(dur: Duration) -> wheel::TimerId {
    CURRENT_LOOP.with(|eloop| eloop.wt_schedule(dur))
}

#[inline]
fn wt_cancel(timer_id: wheel::TimerId) {
    CURRENT_LOOP.with(|eloop| eloop.wt_cancel(timer_id))
}

#[inline]
fn wt_is_expired(timer_id: wheel::TimerId) -> bool {
    CURRENT_LOOP.with(|eloop| eloop.wt_is_expired(timer_id))
}
