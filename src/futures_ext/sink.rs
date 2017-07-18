use futures::Sink;

pub trait IntoSink {
    type Sink: Sink;

    fn into_sink(self) -> Self::Sink;
}
