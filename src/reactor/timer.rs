use std::time::{Duration, Instant};

use futures::{Future, Stream, Async, Poll};

use super::event_loop::TimerTaskId;

#[derive(Debug, Clone, Copy)]
enum TimerState {
    Unscheduled(Instant),
    Scheduled(TimerTaskId),
    Cancelled,
}

#[derive(Debug)]
pub struct Timer {
    state: TimerState,
}

impl Timer {
    #[inline]
    pub fn new(dur: Duration) -> Self {
        Timer {
            state: TimerState::Unscheduled(Instant::now() + dur),
        }
    }

    #[inline]
    fn cancel(&mut self) -> bool {
        match self.state {
            TimerState::Scheduled(timer_task_id) => {
                self.state = TimerState::Cancelled;
                super::cancel_timer_task(timer_task_id)
            }
            TimerState::Unscheduled(..) => {
                self.state = TimerState::Cancelled;
                true
            }
            TimerState::Cancelled => true,
        }
    }
}

impl Future for Timer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.state {
            TimerState::Scheduled(timer_task_id) => {
                match super::is_timer_task_expired(timer_task_id) {
                    true => Ok(Async::Ready(())),
                    false => Ok(Async::NotReady),
                }
            }
            TimerState::Unscheduled(at) => {
                let id = super::schedule_at(at);
                self.state = TimerState::Scheduled(id);
                Ok(Async::NotReady)
            }
            TimerState::Cancelled => ::unreachable(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Debug)]
enum PeriodicTimerState {
    Unscheduled(Instant, Duration),
    Scheduled(TimerTaskId),
    Expired(TimerTaskId),
    Cancelled,
}

#[derive(Debug)]
pub struct PeriodicTimer {
    state: PeriodicTimerState,
}

impl PeriodicTimer {
    #[inline]
    pub fn new(dur: Duration, period: Duration) -> Self {
        PeriodicTimer {
            state: PeriodicTimerState::Unscheduled(Instant::now() + dur, period),
        }
    }

    #[inline]
    fn cancel(&mut self) -> bool {
        match self.state {
            PeriodicTimerState::Scheduled(timer_task_id) |
            PeriodicTimerState::Expired(timer_task_id) => {
                self.state = PeriodicTimerState::Cancelled;
                super::cancel_timer_task(timer_task_id)
            }
            PeriodicTimerState::Unscheduled(..) => {
                self.state = PeriodicTimerState::Cancelled;
                true
            }
            PeriodicTimerState::Cancelled => true,
        }
    }
}

impl Drop for PeriodicTimer {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Stream for PeriodicTimer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.state {
            PeriodicTimerState::Unscheduled(at, period) => {
                let id = super::schedule(at, period);
                self.state = PeriodicTimerState::Scheduled(id);
                Ok(Async::NotReady)
            }
            PeriodicTimerState::Scheduled(timer_task_id) => {
                match super::is_timer_task_expired(timer_task_id) {
                    true => {
                        self.state = PeriodicTimerState::Expired(timer_task_id);
                        Ok(Async::Ready(Some(())))
                    }
                    false => Ok(Async::NotReady),
                }
            }
            PeriodicTimerState::Expired(timer_task_id) => {
                self.state = PeriodicTimerState::Scheduled(timer_task_id);
                Ok(Async::NotReady)
            }
            PeriodicTimerState::Cancelled => ::unreachable(),
        }
    }
}
