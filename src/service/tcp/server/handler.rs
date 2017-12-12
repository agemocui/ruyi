use std::borrow::ToOwned;

use task::Task;
use service::tcp::server::Session;

pub trait Handler {
    fn handle(&mut self, session: Session) -> Option<Task>;
}

pub trait ToHandler {
    type Handler: Handler;

    fn to_handler(&self) -> Self::Handler;
}

impl<H> ToHandler for H
where
    H: Handler + ToOwned<Owned = Self>,
{
    type Handler = Self;

    #[inline]
    fn to_handler(&self) -> Self::Handler {
        self.to_owned()
    }
}
