#[macro_use]
extern crate log;
extern crate env_logger;

extern crate num_cpus;
extern crate futures;
extern crate ruyi;

use std::thread;

use futures::{future, Future};

use ruyi::io;
use ruyi::reactor::{IntoTask, Task};
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct Echo;

impl Handler for Echo {
    fn handle(&mut self, session: Session) -> Task {
        future::result(session.set_nodelay(true))
            .and_then(|_| {
                let (r, w) = io::split(session);
                io::copy(r, w)
            })
            .map_err(|e| error!("{}", e))
            .into_task()
    }
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

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
