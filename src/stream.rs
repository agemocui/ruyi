use futures::Stream;

pub trait Timeout {
    type Stream: Stream;

    fn timeout(self, secs: u64) -> Self::Stream;
}
