use std::io;
use std::fmt;

use super::super::nio::{Pollable, Ops};

#[derive(Debug)]
pub struct PollableIo<P: Pollable + fmt::Debug> {
    io: P,
    interested_ops: Ops,
    sched_idx: Option<usize>,
}

#[inline]
pub fn new<P>(io: P) -> PollableIo<P>
    where P: Pollable + fmt::Debug
{
    PollableIo {
        io,
        interested_ops: Ops::empty(),
        sched_idx: None,
    }
}

impl<P: Pollable + fmt::Debug> PollableIo<P> {
    #[inline]
    pub fn get_ref(&self) -> &P {
        &self.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut P {
        &mut self.io
    }

    #[inline]
    pub fn interested_ops(&self) -> Ops {
        self.interested_ops
    }

    pub fn interest_ops(&mut self, ops: Ops) -> io::Result<()> {
        if let Some(sched_idx) = self.sched_idx {
            if self.interested_ops != ops {
                let sched_io_ops = ops - self.interested_ops;
                super::reregister_io(&self.io, ops, sched_idx, sched_io_ops)?;
                self.interested_ops = ops;
            }
        } else {
            self.sched_idx = Some(super::register_io(&self.io, ops)?);
        }
        Ok(())
    }
}

impl<P: Pollable + fmt::Debug> Drop for PollableIo<P> {
    fn drop(&mut self) {
        if let Some(sched_idx) = self.sched_idx.take() {
            super::deregister_io(&self.io, sched_idx);
        }
    }
}
