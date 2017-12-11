use std::cmp;
use std::collections::binary_heap::BinaryHeap;
use std::time::{Duration, Instant};

use futures::{Async, Future, Poll, Stream};

use slab::{self, Slab};
use task;
use reactor::eloop::TaskRunner;
use reactor::CURRENT_LOOP;

#[derive(Debug, Clone, Copy)]
pub(super) struct TaskId {
    // index in slab Queue.tasks
    index: usize,
}

impl From<usize> for TaskId {
    #[inline]
    fn from(index: usize) -> Self {
        TaskId { index }
    }
}

impl From<TaskId> for usize {
    #[inline]
    fn from(task_id: TaskId) -> Self {
        task_id.index
    }
}

#[derive(Debug)]
enum State {
    Unscheduled(Instant),
    Scheduled(TaskId),
    Cancelled,
}

#[derive(Debug)]
pub struct Timer {
    state: State,
}

impl Timer {
    #[inline]
    pub fn new(dur: Duration) -> Self {
        Timer {
            state: State::Unscheduled(Instant::now() + dur),
        }
    }

    #[inline]
    fn cancel(&mut self) -> bool {
        match self.state {
            State::Scheduled(task_id) => {
                self.state = State::Cancelled;
                cancel_task(task_id)
            }
            State::Unscheduled(..) => {
                self.state = State::Cancelled;
                true
            }
            State::Cancelled => true,
        }
    }

    #[inline]
    fn schedule_at(at: Instant) -> TaskId {
        CURRENT_LOOP.with(|eloop| {
            unsafe { eloop.as_mut() }
                .as_mut_timer_queue()
                .add(at, eloop.current_task())
        })
    }
}

impl Future for Timer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.state {
            State::Scheduled(task_id) => match is_task_expired(task_id) {
                true => Ok(Async::Ready(())),
                false => Ok(Async::NotReady),
            },
            State::Unscheduled(at) => {
                let id = Self::schedule_at(at);
                self.state = State::Scheduled(id);
                Ok(Async::NotReady)
            }
            State::Cancelled => ::unreachable(),
        }
    }
}

impl Drop for Timer {
    #[inline]
    fn drop(&mut self) {
        self.cancel();
    }
}

#[derive(Debug)]
enum PeriodicState {
    Unscheduled(Instant, Duration),
    Scheduled(TaskId),
    Expired(TaskId),
    Cancelled,
}

#[derive(Debug)]
pub struct PeriodicTimer {
    state: PeriodicState,
}

impl PeriodicTimer {
    #[inline]
    pub fn new(dur: Duration, period: Duration) -> Self {
        PeriodicTimer {
            state: PeriodicState::Unscheduled(Instant::now() + dur, period),
        }
    }

    #[inline]
    fn cancel(&mut self) -> bool {
        match self.state {
            PeriodicState::Scheduled(task_id) | PeriodicState::Expired(task_id) => {
                self.state = PeriodicState::Cancelled;
                cancel_task(task_id)
            }
            PeriodicState::Unscheduled(..) => {
                self.state = PeriodicState::Cancelled;
                true
            }
            PeriodicState::Cancelled => true,
        }
    }

    #[inline]
    fn schedule(at: Instant, period: Duration) -> TaskId {
        CURRENT_LOOP.with(|eloop| {
            unsafe { eloop.as_mut() }.as_mut_timer_queue().add_periodic(
                at,
                period,
                eloop.current_task(),
            )
        })
    }
}

impl Drop for PeriodicTimer {
    #[inline]
    fn drop(&mut self) {
        self.cancel();
    }
}

impl Stream for PeriodicTimer {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Option<Self::Item>, Self::Error> {
        match self.state {
            PeriodicState::Unscheduled(at, period) => {
                let id = Self::schedule(at, period);
                self.state = PeriodicState::Scheduled(id);
                Ok(Async::NotReady)
            }
            PeriodicState::Scheduled(task_id) => match is_task_expired(task_id) {
                true => {
                    self.state = PeriodicState::Expired(task_id);
                    Ok(Async::Ready(Some(())))
                }
                false => Ok(Async::NotReady),
            },
            PeriodicState::Expired(task_id) => {
                self.state = PeriodicState::Scheduled(task_id);
                Ok(Async::NotReady)
            }
            PeriodicState::Cancelled => ::unreachable(),
        }
    }
}

// TaskState Transition
// Oneshot -(expire)-> Expired, remove token from Queue.heap
// Oneshot -(cancel)-> Cancelled, wait for event loop to remove from Queue.tasks
// Periodic -(expire) -> Periodic, remove from Queue.heap, reschedule if not cancelled
// Periodic -(cancel)-> Cancelled, wait for event loop to remove from Queue.tasks
// Expired -(cancel)-> Remove from Queue.tasks
// Expired -(expire)-> Should not happen
// Cancelled -(cancel)-> Should not happen
// Cancelled -(expire)-> Remove from Queue.tasks
#[derive(Clone, Copy)]
enum TaskState {
    Oneshot,
    Periodic(Duration),
    Expired,
    Cancelled,
}

