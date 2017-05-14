use std::borrow::Borrow;
use std::cell::UnsafeCell;
use std::cmp;
use std::collections::binary_heap::BinaryHeap;
use std::fmt;
use std::io;
use std::mem;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};
use std::time::{Duration, Instant};

use futures::{Future, Poll, Async};

use super::Task;
use super::wheel::{TimerId, Wheel};
use super::super::slab::{self, Slab};
use super::super::nio::{Event, Poller, Pollable, Ops, Token};
use super::super::unreachable;

#[derive(Debug, Clone, Copy)]
pub struct TimerTaskId {
    // index in slab Queue.timers
    index: usize,
}

impl From<usize> for TimerTaskId {
    #[inline]
    fn from(index: usize) -> Self {
        TimerTaskId { index }
    }
}

impl From<TimerTaskId> for usize {
    #[inline]
    fn from(timer_task_id: TimerTaskId) -> Self {
        timer_task_id.index
    }
}

#[derive(Debug, Clone, Copy)]
pub struct TaskId {
    // index in Inner.tasks
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

impl TaskId {
    #[inline]
    fn is_valid(self) -> bool {
        self.index != slab::invalid_index()
    }

    #[inline]
    fn invalid() -> Self {
        slab::invalid_index()
    }
}

// TimerTaskState Transition
// Oneshot -(expire)-> Expired, remove token from Queue.heap
// Oneshot -(cancel)-> Cancelled, wait for eventloop to remove from Queue.tasks
// Periodic -(expire) -> Periodic, remove from Queue.heap, reschedule if not cancelled
// Periodic -(cancel)-> Cancelled, wait for eventloop to remove from Queue.tasks
// Expired -(cancel)-> Remove from Queue.tasks
// Expired -(expire)-> Should not happen
// Cancelled -(cancel)-> Should not happen
// Cancelled -(expire)-> Remove from Queue.tasks
#[derive(Debug, Clone, Copy)]
enum TimerTaskState {
    Oneshot,
    Periodic(Duration),
    Expired,
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
struct TimerTask {
    state: TimerTaskState,
    task: TaskId,
}

impl TimerTask {
    #[inline]
    fn oneshot(task: TaskId) -> Self {
        TimerTask {
            state: TimerTaskState::Oneshot,
            task,
        }
    }

    #[inline]
    fn periodic(period: Duration, task: TaskId) -> Self {
        TimerTask {
            state: TimerTaskState::Periodic(period),
            task,
        }
    }

    #[inline]
    fn state(&self) -> TimerTaskState {
        self.state
    }

    #[inline]
    fn set_state(&mut self, state: TimerTaskState) {
        self.state = state;
    }

    #[inline]
    fn task(&self) -> TaskId {
        self.task
    }
}

#[derive(Debug, Clone, Copy)]
struct Expiration {
    inner: Instant,
    // index in slab Queue.timers
    timer_task_id: TimerTaskId,
}

impl Expiration {
    #[inline]
    fn new(inner: Instant, timer_task_id: TimerTaskId) -> Self {
        Expiration {
            inner,
            timer_task_id,
        }
    }

    #[inline]
    fn timestamp(&self) -> Instant {
        self.inner
    }

    #[inline]
    fn timer_task_id(&self) -> TimerTaskId {
        self.timer_task_id
    }
}

impl PartialEq for Expiration {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl Eq for Expiration {}

impl PartialOrd for Expiration {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Expiration {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        other.inner.cmp(&self.inner)
    }
}

#[derive(Debug)]
struct TimerQueue {
    timer_tasks: Slab<TimerTask, TimerTaskId>,
    heap: BinaryHeap<Expiration>,
}

enum PollResult {
    Expire(Expiration),
    Wait(Option<Duration>),
}

impl TimerQueue {
    #[inline]
    fn new() -> Self {
        TimerQueue {
            timer_tasks: slab::new(),
            heap: BinaryHeap::new(),
        }
    }

