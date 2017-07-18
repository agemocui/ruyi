use std::cell::UnsafeCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use futures::{Stream, Poll, Async};

use super::event_loop::TaskId;
use super::{IntoTask, PeriodicTimer};
use slab::{self, Slab};

#[derive(Debug, Clone, Copy)]
pub struct TimerId {
    index: usize,
}

impl From<usize> for TimerId {
    #[inline]
    fn from(index: usize) -> Self {
        TimerId { index }
    }
}

impl From<TimerId> for usize {
    #[inline]
    fn from(timer_id: TimerId) -> Self {
        timer_id.index
    }
}

#[derive(Clone, Copy)]
enum Token {
    Entry(TimerId),
    Slot(usize),
    Nil,
}

struct Entry {
    prev: Token,
    next: Token,
    task: TaskId,
    expiration: Option<Instant>,
}

impl Entry {
    #[inline]
    fn new(task: TaskId) -> Self {
        Entry {
            prev: Token::Nil,
            next: Token::Nil,
            task,
            expiration: None,
        }
    }
}

struct Slot {
    prev: Token,
    next: Token,
}

struct Inner {
    entries: Slab<Entry, TimerId>,
    slots: Vec<Slot>,
    current_slot: usize,
    mask: usize,
}

impl Inner {
    fn new() -> Self {
        const NUM_OF_SLOTS: usize = 128; // must be power of 2
        const INIT_CAPACITY: usize = 512;

        let num_of_slots = NUM_OF_SLOTS.next_power_of_two();
        let mut slots = Vec::<Slot>::with_capacity(num_of_slots);
        let mask = num_of_slots - 1;
        unsafe { slots.set_len(num_of_slots) };
        for (i, slot) in slots.iter_mut().enumerate() {
            slot.prev = Token::Slot(i.wrapping_sub(1) & mask);
            slot.next = Token::Slot((i + 1) & mask);
        }

        Inner {
            entries: slab::with_capacity(INIT_CAPACITY),
            slots,
            current_slot: 0,
            mask,
        }
    }

    #[inline]
    fn get_mut_entry(&mut self, timer_id: TimerId) -> &mut Entry {
        unsafe { self.entries.get_unchecked_mut(timer_id) }
    }

    #[inline]
    fn get_entry(&self, timer_id: TimerId) -> &Entry {
        unsafe { self.entries.get_unchecked(timer_id) }
    }

    #[inline]
    fn get_slot(&mut self, slot_idx: usize) -> &Slot {
        unsafe { self.slots.get_unchecked(slot_idx) }
    }

    #[inline]
    fn get_mut_slot(&mut self, slot_idx: usize) -> &mut Slot {
        unsafe { self.slots.get_unchecked_mut(slot_idx) }
    }

    #[inline]
    fn effective_slot(&self, slot: usize) -> usize {
        slot & self.mask
    }

    #[inline]
    fn is_expired(&self, timer_id: TimerId) -> bool {
        match self.get_entry(timer_id).prev {
            Token::Nil => true,
            _ => false,
        }
    }

    #[inline]
    fn round_to_secs(dur: Duration) -> u64 {
        if dur.subsec_nanos() >= 500_000_000 {
            dur.as_secs() + 1
        } else {
            dur.as_secs()
        }
    }

    #[inline]
    fn schedule(&mut self, dur: Duration, task: TaskId) -> TimerId {
        let mut timeout = dur.as_secs() as usize;
        let mut entry = Entry::new(task);
        if timeout > self.mask {
            timeout = self.mask;
            entry.expiration = Some(Instant::now() + dur);
        }
        let slot_idx = self.effective_slot(self.current_slot.wrapping_add(timeout));
        let prev = self.get_slot(slot_idx).prev;
        entry.prev = prev;
        entry.next = Token::Slot(slot_idx);
        let timer_id = self.entries.insert(entry);
        let token = Token::Entry(timer_id);
        match prev {
            Token::Entry(prev_timer_id) => self.get_mut_entry(prev_timer_id).next = token,
            Token::Slot(prev_slot_idx) => self.get_mut_slot(prev_slot_idx).next = token,
            Token::Nil => ::unreachable(),
        }
        self.get_mut_slot(slot_idx).prev = token;
        timer_id
    }

    fn reschedule(&mut self, dur: Duration, exp: Option<Instant>, timer_id: TimerId) {
        if exp.is_none() {
            // cancel, then reschedule
            let (prev, next) = {
                let entry = self.get_entry(timer_id);
                (entry.prev, entry.next)
            };
            match prev {
                Token::Entry(prev_timer_id) => self.get_mut_entry(prev_timer_id).next = next,
                Token::Slot(prev_slot_idx) => self.get_mut_slot(prev_slot_idx).next = next,
                Token::Nil => ::unreachable(),
            }
            match next {
                Token::Entry(next_timer_id) => self.get_mut_entry(next_timer_id).prev = prev,
                Token::Slot(next_slot_idx) => self.get_mut_slot(next_slot_idx).prev = prev,
                Token::Nil => ::unreachable(),
            }
        }
        let mut timeout = Self::round_to_secs(dur) as usize;
        if timeout > self.mask {
            timeout = self.mask;
            self.get_mut_entry(timer_id).expiration = if exp.is_none() {
                Some(Instant::now() + dur)
            } else {
                exp
            };
        }
        let slot_idx = self.effective_slot(self.current_slot.wrapping_add(timeout));
        let prev = self.get_slot(slot_idx).prev;
        {
            let entry = self.get_mut_entry(timer_id);
            entry.prev = prev;
            entry.next = Token::Slot(slot_idx);
        }
        let token = Token::Entry(timer_id);
        match prev {
            Token::Entry(prev_timer_id) => self.get_mut_entry(prev_timer_id).next = token,
            Token::Slot(prev_slot_idx) => self.get_mut_slot(prev_slot_idx).next = token,
            Token::Nil => ::unreachable(),
        }
        self.get_mut_slot(slot_idx).prev = token;
    }

