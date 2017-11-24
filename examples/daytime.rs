#[macro_use]
extern crate log;

extern crate env_logger;

extern crate chrono;
extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::io;
use std::thread;

use chrono::prelude::Local;
use futures::Future;

use ruyi::{IntoTask, Task};
use ruyi::buf::ByteBuf;
use ruyi::net::tcp::send;
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct DayTime;

impl DayTime {
    #[inline]
    fn task(session: Session) -> io::Result<Task> {
        session.as_ref().set_nodelay(true)?;
        let time = Local::now().to_rfc2822() + "\r\n";
        let response = ByteBuf::from(time.into_bytes());
        let task = send(session, response)?
            .map_err(|e| error!("{}", e))
            .into_task();
        Ok(task)
    }
}

impl Handler for DayTime {
    fn handle(&mut self, session: Session) -> Option<Task> {
        match Self::task(session) {
            Ok(t) => Some(t),
            Err(e) => {
                error!("{}", e);
                None
            }
        }
    }
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

    ruyi::net::init();

    let n = num_cpus::get();
    match tcp::Server::with_handler(DayTime)
        .port(10013)
        .num_of_workers(n)
        .start()
    {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