    #[inline]
    fn add(&mut self, at: Instant, task: TaskId) -> TimerTaskId {
        let id = self.timer_tasks.insert(TimerTask::oneshot(task));
        self.heap.push(Expiration::new(at, id));
        id
    }

    #[inline]
    fn add_periodic(&mut self, at: Instant, period: Duration, task: TaskId) -> TimerTaskId {
        let id = self.timer_tasks
            .insert(TimerTask::periodic(period, task));
        self.heap.push(Expiration::new(at, id));
        id
    }

    #[inline]
    fn reschedule(&mut self, at: Instant, timer_task_id: TimerTaskId) {
        self.heap.push(Expiration::new(at, timer_task_id));
    }

    #[inline]
    fn cancel(&mut self, id: TimerTaskId) -> bool {
        {
            let timer_task = unsafe { self.timer_tasks.get_unchecked_mut(id) };
            match timer_task.state() {
                TimerTaskState::Oneshot |
                TimerTaskState::Periodic(..) => {
                    timer_task.set_state(TimerTaskState::Cancelled);
                    return true;
                }
                TimerTaskState::Expired => timer_task.set_state(TimerTaskState::Cancelled),
                TimerTaskState::Cancelled => return true, // should not happen
            }
        }
        // Remove timer task from queue on expiration
        self.timer_tasks.remove(id);
        false
    }

    #[inline]
    fn poll(&mut self) -> PollResult {
        match self.heap.peek() {
            Some(exp) => {
                let now = Instant::now();
                if exp.timestamp() > now {
                    return PollResult::Wait(Some(exp.timestamp() - now));
                }
            }
            None => return PollResult::Wait(None),
        }
        let exp = self.heap.pop().unwrap();
        PollResult::Expire(exp)
    }

    #[inline]
    fn get(&self, timer_task_id: TimerTaskId) -> &TimerTask {
        unsafe { self.timer_tasks.get_unchecked(timer_task_id) }
    }

    #[inline]
    fn get_mut(&mut self, timer_task_id: TimerTaskId) -> &mut TimerTask {
        unsafe { self.timer_tasks.get_unchecked_mut(timer_task_id) }
    }

    #[inline]
    fn remove(&mut self, timer_task_id: TimerTaskId) {
        self.timer_tasks.remove(timer_task_id);
    }
}

#[derive(Clone, Copy)]
struct SchedIo {
    task_r: TaskId,
    task_w: TaskId,
}

#[derive(Clone, Copy)]
pub struct EventLoopId(usize);

struct Inner {
    id: EventLoopId,
    // `tasks` has to be dropped before `sched_ios` and `timers`.
    // So please keep `sched_ios` and `timers` below `tasks`
    tasks: Slab<Task, TaskId>,
    sched_ios: Slab<SchedIo>,
    timer_queue: TimerQueue,
    wheel: Option<Wheel>,

    current_task: TaskId,
    spawn_stack: Vec<TaskId>,
    poller: Poller,
    gate: usize,
    main_task: Option<&'static mut Future<Item = (), Error = ()>>,
}

pub struct EventLoop {
    inner: UnsafeCell<Inner>,
}

#[inline]
pub fn new() -> io::Result<EventLoop> {
    Ok(EventLoop { inner: UnsafeCell::new(Inner::new()?) })
}

impl SchedIo {
    #[inline]
    fn new(task_r: TaskId, task_w: TaskId) -> Self {
        SchedIo {
            task_r: task_r,
            task_w: task_w,
        }
    }

    #[inline]
    fn task_r(&self) -> TaskId {
        self.task_r
    }

    #[inline]
    fn set_task_r(&mut self, task_r: TaskId) {
        self.task_r = task_r;
    }

    #[inline]
    fn task_w(&self) -> TaskId {
        self.task_w
    }

