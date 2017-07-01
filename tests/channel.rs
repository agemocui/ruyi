extern crate ruyi;
extern crate futures;

use std::thread;
use std::time::Duration;

use futures::Stream;

use ruyi::channel::spsc;
use ruyi::reactor::{self, IntoStream};

#[test]
fn spsc() {
    let (tx, rx) = spsc::sync_channel::<usize>(1).unwrap();
    let handle = thread::spawn(move || {
        let task = rx.into_stream().for_each(|_| Ok(()));
        reactor::run(task).unwrap();
    });
    tx.send(1).unwrap();
    thread::sleep(Duration::from_millis(100));
    drop(tx);
    handle.join().unwrap();
}
