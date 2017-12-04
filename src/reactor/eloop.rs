use std::fmt;
use std::io;
use std::mem;

use futures::{Async, Future, Poll};

use sys;
use task::{Task, TaskId};
use reactor::{timer, Wheel};

// FIXME: Make it into global config
const SCHEDULE_CAPACITY: usize = 512;

pub(super) struct TaskRunner {
    inner: sys::EventLoop,
    spawn_stack: Vec<TaskId>,
    gate: usize,
    main_task: Option<&'static mut Future<Item = (), Error = ()>>,
}

impl TaskRunner {
    #[inline]
    fn new() -> io::Result<Self> {
        Ok(TaskRunner {
            inner: ::sys::EventLoop::with_schedule_capacity(SCHEDULE_CAPACITY)?,
            spawn_stack: Vec::new(),
            gate: 0,
            main_task: None,
        })
    }

    #[inline]
    fn set_main_task(&mut self, main_task: &mut Future<Item = (), Error = ()>) {
        self.main_task = Some(unsafe { mem::transmute(main_task) });
    }

    #[inline]
    fn run(&mut self, timer_queue: &mut timer::Queue) {
        if self.run_main_task() {
            return;
        }
        let mut events = Vec::with_capacity(SCHEDULE_CAPACITY * 2);
        'exit: loop {
            match timer_queue.poll(self) {
                Some(timeout) => if let Err(e) = self.inner.poll(&mut events, timeout) {
                    panic!("{} poll error: {:?}", self.inner, e);
                },
                None => break,
            }
            for event in &events {
                use sys::ReadyTasks::{Pair, Single};
                match self.inner.get_ready_tasks(event) {
                    Single(task) => if self.run_task(task) {
                        break 'exit;
                    },
                    Pair(task1, task2) => if self.run_task(task1) || self.run_task(task2) {
                        break 'exit;
                    },
                }
            }
            let n = events.len();
            unsafe { events.set_len(0) };
            if n == events.capacity() {
                events.reserve_exact(n + n); // double the capacity
            }
        }
    }

    #[inline]
    fn spawn(&mut self, task: Task) {
        debug_assert!(self.main_task.is_some(), "Missing main task");

        self.spawn_stack.push(self.inner.current_task());
        let current_task = self.inner.add_task(task);
        self.inner.set_current_task(current_task);
        match unsafe { self.inner.get_unchecked_mut_task(current_task) }.poll() {
            Ok(Async::NotReady) => {}
            _ => drop(self.inner.remove_task(current_task)),
        }
        match self.spawn_stack.pop() {
            Some(task_id) => self.inner.set_current_task(task_id),
            None => ::unreachable(),
        }
    }

    #[inline]
    fn run_main_task(&mut self) -> bool {
        match self.main_task {
            Some(ref mut main_task) => match main_task.poll() {
                Ok(Async::NotReady) => return false,
                _ => {}
            },
            _ => ::unreachable(),
        }
        self.main_task = None;
        !self.has_gate()
    }

    #[inline]
    pub(super) fn run_task(&mut self, task: TaskId) -> bool {
        self.inner.set_current_task(task);
        if task.is_valid() {
            match unsafe { self.inner.get_unchecked_mut_task(task) }.poll() {
                Ok(Async::NotReady) => {}
                _ => self.inner.remove_task(task),
            }
            self.main_task.is_none() && !self.has_gate()
        } else {
            self.run_main_task()
        }
    }

    #[inline]
    fn clear(&mut self) {
        self.inner.clear();
        self.main_task = None;
    }

    #[inline]
    fn has_gate(&self) -> bool {
        self.gate != 0
    }

    #[inline]
    fn enter_gate(&mut self) -> bool {
        match self.main_task {
            Some(_) => {
                self.gate += 1;
                true
            }
            None => false,
        }
    }

    #[inline]
    fn leave_gate(&mut self) {
        self.gate -= 1;
    }
}

struct MainTask<F: Future> {
    inner: Option<F>,
    ret: Option<Result<F::Item, F::Error>>,
}

impl<F: Future> MainTask<F> {
    #[inline]
    fn new(f: F) -> Self {
        MainTask {
            inner: Some(f),
            ret: None,
        }
    }

    #[inline]
    fn take_result(&mut self) -> Result<F::Item, F::Error> {
        self.ret.take().unwrap()
    }
}

impl<F: Future> Future for MainTask<F> {
    type Item = ();
    type Error = ();

    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        match self.inner.as_mut().unwrap().poll() {
            Ok(Async::NotReady) => Ok(Async::NotReady),
            Ok(Async::Ready(t)) => {
                self.ret = Some(Ok(t));
                self.inner = None;
                Ok(Async::Ready(()))
            }
            Err(e) => {
                self.ret = Some(Err(e));
                self.inner = None;
                Err(())
            }
        }
    }
}

pub(crate) struct EventLoop {
    runner: TaskRunner,
    timer_queue: timer::Queue,
    wheel: Wheel,
}

impl EventLoop {
    #[inline]
    pub(super) fn new() -> io::Result<Self> {
        Ok(EventLoop {
            runner: TaskRunner::new()?,
            timer_queue: timer::Queue::new(),
            wheel: Wheel::new(),
        })
    }

    #[inline]
    pub(crate) unsafe fn as_mut(&self) -> &mut Self {
        mem::transmute(self as *const Self as *mut Self)
    }

    #[inline]
    pub(crate) fn as_inner(&self) -> &sys::EventLoop {
        &self.runner.inner
    }

    #[inline]
    pub(crate) fn as_mut_inner(&mut self) -> &mut sys::EventLoop {
        &mut self.runner.inner
    }

    #[inline]
    pub(super) fn run<F>(&mut self, f: F) -> Result<F::Item, F::Error>
    where
        F: Future,
    {
        let mut main_task = MainTask::new(f);
        self.runner.set_main_task(&mut main_task);
        self.runner.run(&mut self.timer_queue);
        self.clear();
        main_task.take_result()
    }

    #[inline]
    pub(super) fn spawn(&mut self, task: Task) {
        self.runner.spawn(task);
    }

    #[inline]
    pub(super) fn enter_gate(&mut self) -> bool {
        self.runner.enter_gate()
    }

    #[inline]
    pub(super) fn leave_gate(&mut self) {
        self.runner.leave_gate();
    }

    #[inline]
    pub(super) fn as_mut_task_runner(&mut self) -> &mut TaskRunner {
        &mut self.runner
    }

    #[inline]
    pub(super) fn as_mut_timer_queue(&mut self) -> &mut timer::Queue {
        &mut self.timer_queue
    }

    #[inline]
    pub(super) fn as_wheel(&self) -> &Wheel {
        &self.wheel
    }

    #[inline]
    pub(super) fn as_mut_wheel(&mut self) -> &mut Wheel {
        &mut self.wheel
    }

    #[inline]
    pub(super) fn current_task(&self) -> TaskId {
        self.runner.inner.current_task()
    }

    #[inline]
    fn clear(&mut self) {
        self.runner.clear();
    }
}

impl fmt::Display for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.runner.inner.fmt(f)
    }
}

impl fmt::Debug for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.runner.inner.fmt(f)
    }
}