    #[inline]
    fn set_task_w(&mut self, task_w: TaskId) {
        self.task_w = task_w;
    }
}

impl fmt::Display for EventLoopId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Loop-{}", self.0)
    }
}

impl fmt::Debug for EventLoopId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Loop-{}", self.0)
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

impl Inner {
    fn new() -> io::Result<Self> {
        static SEQ: AtomicUsize = ATOMIC_USIZE_INIT;
        let id = EventLoopId(SEQ.fetch_add(1, Ordering::Relaxed));
        Ok(Inner {
               id: id,
               tasks: slab::with_capacity(512),
               sched_ios: slab::with_capacity(512),
               timer_queue: TimerQueue::new(),
               wheel: None,
               current_task: TaskId::invalid(),
               spawn_stack: Vec::new(),
               poller: Poller::new()?,
               gate: 0,
               main_task: None,
           })
    }

    #[inline]
    fn enter_gate(&mut self) -> bool {
        if self.main_task.is_none() {
            false
        } else {
            self.gate += 1;
            true
        }
    }

    #[inline]
    fn leave_gate(&mut self) {
        self.gate -= 1;
    }

    #[inline]
    fn run(&mut self, main_task: &mut Future<Item = (), Error = ()>) {
        self.main_task = Some(unsafe { mem::transmute(main_task) });
        let mut events = Vec::with_capacity(self.sched_ios.capacity());
        loop {
            match self.poll_timer_queue() {
                Ok(timeout) => {
                    self.poll(&mut events, timeout).unwrap();
                }
                Err(()) => break,
            }
            if self.poll_timer_queue().is_err() {
                break;
            }

            for event in &events {
                let sched_io = unsafe { *self.sched_ios.get_unchecked(event.token().into()) };
                if event.ready_ops().contains_read() {
                    if self.run_task(sched_io.task_r()) {
                        break;
                    }
                }
                if event.ready_ops().contains_write() {
                    if self.run_task(sched_io.task_w()) {
                        break;
                    }
                }
            }
        }
        self.tasks.clear();
    }

    #[inline]
    fn run_task(&mut self, task_id: TaskId) -> bool {
        self.current_task = task_id;
        if task_id.is_valid() {
            match unsafe { self.tasks.get_unchecked_mut(task_id) }.poll() {
                Ok(Async::NotReady) => {}
                _ => drop(self.tasks.remove(task_id)),
            }
            self.main_task.is_none() && self.gate == 0
        } else {
            let main_task = self.main_task.take().unwrap();
            match main_task.poll() {
                Ok(Async::NotReady) => {
                    self.main_task = Some(main_task);
                    false
                }
                _ => self.gate == 0,
            }
        }
    }

    #[inline]
    fn is_timer_task_expired(&self, timer_task_id: TimerTaskId) -> bool {
        match self.timer_queue.get(timer_task_id).state() {
            TimerTaskState::Expired => true,
            _ => false,
        }
    }

    // true to exit
    #[inline]
    fn run_timer_task(&mut self, exp: Expiration) -> bool {
        let timer_task = *self.timer_queue.get(exp.timer_task_id());
        match timer_task.state() {
            TimerTaskState::Periodic(period) => {
                self.timer_queue
                    .get_mut(exp.timer_task_id())
                    .set_state(TimerTaskState::Expired);
                if self.run_task(timer_task.task()) {
                    return true;
                }
                match self.timer_queue.get(exp.timer_task_id()).state() {
                    TimerTaskState::Cancelled => self.timer_queue.remove(exp.timer_task_id()),
                    _ => {
                        self.timer_queue
                            .get_mut(exp.timer_task_id())
                            .set_state(TimerTaskState::Periodic(period));
                        let at = exp.timestamp() + period;
                        self.timer_queue.reschedule(at, exp.timer_task_id());
                    }
                }
            }
            TimerTaskState::Oneshot => {
                self.timer_queue
                    .get_mut(exp.timer_task_id())
                    .set_state(TimerTaskState::Expired);
                return self.run_task(timer_task.task());
            }
            TimerTaskState::Cancelled => self.timer_queue.remove(exp.timer_task_id()),
            TimerTaskState::Expired => unreachable(),
        }
        false
    }

