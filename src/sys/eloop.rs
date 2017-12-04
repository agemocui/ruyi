use std::fmt;
use std::io;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::Duration;

use slab::{self, Slab};
use task::{Task, TaskId};
use sys::poll::{Event, Poller};

#[allow(dead_code)]
pub(crate) enum ReadyTasks {
    Single(TaskId),
    Pair(TaskId, TaskId),
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Token(usize);

impl From<Token> for usize {
    #[inline]
    fn from(token: Token) -> Self {
        token.0
    }
}

#[derive(Debug)]
pub(super) struct Schedule {
    read_task: TaskId,
    write_task: TaskId,
    read_ready: bool,
    write_ready: bool,
}

impl Schedule {
    #[inline]
    pub fn new(task: TaskId) -> Self {
        Schedule {
            read_task: task,
            write_task: task,
            read_ready: true,
            write_ready: true,
        }
    }

    #[inline]
    pub fn read_task(&self) -> TaskId {
        self.read_task
    }

    #[inline]
    pub fn write_task(&self) -> TaskId {
        self.write_task
    }

    #[inline]
    pub fn set_read_task(&mut self, task: TaskId) {
        self.read_task = task;
    }

    #[inline]
    pub fn set_write_task(&mut self, task: TaskId) {
        self.write_task = task;
    }

    #[inline]
    pub fn is_read_ready(&self) -> bool {
        self.read_ready
    }

    #[inline]
    pub fn is_write_ready(&self) -> bool {
        self.write_ready
    }

    #[inline]
    pub fn set_read_ready(&mut self, ready: bool) {
        self.read_ready = ready;
    }

    #[inline]
    pub fn set_write_ready(&mut self, ready: bool) {
        self.write_ready = ready;
    }
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
        super::get_ready_tasks(event, schedule)
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
        let cap = events.capacity();
        unsafe { events.set_len(cap) };
        let n = self.as_poller().poll(events.as_mut_slice(), timeout)?;
        unsafe { events.set_len(n) };
        Ok(())
    }

    #[inline]
    pub(super) fn as_poller(&self) -> &Poller {
        &self.poller
    }

    #[inline]
    pub(super) fn schedule(&mut self) -> Token {
        Token(self.schedules.insert(Schedule::new(self.current_task)))
    }

    #[inline]
    pub(super) fn schedule_read(&mut self, token: Token) {
        let schedule = unsafe { self.schedules.get_unchecked_mut(token.into()) };
        schedule.set_read_ready(false);
        schedule.set_read_task(self.current_task);
    }

    #[inline]
    pub(super) fn schedule_write(&mut self, token: Token) {
        let schedule = unsafe { self.schedules.get_unchecked_mut(token.into()) };
        schedule.set_write_ready(false);
        schedule.set_write_task(self.current_task);
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn cancel_read(&mut self, token: Token) {
        let schedule = unsafe { self.schedules.get_unchecked_mut(token.into()) };
        schedule.set_read_ready(true);
    }

    #[inline]
    #[allow(dead_code)]
    pub(super) fn cancel_write(&mut self, token: Token) {
        let schedule = unsafe { self.schedules.get_unchecked_mut(token.into()) };
        schedule.set_write_ready(true);
    }

    #[inline]
    pub(super) fn cancel(&mut self, token: Token) {
        self.schedules.remove(token.into());
    }

    #[inline]
    pub(super) unsafe fn get_schedule_unchecked(&self, token: Token) -> &Schedule {
        self.schedules.get_unchecked(token.into())
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
