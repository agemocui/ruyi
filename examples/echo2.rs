extern crate structopt;
#[macro_use]
extern crate structopt_derive;

extern crate env_logger;
#[macro_use]
extern crate log;

extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::io;
use std::thread;

use futures::{Future, Sink};
use structopt::StructOpt;

use ruyi::{IntoTask, Task};
use ruyi::net::tcp::split;
use ruyi::service::tcp;
use ruyi::service::tcp::server::{Handler, Session};

#[derive(StructOpt, Debug)]
#[structopt(name = "echo2", about = "A program that implements Echo Protocol.")]
struct Opt {
    #[structopt(short = "p", long = "port", help = "Listening port to bind",
                default_value = "10007")]
    port: u16,
    #[structopt(short = "w", long = "workers", help = "Number of workers", default_value = "0")]
    workers: usize,
}

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
    let mut opt = Opt::from_args();

    // Initialize logger
    env_logger::init().unwrap();

    ruyi::net::init();

    if opt.workers < 1 {
        opt.workers = num_cpus::get();
    }
    match tcp::Server::with_handler(Echo)
        .port(opt.port)
        .num_of_workers(opt.workers)
        .start()
    {
        Ok(()) => thread::park(),
        Err(e) => error!("{}", e),
    }
}
