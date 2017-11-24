use futures::Future;

pub trait Timeout {
    type Future: Future;

    fn timeout(self, secs: u64) -> Self::Future;
}
