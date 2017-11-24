extern crate env_logger;
#[macro_use]
extern crate log;

extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::io;
use std::thread;

use futures::{Future, Sink};

use ruyi::{IntoTask, Task};
use ruyi::net::tcp::split;
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct Echo;

impl Echo {
    #[inline]
    fn task(session: Session) -> io::Result<Task> {
        session.as_ref().set_nodelay(true)?;
        let (r, w) = split(session)?;
        let task = w.send_all(r).map_err(|e| error!("{}", e)).into_task();
        Ok(task)
    }
}

impl Handler for Echo {
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
    match tcp::Server::with_handler(Echo)
        .port(10007)
        .num_of_workers(n)
        .start()
    {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
