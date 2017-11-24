use std::fmt;
use std::io;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;

use slab::{self, Slab};
use sys::poll::{Event, Ops, Poller};
use task::{Task, TaskId};

#[allow(dead_code)]
pub(crate) enum ReadyTasks {
    Single(TaskId),
    Pair(TaskId, TaskId),
}

#[derive(Clone)]
pub(super) struct Token {
    inner: usize,
    ready_ops: &'static Ops,
}

impl Token {
    #[inline]
    pub(super) fn as_inner(&self) -> usize {
        self.inner
    }

    #[inline]
    pub(super) fn is_read_ready(&self) -> bool {
        self.ready_ops.contains(Ops::READ)
    }

    #[inline]
    pub(super) fn is_write_ready(&self) -> bool {
        self.ready_ops.contains(Ops::WRITE)
    }
}

impl fmt::Debug for Token {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Token {{ inner: {}, ready_ops: {:?} }}",
            self.inner,
            *self.ready_ops
        )
    }
}

#[derive(Debug)]
struct Schedule {
    read_task: TaskId,
    write_task: TaskId,
    ready_ops: Ops,
}

pub(crate) struct EventLoop {
    id: usize,
    tasks: Slab<Task, TaskId>,
    schedules: Slab<Schedule>,
    current_task: TaskId,
    poller: Poller,
}

impl EventLoop {
    #[inline]
    pub(crate) fn with_schedule_capacity(cap: usize) -> io::Result<Self> {
        static SEQ: AtomicUsize = ATOMIC_USIZE_INIT;
        let id = SEQ.fetch_add(1, Ordering::Relaxed);
        Ok(EventLoop {
            id,
            tasks: slab::with_capacity(cap << 1),
            schedules: slab::with_capacity(cap),
            current_task: TaskId::invalid(),
            poller: Poller::new()?,
        })
    }

    #[inline]
    pub(crate) fn get_ready_tasks(&mut self, event: &Event) -> ReadyTasks {
        let schedule = unsafe { self.schedules.get_unchecked_mut(event.token()) };
        let (ready_tasks, ready_ops) =
            super::get_ready_tasks(event, schedule.read_task, schedule.write_task);
        schedule.ready_ops |= ready_ops;
        ready_tasks
    }

    #[inline]
    pub(crate) fn current_task(&self) -> TaskId {
        self.current_task
    }

    #[inline]
    pub(crate) fn set_current_task(&mut self, task_id: TaskId) {
        self.current_task = task_id;
    }

    #[inline]
    pub(crate) fn add_task(&mut self, task: Task) -> TaskId {
        self.tasks.insert(task)
    }

    #[inline]
    pub(crate) unsafe fn get_unchecked_mut_task(&mut self, task_id: TaskId) -> &mut Task {
        self.tasks.get_unchecked_mut(task_id)
    }

    #[inline]
    pub(crate) fn remove_task(&mut self, task_id: TaskId) {
        self.tasks.remove(task_id);
    }

    #[inline]
    pub(crate) fn clear(&mut self) {
        self.tasks.clear();
        self.set_current_task(TaskId::invalid());
    }

    #[inline]
    pub(crate) fn poll(
        &self,
        events: &mut Vec<Event>,
        timeout: Option<Duration>,
    ) -> io::Result<()> {
        let mut cap = events.capacity();
        if events.len() == cap {
            // double the capacity
            events.reserve_exact(cap);
            cap = events.capacity();
        }
        unsafe { events.set_len(cap) };
        let n = self.as_poller()
            .poll(events.as_mut_slice(), timeout)
            .or_else(|e| {
                unsafe { events.set_len(0) };
                Err(e)
            })?;
        unsafe { events.set_len(n) };
        Ok(())
    }

    #[inline]
    pub(super) fn as_poller(&self) -> &Poller {
        &self.poller
    }

    #[inline]
    pub(super) fn schedule(&mut self, ops: Ops) -> Token {
        let inner = self.schedules.insert(Schedule {
            read_task: self.current_task,
            write_task: self.current_task,
            ready_ops: Ops::all() ^ ops,
        });
        let ready_ops = unsafe { mem::transmute(&self.schedules.get_unchecked(inner).ready_ops) };
        Token { inner, ready_ops }
    }

    #[inline]
    pub(super) fn reschedule(&mut self, token: &Token, new_ops: Ops) {
        let schedule = unsafe { self.schedules.get_unchecked_mut(token.as_inner()) };
        schedule.ready_ops &= Ops::all() ^ new_ops;
        if new_ops.contains(Ops::READ) {
            schedule.read_task = self.current_task;
        }
        if new_ops.contains(Ops::WRITE) {
            schedule.write_task = self.current_task;
        }
    }

    #[inline]
    pub(super) fn cancel(&mut self, token: &Token) {
        self.schedules.remove(token.as_inner());
    }
}

impl fmt::Display for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ruyi-loop-{}", self.id)
    }
}

impl fmt::Debug for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "ruyi-loop-{}", self.id)
    }
}
