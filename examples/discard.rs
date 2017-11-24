extern crate env_logger;
#[macro_use]
extern crate log;

extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::io;
use std::thread;

use futures::{Future, Stream};

use ruyi::{IntoTask, Task};
use ruyi::net::tcp::recv;
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct Discard;

impl Discard {
    #[inline]
    fn task(session: Session) -> io::Result<Task> {
        let task = recv(session)?
            .for_each(|data| {
                debug!("Discard:\n{}", data.as_hex_dump());
                Ok(())
            })
            .map_err(|e| error!("{}", e))
            .into_task();
        Ok(task)
    }
}

impl Handler for Discard {
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
    match tcp::Server::with_handler(Discard)
        .port(10009)
        .num_of_workers(n)
        .start()
    {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
