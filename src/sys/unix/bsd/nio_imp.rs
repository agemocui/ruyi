use std::fmt;
use std::io;
use std::marker::PhantomData;
use std::os::unix::io::AsRawFd;

use reactor::CURRENT_LOOP;
use sys::{ReadyTasks, Schedule, Token};
use sys::unix::poll::{Event, Filter};

pub struct Nio<H, T = H>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    io: T,
    token: Token,
    read_sched: bool,
    write_sched: bool,
    _marker: PhantomData<H>,
}

impl<H, T> Nio<H, T>
where
    H: AsRawFd + fmt::Debug,
    T: AsRef<H>,
{
    #[inline]
    pub fn try_from(io: T) -> io::Result<Self> {
        Ok(Nio {
            io,
            token: Self::register(),
            read_sched: false,
            write_sched: false,
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
        if self.read_sched {
            Ok(())
        } else {
            self.read_sched = true;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop
                    .as_poller()
                    .register(self.io.as_ref().as_raw_fd(), Filter::Read, self.token)?;
                eloop.schedule_read(self.token);
                Ok(())
            })
        }
    }

    #[inline]
    pub fn schedule_write(&mut self) -> io::Result<()> {
        if self.write_sched {
            Ok(())
        } else {
            self.write_sched = true;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop.as_poller().register(
                    self.io.as_ref().as_raw_fd(),
                    Filter::Write,
                    self.token,
                )?;
                eloop.schedule_write(self.token);
                Ok(())
            })
        }
    }

    #[inline]
    pub fn cancel_write(&mut self) -> io::Result<()> {
        if self.write_sched {
            self.write_sched = false;
            CURRENT_LOOP.with(|current_loop| {
                let eloop = unsafe { current_loop.as_mut() }.as_mut_inner();
                eloop.as_poller().deregister(
                    self.io.as_ref().as_raw_fd(),
                    Filter::Write,
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
        match self.read_sched {
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
        match self.write_sched {
            true => CURRENT_LOOP.with(|current_loop| {
                let eloop = current_loop.as_inner();
                let schedule = unsafe { eloop.get_schedule_unchecked(self.token) };
                schedule.is_write_ready()
            }),
            false => true,
        }
    }

    #[inline]
    fn register() -> Token {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() };
            eloop.as_mut_inner().schedule()
        })
    }

    #[inline]
    fn deregister(token: Token) {
        CURRENT_LOOP.with(|current_loop| {
            let eloop = unsafe { current_loop.as_mut() };
            eloop.as_mut_inner().cancel(token);
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
            "Nio {{ io: {:?}, token: {:?}, read_sched: {}, write_sched: {} }}",
            self.io.as_ref(),
            self.token,
            self.read_sched,
            self.write_sched,
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
        Self::deregister(self.token);
    }
}

#[inline]
pub(in sys) fn get_ready_tasks(event: &Event, schedule: &mut Schedule) -> ReadyTasks {
    match event.is_read() {
        true => {
            schedule.set_read_ready(true);
            ReadyTasks::Single(schedule.read_task())
        }
        false => {
            schedule.set_write_ready(true);
            ReadyTasks::Single(schedule.write_task())
        }
    }
}
