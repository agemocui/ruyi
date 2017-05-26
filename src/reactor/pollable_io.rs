use std::io;

use super::super::nio::{Pollable, Ops};

#[derive(Debug)]
pub struct PollableIo<P: Pollable> {
    io: P,
    interested_ops: Ops,
    sched_idx: Option<usize>,
}

impl<P: Pollable> PollableIo<P> {
    #[inline]
    pub fn new(io: P) -> Self {
        PollableIo {
            io,
            interested_ops: Ops::empty(),
            sched_idx: None,
        }
    }

    #[inline]
    pub fn get_ref(&self) -> &P {
        &self.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut P {
        &mut self.io
    }

    #[inline]
    pub fn need_read(&mut self) -> io::Result<()> {
        let ops = self.interested_ops() | Ops::read();
        self.interest_ops(ops)
    }

    #[inline]
    pub fn need_write(&mut self) -> io::Result<()> {
        let ops = self.interested_ops() | Ops::write();
        self.interest_ops(ops)
    }

    #[inline]
    pub fn no_need_read(&mut self) -> io::Result<()> {
        let ops = self.interested_ops() - Ops::read();
        self.interest_ops(ops)
    }

    #[inline]
    pub fn no_need_write(&mut self) -> io::Result<()> {
        let ops = self.interested_ops() - Ops::write();
        self.interest_ops(ops)
    }

    pub fn is_readable(&self) -> bool {
        match self.interested_ops().contains_read() {
            true => {
                match self.sched_idx {
                    Some(idx) => super::is_readable(idx),
                    None => true,
                }
            }
            false => true,
        }
    }

    pub fn is_writable(&self) -> bool {
        match self.interested_ops().contains_write() {
            true => {
                match self.sched_idx {
                    Some(idx) => super::is_writable(idx),
                    None => true,
                }
            }
            false => true,
        }
    }

    #[inline]
    fn interested_ops(&self) -> Ops {
        self.interested_ops
    }

    fn interest_ops(&mut self, ops: Ops) -> io::Result<()> {
        if let Some(sched_idx) = self.sched_idx {
            if self.interested_ops != ops {
                let sched_io_ops = ops - self.interested_ops;
                super::reregister_io::<P, _>(&self.io, ops, sched_idx, sched_io_ops)?;
                self.interested_ops = ops;
            }
        } else {
            self.sched_idx = Some(super::register_io::<P, _>(&self.io, ops)?);
        }
        Ok(())
    }
}

impl<P: Pollable> Drop for PollableIo<P> {
    fn drop(&mut self) {
        if let Some(sched_idx) = self.sched_idx.take() {
            super::deregister_io::<P, _>(&self.io, sched_idx)
                .unwrap_or_else(|e| error!("Failed to deregister {:?}: {}", self.io, e));
        }
    }
}
