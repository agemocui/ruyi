use futures::Stream;

pub trait IntoStream {
    type Stream: Stream;

    fn into_stream(self) -> Self::Stream;
}

pub trait Timeout {
    type Stream: Stream;

    fn timeout(self, secs: u64) -> Self::Stream;
}
