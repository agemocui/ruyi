mod session;
pub use self::session::*;

mod handler;
pub use self::handler::*;

mod worker;
use self::worker::Worker;

use std::fmt;
use std::io;
use std::mem;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::thread::{self, JoinHandle};

use futures::{Future, Stream};

use channel::err::SendError;
use channel::spsc::{self, SyncSender};
use net::{TcpListener, TcpListenerBuilder};
use reactor::{self, IntoStream, IntoTask};

struct Inner {
    name: String,
    listener: Option<TcpListener>,
    workers: Vec<Worker>,
    mask: usize,
    idx: usize,
    worker_conns: usize,
}

impl Inner {
    #[inline]
    fn init<H>(&mut self, to_handler: Arc<H>)
    where
        H: ToHandler + Send + Sync + 'static,
    {
        for _ in 0..self.mask + 1 {
            let (tx, rx) = spsc::sync_channel(self.worker_conns).unwrap();
            let conn_count = Arc::new(AtomicUsize::new(0));
            let join_handle = {
                let conn_count = conn_count.clone();
                let to_handler = to_handler.clone();
                thread::spawn(move || {
                    let mut handler = to_handler.to_handler();
                    let handle = rx.into_stream().for_each(|conn| {
                        let session = Session::new(conn, unsafe { mem::transmute(&conn_count) });
                        Ok(reactor::spawn(handler.handle(session)))
                    });
                    reactor::run(handle).map_err(|e| error!("{}", e)).ok();
                })
            };
            let worker = Worker::new(tx, join_handle, conn_count);
            self.workers.push(worker);
        }
    }

    #[inline]
    fn run(mut self) {
        info!("{} started", self);
        let accept = self.listener
            .take()
            .unwrap()
            .incoming()
            .for_each(move |(s, _)| {
                let mut idx = self.idx;
                loop {
                    let worker: &Worker = unsafe { self.workers.get_unchecked(idx) };
                    idx = (idx + 1) & self.mask;
                    if worker.conn_count() < self.worker_conns {
                        self.idx = idx;
                        worker.send(s).map_err(|e| error!("{:?}", e)).ok();
                        worker.inc_conn_count();
                        break;
                    }
                    if idx == self.idx {
                        warn!("{} drops {} not to exceed worker_conns", self, s);
                        break;
                    }
                }
                Ok(())
            })
            .map_err(|e| error!("{}", e))
            .into_task();
        reactor::spawn(accept);
    }
}

impl fmt::Display for Inner {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "TcpServer({})", self.name)
    }
}

impl Drop for Inner {
    #[inline]
    fn drop(&mut self) {
        info!("{} stopped", self);
    }
}

pub struct Server<H> {
    listener_builder: TcpListenerBuilder,
    num_of_workers: usize,
    worker_conns: usize, // Max number of simultaneous connections per worker
    tx: Option<SyncSender<(Inner, Arc<H>)>>,
    join_handle: Option<JoinHandle<()>>,
    to_handler: Arc<H>,
}

impl<H> Server<H>
where
    H: ToHandler + Send + Sync + 'static,
{
    #[inline]
    pub fn with_handler(to_handler: H) -> Self {
        Server {
            listener_builder: TcpListenerBuilder::default(),
            num_of_workers: 1,
            worker_conns: 512,
            tx: None,
            join_handle: None,
            to_handler: Arc::new(to_handler),
        }
    }

    #[inline]
    pub fn addr(&mut self, addr: SocketAddr) -> &mut Self {
        self.listener_builder.addr(addr);
        self
    }

    #[inline]
    pub fn port(&mut self, port: u16) -> &mut Self {
        self.listener_builder.port(port);
        self
    }

    #[inline]
    pub fn backlog(&mut self, blacklog: i32) -> &mut Self {
        self.listener_builder.backlog(blacklog);
        self
    }

    #[inline]
    pub fn ttl(&mut self, ttl: Option<u32>) -> &mut Self {
        self.listener_builder.ttl(ttl);
        self
    }

    #[inline]
    pub fn only_v6(&mut self, only_v6: Option<bool>) -> &mut Self {
        self.listener_builder.only_v6(only_v6);
        self
    }

    #[inline]
    pub fn num_of_workers(&mut self, num_of_workers: usize) -> &mut Self {
        if let Some(n) = num_of_workers.checked_next_power_of_two() {
            self.num_of_workers = n;
        }
        self
    }

    #[inline]
    pub fn worker_conns(&mut self, worker_conns: usize) -> &mut Self {
        if worker_conns > 0 {
            self.worker_conns = worker_conns;
        }
        self
    }

    pub fn start(&mut self) -> io::Result<()> {
        let listener = self.listener_builder.build()?;
        let name = format!("{}", listener.local_addr()?);
        let inner = Inner {
            name,
            listener: Some(listener),
            workers: Vec::with_capacity(self.num_of_workers),
            mask: self.num_of_workers - 1,
            idx: 0,
            worker_conns: self.worker_conns,
        };
        let (tx, rx) = spsc::sync_channel(1)?;
        match tx.send((inner, self.to_handler.clone())) {
            Ok(..) => {}
            Err(SendError::Io(e)) => return Err(e),
            Err(SendError::Disconnected(..)) => ::unreachable(),
        }
        let join_handle = thread::spawn(move || {
            let server = rx.into_stream().for_each(|(mut inner, h)| {
                inner.init(h);
                inner.run();
                Ok(())
            });
            reactor::run(server).map_err(|e| error!("{}", e)).ok();
        });
        self.tx = Some(tx);
        self.join_handle = Some(join_handle);
        Ok(())
    }
}

impl<H> Drop for Server<H> {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}