    #[inline]
    fn poll_timer_queue(&mut self) -> Result<Option<Duration>, ()> {
        loop {
            match self.timer_queue.poll() {
                PollResult::Expire(exp) => {
                    if self.run_timer_task(exp) {
                        return Err(());
                    }
                }
                PollResult::Wait(wait) => return Ok(wait),
            }
        }
    }

    #[inline]
    fn spawn(&mut self, task: Task) {
        self.spawn_stack.push(self.current_task);
        self.current_task = self.tasks.insert(task);
        match unsafe { self.tasks.get_unchecked_mut(self.current_task) }.poll() {
            Ok(Async::NotReady) => {}
            _ => drop(self.tasks.remove(self.current_task)),
        }
        self.current_task = self.spawn_stack.pop().unwrap();
    }

    #[inline]
    fn poll(&self, events: &mut Vec<Event>, timeout: Option<Duration>) -> io::Result<()> {
        let mut cap = events.capacity();
        if events.len() == cap {
            // double the capacity
            events.reserve_exact(cap);
            cap = events.capacity();
        }
        unsafe { events.set_len(cap) };
        let n = self.poller
            .poll(events.as_mut_slice(), timeout)
            .or_else(|e| {
                         unsafe { events.set_len(0) };
                         Err(e)
                     })?;
        unsafe { events.set_len(n) };
        Ok(())
    }

    #[inline]
    fn register_io<P, B>(&mut self, pollable: B, interested_ops: Ops) -> io::Result<usize>
        where P: Pollable,
              B: Borrow<P>
    {
        let sched_idx = self.sched_ios
            .insert(SchedIo::new(self.current_task, self.current_task));
        match pollable
                  .borrow()
                  .register(&self.poller, interested_ops, Token::from(sched_idx)) {
            Ok(()) => Ok(sched_idx),
            Err(e) => {
                self.sched_ios.remove(sched_idx);
                Err(e)
            }
        }
    }

    #[inline]
    fn reregister_io<P, B>(&mut self,
                           pollable: B,
                           interested_ops: Ops,
                           sched_idx: usize,
                           sched_io_ops: Ops)
                           -> io::Result<()>
        where P: Pollable,
              B: Borrow<P>
    {
        let sched_io = unsafe { self.sched_ios.get_unchecked_mut(sched_idx) };
        if sched_io_ops.contains_read() {
            sched_io.set_task_r(self.current_task);
        }
        if sched_io_ops.contains_write() {
            sched_io.set_task_w(self.current_task);
        }
        pollable
            .borrow()
            .reregister(&self.poller, interested_ops, Token::from(sched_idx))
    }

    #[inline]
    fn deregister_io<P, B>(&mut self, pollable: B, sched_idx: usize) -> io::Result<()>
        where P: Pollable,
              B: Borrow<P>
    {
        match pollable.borrow().deregister(&self.poller) {
            Ok(()) => {
                self.sched_ios.remove(sched_idx);
                Ok(())
            }
            Err(e) => Err(e),
        }
    }

    #[inline]
    fn schedule_at(&mut self, at: Instant) -> TimerTaskId {
        self.timer_queue.add(at, self.current_task)
    }

    #[inline]
    fn schedule(&mut self, at: Instant, period: Duration) -> TimerTaskId {
        self.timer_queue
            .add_periodic(at, period, self.current_task)
    }

    #[inline]
    fn cancel_timer_task(&mut self, timer_task_id: TimerTaskId) -> bool {
        self.timer_queue.cancel(timer_task_id)
    }

    #[inline]
    fn wt_schedule(&mut self, dur: Duration) -> TimerId {
        match self.wheel.as_ref() {
            None => self.wheel = Some(Wheel::new()),
            _ => (),
        }
        match self.wheel.as_mut() {
            Some(wheel) => wheel.schedule(dur, self.current_task),
            _ => unreachable(),
        }
    }

