#[macro_use]
extern crate log;
extern crate env_logger;

extern crate num_cpus;
extern crate chrono;
extern crate futures;
extern crate ruyi;

use std::thread;

use chrono::prelude::Local;
use futures::{future, Sink, Future};

use ruyi::buf::ByteBuf;
use ruyi::sink::IntoSink;
use ruyi::reactor::{IntoTask, Task};
use ruyi::service::tcp::{self, Handler, Session};

#[derive(Clone)]
struct DayTime;

impl Handler for DayTime {
    fn handle(&mut self, session: Session) -> Task {
        future::result(session.set_nodelay(true))
            .and_then(|_| {
                let time = Local::now().to_rfc2822() + "\r\n";
                let response = ByteBuf::from(time.into_bytes());
                session.into_sink().send(response)
            })
            .map_err(|e| error!("{}", e))
            .into_task()
    }
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

    let n = num_cpus::get();
    match tcp::Server::with_handler(DayTime)
        .port(10013)
        .num_of_workers(n)
        .start() {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