struct Task {
    id: task::TaskId,
    state: TaskState,
}

impl Task {
    #[inline]
    fn oneshot(id: task::TaskId) -> Self {
        Task {
            id,
            state: TaskState::Oneshot,
        }
    }

    #[inline]
    fn periodic(id: task::TaskId, period: Duration) -> Self {
        Task {
            id,
            state: TaskState::Periodic(period),
        }
    }

    #[inline]
    fn id(&self) -> task::TaskId {
        self.id
    }

    #[inline]
    fn set_state(&mut self, state: TaskState) {
        self.state = state;
    }

    #[inline]
    fn state(&self) -> TaskState {
        self.state
    }
}

#[derive(Clone, Copy)]
struct Expiration {
    inner: Instant,
    // index in slab Queue.tasks
    task_id: TaskId,
}

impl Expiration {
    #[inline]
    fn new(inner: Instant, task_id: TaskId) -> Self {
        Expiration { inner, task_id }
    }

    #[inline]
    fn timestamp(&self) -> Instant {
        self.inner
    }

    #[inline]
    fn task_id(&self) -> TaskId {
        self.task_id
    }
}

impl PartialEq for Expiration {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Expiration {}

impl PartialOrd for Expiration {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Expiration {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        other.inner.cmp(&self.inner)
    }
}

pub(super) struct Queue {
    tasks: Slab<Task, TaskId>,
    heap: BinaryHeap<Expiration>,
}

impl Queue {
    #[inline]
    pub(super) fn new() -> Self {
        Queue {
            tasks: slab::new(),
            heap: BinaryHeap::new(),
        }
    }

    #[inline]
    pub(super) fn poll(&mut self, runner: &mut TaskRunner) -> Option<Option<Duration>> {
        loop {
            match self.heap.peek() {
                Some(exp) => {
                    let now = Instant::now();
                    if exp.timestamp() > now {
                        return Some(Some(exp.timestamp() - now));
                    }
                }
                None => return Some(None),
            }
            let exp = match self.heap.pop() {
                Some(exp) => exp,
                None => ::unreachable(),
            };
            let remove_task = {
                let task = unsafe { self.tasks.get_unchecked_mut(exp.task_id()) };
                match task.state() {
                    TaskState::Periodic(dur) => {
                        task.set_state(TaskState::Expired);
                        if runner.run_task(task.id()) {
                            return None;
                        }
                        match task.state() {
                            TaskState::Cancelled => true,
                            _ => {
                                task.set_state(TaskState::Periodic(dur));
                                let at = exp.timestamp() + dur;
                                // reschedule
                                self.heap.push(Expiration::new(at, exp.task_id()));
                                false
                            }
                        }
                    }
                    TaskState::Oneshot => {
                        task.set_state(TaskState::Expired);
                        if runner.run_task(task.id()) {
                            return None;
                        }
                        false
                    }
                    TaskState::Cancelled => true,
                    TaskState::Expired => ::unreachable(),
                }
            };
            if remove_task {
                self.tasks.remove(exp.task_id());
            }
        }
    }

    #[inline]
    fn cancel(&mut self, task_id: TaskId) -> bool {
        {
            let task = unsafe { self.tasks.get_unchecked_mut(task_id) };
            match task.state() {
                TaskState::Oneshot | TaskState::Periodic(..) => {
                    task.set_state(TaskState::Cancelled);
                    return true;
                }
                TaskState::Expired => task.set_state(TaskState::Cancelled),
                TaskState::Cancelled => return true, // should not happen
            }
        }
        // Remove timer task from queue on expiration
        self.tasks.remove(task_id);
        false
    }

    #[inline]
    fn add(&mut self, at: Instant, task: task::TaskId) -> TaskId {
        let id = self.tasks.insert(Task::oneshot(task));
        self.heap.push(Expiration::new(at, id));
        id
    }

    #[inline]
    fn add_periodic(&mut self, at: Instant, period: Duration, task: task::TaskId) -> TaskId {
        let id = self.tasks.insert(Task::periodic(task, period));
        self.heap.push(Expiration::new(at, id));
        id
    }
}

#[inline]
fn cancel_task(task_id: TaskId) -> bool {
    CURRENT_LOOP.with(|eloop| {
        unsafe { eloop.as_mut() }
            .as_mut_timer_queue()
            .cancel(task_id)
    })
}

#[inline]
fn is_task_expired(task_id: TaskId) -> bool {
    CURRENT_LOOP.with(|eloop| {
        let queue = unsafe { eloop.as_mut() }.as_mut_timer_queue();
        let state = unsafe { queue.tasks.get_unchecked(task_id) }.state();
        match state {
            TaskState::Expired => true,
            _ => false,
        }
    })
}