    #[inline]
    fn wt_cancel(&mut self, timer_id: TimerId) {
        match self.wheel.as_mut() {
            Some(wheel) => wheel.cancel(timer_id),
            None => unreachable(),
        }
    }

    #[inline]
    fn wt_is_expired(&self, timer_id: TimerId) -> bool {
        match self.wheel.as_ref() {
            Some(wheel) => wheel.is_expired(timer_id),
            None => unreachable(),
        }
    }
}

impl fmt::Display for Inner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.id.fmt(f)
    }
}

impl EventLoop {
    #[inline]
    pub fn run<F>(&self, mut f: F) -> Result<F::Item, F::Error>
        where F: Future
    {
        match f.poll() {
            Ok(Async::NotReady) => {}
            Ok(Async::Ready(t)) => return Ok(t),
            Err(e) => return Err(e),
        }

        let mut main_task = MainTask::new(f);
        self.as_mut_inner().run(&mut main_task);
        main_task.take_result()
    }

    #[inline]
    pub fn spawn(&self, task: Task) {
        self.as_mut_inner().spawn(task);
    }

    #[inline]
    pub fn register_io<P, B>(&self, pollable: B, interested_ops: Ops) -> io::Result<usize>
        where P: Pollable,
              B: Borrow<P>
    {
        self.as_mut_inner().register_io(pollable, interested_ops)
    }

    #[inline]
    pub fn reregister_io<P, B>(&self,
                               pollable: B,
                               interested_ops: Ops,
                               sched_idx: usize,
                               sched_io_ops: Ops)
                               -> io::Result<()>
        where P: Pollable,
              B: Borrow<P>
    {
        self.as_mut_inner()
            .reregister_io(pollable, interested_ops, sched_idx, sched_io_ops)
    }

    #[inline]
    pub fn deregister_io<P, B>(&self, pollable: B, sched_idx: usize) -> io::Result<()>
        where P: Pollable,
              B: Borrow<P>
    {
        self.as_mut_inner().deregister_io(pollable, sched_idx)
    }

    #[inline]
    pub fn schedule_at(&self, at: Instant) -> TimerTaskId {
        self.as_mut_inner().schedule_at(at)
    }

    #[inline]
    pub fn schedule(&self, at: Instant, period: Duration) -> TimerTaskId {
        self.as_mut_inner().schedule(at, period)
    }

    #[inline]
    pub fn is_timer_task_expired(&self, timer_task_id: TimerTaskId) -> bool {
        self.as_inner().is_timer_task_expired(timer_task_id)
    }

    #[inline]
    pub fn cancel_timer_task(&self, timer_task_id: TimerTaskId) -> bool {
        self.as_mut_inner().cancel_timer_task(timer_task_id)
    }

    #[inline]
    pub fn enter_gate(&self) -> bool {
        self.as_mut_inner().enter_gate()
    }

    #[inline]
    pub fn leave_gate(&self) {
        self.as_mut_inner().leave_gate();
    }

    #[inline]
    pub fn run_task(&self, task_id: TaskId) -> bool {
        self.as_mut_inner().run_task(task_id)
    }

    #[inline]
    pub fn wt_schedule(&self, dur: Duration) -> TimerId {
        self.as_mut_inner().wt_schedule(dur)
    }

    #[inline]
    pub fn wt_cancel(&self, timer_id: TimerId) {
        self.as_mut_inner().wt_cancel(timer_id);
    }

    #[inline]
    pub fn wt_is_expired(&self, timer_id: TimerId) -> bool {
        self.as_inner().wt_is_expired(timer_id)
    }

    #[inline]
    fn as_mut_inner(&self) -> &mut Inner {
        unsafe { mem::transmute(self.inner.get()) }
    }

    #[inline]
    fn as_inner(&self) -> &Inner {
        self.as_mut_inner()
    }
}

impl fmt::Display for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_inner().fmt(f)
    }
}

impl fmt::Debug for EventLoop {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.as_inner().fmt(f)
    }
}
