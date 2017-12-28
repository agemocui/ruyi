extern crate structopt;
#[macro_use]
extern crate structopt_derive;

extern crate env_logger;
#[macro_use]
extern crate log;

extern crate chrono;
extern crate futures;
extern crate num_cpus;
extern crate ruyi;

use std::env;
use std::mem;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use chrono::prelude::Utc;
use env_logger::LogBuilder;
use futures::{stream, Future, Sink, Stream};
use structopt::StructOpt;

use ruyi::buf::ByteBuf;
use ruyi::sync::spsc;
use ruyi::net::TcpStream;
use ruyi::net::tcp::connect;
use ruyi::reactor::{self, Timer};
use ruyi::{IntoTask, Task};

#[derive(StructOpt, Debug)]
#[structopt(name = "pingpong_client", about = "Ping-pong client.")]
struct Conf {
    #[structopt(short = "t", long = "threads", help = "Number of threads", default_value = "0")]
    threads: usize,

    #[structopt(short = "b", long = "bytes", help = "Number of bytes to send",
                default_value = "16384")]
    bytes: usize,

    #[structopt(short = "c", long = "connections", help = "Concurrent connections per thread",
                default_value = "25")]
    conns: usize,

    #[structopt(short = "s", long = "seconds", help = "Seconds to run", default_value = "60")]
    secs: usize,

    #[structopt(help = "Server IP to connect to", default_value = "127.0.0.1")]
    host: Option<IpAddr>,

    #[structopt(help = "Server port to connect to", default_value = "10007")] port: Option<u16>,
}

struct Vars {
    msgs: usize,
    bytes: usize,
}

fn ping_pong(
    addr: &SocketAddr,
    len: usize,
    vars: &'static mut Vars,
    conns: &'static mut usize,
) -> Task {
    let n = *conns;
    connect::<TcpStream>(addr)
        .and_then(move |s| {
            *conns -= 1;
            if *conns == 0 {
                info!("All {} connections are established", n);
            }
            s.as_ref().set_nodelay(true)?;
            Ok(s)
        })
        .and_then(move |s| {
            let (r, w) = s.into_twoway();
            let mut data = Vec::<u8>::with_capacity(len);
            unsafe { data.set_len(len) };
            w.send_all(
                stream::once(Ok(ByteBuf::from(data))).chain(r.filter(move |b| {
                    vars.msgs += 1;
                    vars.bytes += b.len();
                    true
                })),
            )
        })
        .map_err(|e| error!("{}", e))
        .into_task()
}

fn run(conf: &Conf) {
    info!("Start - {:?}", conf);
    let addr = SocketAddr::new(conf.host.unwrap(), conf.port.unwrap());
    ruyi::net::init();
    let timer = Timer::new(Duration::from_secs(conf.secs as u64));
    let mut threads = Vec::with_capacity(conf.threads);
    let total_msgs = Arc::new(AtomicUsize::new(0));
    let total_bytes = Arc::new(AtomicUsize::new(0));
    for _ in 0..conf.threads {
        let (tx, rx) = spsc::sync_channel(1).unwrap();
        let total_msgs = total_msgs.clone();
        let total_bytes = total_bytes.clone();
        let conns = conf.conns;
        let handle = thread::spawn(move || {
            let mut vars = Vars { msgs: 0, bytes: 0 };
            {
                let mut n = conns;
                let task = rx.recv().unwrap().for_each(|(addr, bytes)| {
                    for _ in 0..conns {
                        let s_vars: &'static mut Vars = unsafe { mem::transmute(&mut vars) };
                        let s_conns: &'static mut usize = unsafe { mem::transmute(&mut n) };
                        reactor::spawn(ping_pong(&addr, bytes, s_vars, s_conns));
                    }
                    Ok(())
                });
                reactor::run(task).unwrap();
            }
            total_msgs.as_ref().fetch_add(vars.msgs, Ordering::Relaxed);
            total_bytes
                .as_ref()
                .fetch_add(vars.bytes, Ordering::Relaxed);
        });
        tx.send((addr, conf.bytes)).unwrap();
        threads.push((Some(handle), Some(tx)));
    }
    reactor::run(timer).unwrap();
    for thread in threads.iter_mut() {
        thread.1 = None;
    }
    for thread in threads.iter_mut() {
        thread.0.take().unwrap().join().unwrap();
    }
    let bytes = total_bytes.as_ref().load(Ordering::Relaxed);
    let msgs = total_msgs.as_ref().load(Ordering::Relaxed);
    info!("Total bytes read: {}", bytes);
    info!("Total messages read: {}", msgs);
    info!("Average message size: {}", bytes as f64 / msgs as f64);
    info!(
        "Throughput: {} MiB/s",
        bytes as f64 / (conf.secs * 1024 * 1024) as f64
    );
}

fn main() {
    let mut conf = Conf::from_args();

    let mut builder = LogBuilder::new();
    builder.format(|r| {
        format!(
            "{} {:<5} {}",
            Utc::now().format("%Y-%m-%d %H:%M:%S.%f"),
            r.level(),
            r.args()
        )
    });
    if let Ok(v) = env::var("RUST_LOG") {
        builder.parse(&v);
    }
    builder.init().unwrap();

    if conf.threads < 1 {
        conf.threads = num_cpus::get();
    }

    run(&conf);
}
