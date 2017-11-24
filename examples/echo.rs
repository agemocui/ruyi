#[macro_use]
extern crate log;

extern crate env_logger;

extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::io;
use std::thread;

use futures::{Future, Sink, Stream};
use ruyi::channel::spsc;
use ruyi::net::{TcpListener, TcpStream};
use ruyi::net::tcp::split;
use ruyi::{reactor, IntoTask, Task};

fn echo(stream: TcpStream) -> io::Result<Task> {
    // Disable Nagle's algorithm
    stream.set_nodelay(true)?;

    let (r, w) = split(stream)?;

    // Send whatever is received back to client
    let task = w.send_all(r).map_err(|e| error!("{:?}", e)).into_task();
    Ok(task)
}

#[inline]
fn run(rx: spsc::Receiver<TcpStream>) -> io::Result<()> {
    reactor::run(rx.recv()?.for_each(|s| Ok(reactor::spawn(echo(s)?))))
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

    ruyi::net::init();

    // Number of workers to create
    let n = num_cpus::get();
    let mask = n - 1;
    let mut workers = Vec::with_capacity(n);
    for _ in 0..n {
        // Create a spsc queue to send accepted sockets
        // to the corresponding IO thread
        let (tx, rx) = spsc::sync_channel(512).unwrap();

        // Start an IO worker
        thread::spawn(move || run(rx).unwrap());
        workers.push(tx);
    }

    let mut i = 0;

    // Build a TCP acceptor
    let acceptor = TcpListener::builder()
        .port(10007)
        .build()
        .unwrap()
        .incoming()
        .unwrap()
        .for_each(|(sock, _)| {
            // Dispatch sockets to IO workers in a round-robin manner
            workers[i].try_send(sock).unwrap();
            i = (i + 1) & mask;
            Ok(())
        });

    // Run acceptor
    reactor::run(acceptor).unwrap();
}
