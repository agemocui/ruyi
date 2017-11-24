extern crate env_logger;
#[macro_use]
extern crate log;

extern crate chrono;
extern crate futures;
extern crate getopts;
extern crate num_cpus;
extern crate ruyi;

use std::env;
use std::mem;
use std::net::SocketAddr;
use std::path::Path;
use std::process;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::Duration;

use chrono::prelude::Utc;
use futures::{stream, Future, IntoFuture, Sink, Stream};
use env_logger::LogBuilder;

use ruyi::buf::ByteBuf;
use ruyi::channel::spsc;
use ruyi::net::TcpStream;
use ruyi::net::tcp::connect;
use ruyi::reactor::{self, Timer};
use ruyi::{IntoTask, Task};

#[derive(Debug, Clone, Copy)]
struct Conf {
    threads: usize,
    bytes: usize,
    connections: usize,
    seconds: u64,
    addr: SocketAddr,
}

impl Default for Conf {
    fn default() -> Self {
        Conf {
            threads: num_cpus::get(),
            bytes: 16_384,
            connections: 100,
            seconds: 60,
            addr: "127.0.0.1:10007".parse().unwrap(),
        }
    }
}

fn print_usage(program: &str) {
    println!("Usage: {} [OPTION...] [HOST [PORT]]", program);
    println!();
    println!("    Ping-pong client");
    println!();
    println!("Host:");
    println!("    Server IP to connect to. Default to 127.0.0.1");
    println!();
    println!("Port:");
    println!("    Server port to connect to. Default to 10007");
    println!();
    println!("Options:");
    println!("    -h, --help                        Print this help and exit.");
    println!("    -t<num>, --threads=<num>          Number of threads.");
    println!("    -b<num>, --bytes=<num>            Number of bytes to send.");
    println!("    -c<num>, --connections=<num>      Concurrent connections per thread.");
    println!("    -s<secs>, --seconds=<secs>        Seconds to run.");
    println!();
}

fn process_command_line() -> Result<Option<Conf>, String> {
    let mut opts = getopts::Options::new();
    opts.optflag("h", "help", "");
    opts.optopt("t", "threads", "", "");
    opts.optopt("b", "bytes", "", "");
    opts.optopt("c", "connections", "", "");
    opts.optopt("s", "seconds", "", "");

    let args: Vec<String> = env::args().collect();
    let matches = opts.parse(&args[1..])
        .map_err(|e| format!("Failed to parse command options: {}", e))?;
    if matches.opt_present("h") {
        let path = Path::new(&args[0]);
        let program = path.file_stem().unwrap().to_string_lossy();
        print_usage(&program);
        return Ok(None);
    }

    let mut conf = Conf::default();
    if matches.free.len() > 0 {
        let ip = matches.free[0]
            .parse()
            .map_err(|e| format!("Failed to parse HOST: {}", e))?;
        conf.addr.set_ip(ip);
    }
    if matches.free.len() > 1 {
        let port = matches.free[1]
            .parse()
            .map_err(|e| format!("Failed to parse PORT: {}", e))?;
        conf.addr.set_port(port);
    }

    if let Some(t) = matches.opt_str("t") {
        let threads = t.parse().unwrap();
        if threads > 0 {
            conf.threads = threads;
        }
    }
    if let Some(b) = matches.opt_str("b") {
        let bytes = b.parse().unwrap();
        if bytes > 0 {
            conf.bytes = bytes;
        }
    }
    if let Some(c) = matches.opt_str("c") {
        let connections = c.parse().unwrap();
        if connections > 0 {
            conf.connections = connections;
        }
    }
    if let Some(s) = matches.opt_str("s") {
        let seconds = s.parse().unwrap();
        if seconds > 0 {
            conf.seconds = seconds;
        }
    }

    Ok(Some(conf))
}

struct Vars {
    msgs: usize,
    bytes: usize,
    conns: usize,
    count: Arc<AtomicUsize>,
}

fn ping_pong(addr: &SocketAddr, len: usize, vars: &'static mut Vars) -> Task {
    connect::<TcpStream>(addr)
        .and_then(move |s| {
            vars.conns -= 1;
            if vars.conns == 0 {
                if vars.count.as_ref().fetch_sub(1, Ordering::Relaxed) == 1 {
                    info!("All connected");
                }
            }
            s.as_ref()
                .set_nodelay(true)
                .into_future()
                .and_then(move |_| {
                    let (r, w) = s.into_2way();
                    let mut data = Vec::<u8>::with_capacity(len);
                    unsafe { data.set_len(len) };
                    w.send_all(stream::once(Ok(ByteBuf::from(data))).chain(
                        r.filter(move |b| {
                            vars.msgs += 1;
                            vars.bytes += b.len();
                            true
                        }),
                    ))
                })
        })
        .map_err(|e| error!("{}", e))
        .into_task()
}

fn run(conf: Conf) {
    info!("Start - {:?}", conf);
    ruyi::net::init();
    let timer = Timer::new(Duration::from_secs(conf.seconds));
    let mut threads = Vec::with_capacity(conf.threads);
    let total_msgs = Arc::new(AtomicUsize::new(0));
    let total_bytes = Arc::new(AtomicUsize::new(0));
    let n = Arc::new(AtomicUsize::new(conf.threads));
    for _ in 0..conf.threads {
        let (tx, rx) = spsc::sync_channel(1).unwrap();
        let total_msgs = total_msgs.clone();
        let total_bytes = total_bytes.clone();
        let conns = conf.connections;
        let n = n.clone();
        let handle = thread::spawn(move || {
            let mut vars = Vars {
                msgs: 0,
                bytes: 0,
                conns,
                count: n,
            };
            {
                let task = rx.recv().unwrap().for_each(|(addr, bytes)| {
                    for _ in 0..conns {
                        let s_vars: &'static mut Vars = unsafe { mem::transmute(&mut vars) };
                        reactor::spawn(ping_pong(&addr, bytes, s_vars));
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
        tx.send((conf.addr, conf.bytes)).unwrap();
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
        bytes as f64 / (conf.seconds * 1024 * 1024) as f64
    );
}

#[inline]
fn exit(err_msg: &str) -> ! {
    if !err_msg.is_empty() {
        error!("{}", err_msg);
    }
    process::exit(1)
}

fn main() {
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

    match process_command_line() {
        Ok(Some(conf)) => run(conf),
        Ok(None) => {}
        Err(e) => exit(&e),
    }
}
