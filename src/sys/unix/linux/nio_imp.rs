use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::{AsRawFd, RawFd};

use reactor::CURRENT_LOOP;
use task::TaskId;
use sys::{ReadyTasks, Schedule, Token};
use sys::unix::linux::poll::{Event, Ops};

pub struct Nio<H, T = H>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    io: T,
    token: Token,
    sched_ops: Ops,
    _marker: PhantomData<H>,
}

impl<H, T> Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        let token = Self::register(io.as_ref().as_raw_fd())?;
        Ok(Nio {
            io,
            token,
            sched_ops: Ops::empty(),
            _marker: PhantomData,
        })
    }

    #[inline]
    pub fn get_ref(&self) -> &T {
        &self.io
    }

    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.io
    }

    #[inline]
    pub fn schedule_read(&mut self) -> io::Result<()> {
        if self.sched_ops.contains(Ops::READ) {
            Ok(())
        } else {
            self.sched_ops |= Ops::READ;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop.as_poller().reregister(
                    self.io.as_ref().as_raw_fd(),
                    self.sched_ops,
                    self.token,
                )?;
                eloop.schedule_read(self.token);
                Ok(())
            })
        }
    }

    #[inline]
    pub fn schedule_write(&mut self) -> io::Result<()> {
        if self.sched_ops.contains(Ops::WRITE) {
            Ok(())
        } else {
            self.sched_ops |= Ops::WRITE;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop.as_poller().reregister(
                    self.io.as_ref().as_raw_fd(),
                    self.sched_ops,
                    self.token,
                )?;
                eloop.schedule_write(self.token);
                Ok(())
            })
        }
    }

    #[inline]
    pub fn cancel_write(&mut self) -> io::Result<()> {
        if self.sched_ops.contains(Ops::WRITE) {
            self.sched_ops -= Ops::WRITE;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop.as_poller().reregister(
                    self.io.as_ref().as_raw_fd(),
                    self.sched_ops,
                    self.token,
                )?;
                eloop.cancel_write(self.token);
                Ok(())
            })
        } else {
            Ok(())
        }
    }

    #[inline]
    pub fn is_read_ready(&self) -> bool {
        match self.sched_ops.contains(Ops::READ) {
            true => CURRENT_LOOP.with(|current_loop| {
                let eloop = current_loop.as_inner();
                let schedule = unsafe { eloop.get_schedule_unchecked(self.token) };
                schedule.is_read_ready()
            }),
            false => true,
        }
    }

    #[inline]
    pub fn is_write_ready(&self) -> bool {
        match self.sched_ops.contains(Ops::WRITE) {
            true => CURRENT_LOOP.with(|current_loop| {
                let eloop = current_loop.as_inner();
                let schedule = unsafe { eloop.get_schedule_unchecked(self.token) };
                schedule.is_write_ready()
            }),
            false => true,
        }
    }

    #[inline]
    fn register(fd: RawFd) -> io::Result<Token> {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            let token = eloop.schedule();
            match eloop.as_poller().register(fd, Ops::empty(), token) {
                Ok(()) => Ok(token),
                Err(e) => {
                    eloop.cancel(token);
                    Err(e)
                }
            }
        })
    }

    #[inline]
    fn deregister(fd: RawFd, token: Token) -> io::Result<()> {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
            eloop.as_poller().deregister(fd)?;
            eloop.cancel(token);
            Ok(())
        })
    }
}

impl<H, T> fmt::Debug for Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Nio {{ io: {:?}, token: {:?}, interested_ops: {:?} }}",
            self.io.as_ref(),
            self.token,
            self.sched_ops
        )
    }
}

impl<H, T> Drop for Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    fn drop(&mut self) {
        Self::deregister(self.io.as_ref().as_raw_fd(), self.token)
            .unwrap_or_else(|e| error!("Failed to deregister {:?}: {}", self.io.as_ref(), e));
    }
}

#[inline]
pub(in sys) fn get_ready_tasks(event: &Event, schedule: &mut Schedule) -> ReadyTasks {
    match event.is_read() {
        true => match event.is_write() {
            true => {
                schedule.set_read_ready(true);
                schedule.set_write_ready(true);
                ReadyTasks::Pair(schedule.read_task(), schedule.write_task())
            }
            false => {
                schedule.set_read_ready(true);
                ReadyTasks::Single(schedule.read_task())
            }
        },
        false => {
            schedule.set_write_ready(true);
            ReadyTasks::Single(schedule.write_task())
        }
    }
}
