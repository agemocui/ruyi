use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread::JoinHandle;

use channel::err::SendError;
use channel::spsc::SyncSender;
use net::TcpStream;

pub struct Worker {
    tx: Option<SyncSender<TcpStream>>,
    join_handle: Option<JoinHandle<()>>,
    conn_count: Arc<AtomicUsize>,
}

impl Worker {
    #[inline]
    pub fn new(
        tx: SyncSender<TcpStream>,
        join_handle: JoinHandle<()>,
        conn_count: Arc<AtomicUsize>,
    ) -> Self {
        Worker {
            tx: Some(tx),
            join_handle: Some(join_handle),
            conn_count,
        }
    }

    #[inline]
    pub fn send(&self, conn: TcpStream) -> Result<(), SendError<TcpStream>> {
        match self.tx.as_ref() {
            Some(tx) => tx.send(conn),
            None => ::unreachable(),
        }
    }

    #[inline]
    pub fn conn_count(&self) -> usize {
        self.conn_count.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn inc_conn_count(&self) {
        self.conn_count.fetch_add(1, Ordering::Relaxed);
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.tx = None;
        if let Some(join_handle) = self.join_handle.take() {
            join_handle.join().ok();
        }
    }
}
