#[macro_use]
extern crate log;
extern crate env_logger;

extern crate num_cpus;
extern crate futures;
extern crate ruyi;

use std::thread;

use futures::{future, Future, Stream};
use ruyi::channel::spsc;
use ruyi::io;
use ruyi::net::{TcpStream, TcpListener};
use ruyi::reactor::{self, IntoStream, IntoTask};

fn echo(sock: TcpStream) {
    // Disable Nagle's algorithm
    let task = future::result(sock.set_nodelay(true))
        .and_then(|_| {
            let (r, w) = io::split(sock);
            // Send whatever received back to client
            io::copy(r, w)
        })
        .map_err(|e| error!("{}", e))
        .into_task();
    reactor::spawn(task);
}

fn main() {
    // Initialize logger
    env_logger::init().unwrap();

    // Get number of cpu cores to create same quantity of IO workers
    let n = num_cpus::get();
    let mask = n - 1;
    let mut workers = Vec::with_capacity(n);
    for _ in 0..n {
        // Create a spsc queue to send accepted sockets
        // to the corresponding IO thread
        let (tx, rx) = spsc::sync_channel(512).unwrap();

        // Start an IO worker
        thread::spawn(move || {
            reactor::run(rx.into_stream().for_each(|s| Ok(echo(s)))).unwrap()
        });
        workers.push(tx);
    }

    let mut i = 0;

    // Build a TCP acceptor
    let acceptor = TcpListener::builder()
        .port(10007)
        .build()
        .unwrap()
        .incoming()
        .for_each(|(sock, _)| {
            // Dispatch sockets to IO workers in a round-robin manner
            workers[i].try_send(sock).unwrap();
            i = (i + 1) & mask;
            Ok(())
        });

    // Run acceptor
    reactor::run(acceptor).unwrap();
}