    #[inline]
    fn cancel(&mut self, timer_id: TimerId) {
        let entry = self.entries.remove(timer_id).unwrap();
        match entry.prev {
            Token::Entry(timer_id) => self.get_mut_entry(timer_id).next = entry.next,
            Token::Slot(slot_idx) => self.get_mut_slot(slot_idx).next = entry.next,
            Token::Nil => return, // expired
        }
        match entry.next {
            Token::Entry(timer_id) => self.get_mut_entry(timer_id).prev = entry.prev,
            Token::Slot(slot_idx) => self.get_mut_slot(slot_idx).prev = entry.prev,
            Token::Nil => ::unreachable(), // expired
        }
    }

    #[inline]
    fn tick(&mut self) {
        let current_slot = self.current_slot;
        let mut timer_id = match self.get_slot(current_slot).next {
            Token::Slot(index) => {
                self.current_slot = index;
                return;
            }
            Token::Entry(id) => id,
            Token::Nil => ::unreachable(),
        };
        let next_slot;
        let mut exit = false;
        loop {
            let (next, expiration, task) = {
                let entry = self.get_mut_entry(timer_id);
                let next = entry.next;
                entry.prev = Token::Nil;
                entry.next = Token::Nil;
                (next, entry.expiration.take(), entry.task)
            };
            if !exit {
                if let Some(exp) = expiration {
                    let now = Instant::now();
                    if exp >= now + Duration::from_secs(1) {
                        self.reschedule(exp - now, Some(exp), timer_id);
                    } else {
                        exit = super::run_expired_task(task);
                    }
                } else {
                    exit = super::run_expired_task(task);
                }
            }
            match next {
                Token::Entry(id) => timer_id = id,
                Token::Slot(index) => {
                    next_slot = index;
                    break;
                }
                Token::Nil => ::unreachable(),
            }
        }
        self.get_mut_slot(current_slot).next = Token::Slot(next_slot);
        self.get_mut_slot(next_slot).prev = Token::Slot(self.current_slot);
        self.current_slot = next_slot;
    }
}

pub(super) struct Wheel {
    inner: Rc<UnsafeCell<Inner>>,
}

impl Wheel {
    pub fn new() -> Self {
        let dur = Duration::from_secs(1);
        let inner = Rc::new(UnsafeCell::new(Inner::new()));
        let wheel = Wheel {
            inner: inner.clone(),
        };
        super::spawn(
            PeriodicTimer::new(dur, dur)
                .for_each(move |_| {
                    unsafe { &mut *inner.as_ref().get() }.tick();
                    Ok(())
                })
                .into_task(),
        );
        wheel
    }

    #[inline]
    pub fn schedule(&self, dur: Duration, task: TaskId) -> TimerId {
        self.as_mut_inner().schedule(dur, task)
    }

    #[inline]
    pub fn reschedule(&self, dur: Duration, timer_id: TimerId) {
        self.as_mut_inner().reschedule(dur, None, timer_id)
    }

    #[inline]
    pub fn cancel(&self, timer_id: TimerId) {
        self.as_mut_inner().cancel(timer_id);
    }

    #[inline]
    pub fn is_expired(&self, timer_id: TimerId) -> bool {
        self.as_inner().is_expired(timer_id)
    }

    #[inline]
    fn as_mut_inner(&self) -> &mut Inner {
        unsafe { &mut *(&self.inner).get() }
    }

    #[inline]
    fn as_inner(&self) -> &Inner {
        unsafe { &*(&self.inner).get() }
    }
}

#[derive(Debug, Clone, Copy)]
enum TimerState {
    Unscheduled(Duration),
    Scheduled(TimerId),
    Expired,
    Cancelled,
}

#[derive(Debug)]
pub(super) struct Timer {
    state: TimerState,
}

impl Timer {
    #[inline]
    pub fn new(secs: u64) -> Self {
        let state = if secs == 0 {
            TimerState::Expired
        } else {
            TimerState::Unscheduled(Duration::from_secs(secs))
        };
        Timer { state }
    }

    #[inline]
    pub fn poll(&mut self) -> Poll<(), ()> {
        match self.state {
            TimerState::Scheduled(timer_id) => {
                match super::wt_is_expired(timer_id) {
                    true => Ok(Async::Ready(())),
                    false => Ok(Async::NotReady),
                }
            }
            TimerState::Unscheduled(dur) => {
                let timer_id = super::wt_schedule(dur);
                self.state = TimerState::Scheduled(timer_id);
                Ok(Async::NotReady)
            }
            TimerState::Expired => Ok(Async::Ready(())),
            TimerState::Cancelled => ::unreachable(),
        }
    }

    #[inline]
    pub fn reschedule(&mut self, secs: u64) -> bool {
        match self.state {
            TimerState::Scheduled(timer_id) => {
                super::wt_reschedule(Duration::from_secs(secs), timer_id);
                true
            }
            TimerState::Unscheduled(dur) => {
                let timer_id = super::wt_schedule(dur);
                self.state = TimerState::Scheduled(timer_id);
                true
            }
            TimerState::Expired => false,
            TimerState::Cancelled => ::unreachable(),
        }
    }
}

impl Drop for Timer {
    fn drop(&mut self) {
        if let TimerState::Scheduled(timer_id) = self.state {
            super::wt_cancel(timer_id);
            self.state = TimerState::Cancelled;
        }
    }
}
