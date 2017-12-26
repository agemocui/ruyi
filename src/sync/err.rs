use std::error;
use std::fmt;
use std::io;

pub enum SendError<T> {
    Io(io::Error),
    Disconnected(T),
}

pub enum TrySendError<T> {
    Io(io::Error),
    Full(T),
    Disconnected(T),
}

#[derive(Debug)]
pub enum RecvError {
    Io(io::Error),
    Disconnected,
}

#[derive(Debug)]
pub enum TryRecvError {
    Io(io::Error),
    Empty,
    Disconnected,
}

impl<T> From<io::Error> for SendError<T> {
    fn from(e: io::Error) -> Self {
        SendError::Io(e)
    }
}

impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SendError::Io(ref e) => write!(f, "Io({:?})", e),
            SendError::Disconnected(..) => write!(f, "Disconnected(..)"),
        }
    }
}

impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SendError::Io(..) => write!(f, "io error on sending"),
            SendError::Disconnected(..) => write!(f, "sending on a closed channel"),
        }
    }
}

impl<T: Send> error::Error for SendError<T> {
    fn description(&self) -> &str {
        match *self {
            SendError::Io(..) => "io error on sending",
            SendError::Disconnected(..) => "sending on a closed channel",
        }
    }
}

impl<T> From<io::Error> for TrySendError<T> {
    fn from(e: io::Error) -> Self {
        TrySendError::Io(e)
    }
}

impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TrySendError::Io(ref e) => write!(f, "Io({:?}", e),
            TrySendError::Full(..) => write!(f, "Full(..)"),
            TrySendError::Disconnected(..) => write!(f, "Disconnected(..)"),
        }
    }
}

impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TrySendError::Io(..) => write!(f, "io error on sending"),
            TrySendError::Full(..) => write!(f, "sending on a full channel"),
            TrySendError::Disconnected(..) => write!(f, "sending on a closed channel"),
        }
    }
}

impl<T: Send> error::Error for TrySendError<T> {
    fn description(&self) -> &str {
        match *self {
            TrySendError::Io(..) => "io error on sending",
            TrySendError::Full(..) => "sending on a full channel",
            TrySendError::Disconnected(..) => "sending on a closed channel",
        }
    }
}
