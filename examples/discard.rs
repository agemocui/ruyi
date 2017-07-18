#[macro_use]
extern crate log;
extern crate env_logger;

extern crate num_cpus;
extern crate futures;
extern crate ruyi;

use std::thread;

use futures::{Future, Stream};

use ruyi::stream::IntoStream;
use ruyi::reactor::{IntoTask, Task};
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct Discard;

impl Handler for Discard {
    fn handle(&mut self, session: Session) -> Task {
        session
            .into_stream()
            .for_each(|data| {
                debug!("Discard:\n{}", data.as_hex_dump());
                Ok(())
            })
            .map_err(|e| error!("{}", e))
            .into_task()
    }
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

    let n = num_cpus::get();
    match tcp::Server::with_handler(Discard)
        .port(10009)
        .num_of_workers(n)
        .start() {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
