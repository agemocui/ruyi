use std::fmt;
use std::io;
use std::ops;
use std::time::Duration;

use super::sys;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Ops {
    bits: usize,
}

impl Ops {
    #[inline]
    fn from(bits: usize) -> Self {
        Ops { bits: bits }
    }

    #[inline]
    pub fn empty() -> Self {
        Self::from(0)
    }

    #[inline]
    pub fn read() -> Self {
        Self::from(sys::OP_READ)
    }

    #[inline]
    pub fn write() -> Self {
        Self::from(sys::OP_WRITE)
    }

    #[inline]
    pub fn all() -> Self {
        Self::from(sys::OP_READ | sys::OP_WRITE)
    }

    #[inline]
    pub fn insert(&mut self, ops: Self) {
        self.bits |= ops.bits
    }

    #[inline]
    pub fn remove(&mut self, ops: Self) {
        self.bits &= !ops.bits
    }

    #[inline]
    pub fn contains_read(self) -> bool {
        self.contains(Self::read())
    }

    #[inline]
    pub fn contains_write(self) -> bool {
        self.contains(Self::write())
    }

    #[inline]
    pub fn contains(self, other: Self) -> bool {
        (self.bits & other.bits) == other.bits
    }

    #[inline]
    fn bits(self) -> usize {
        self.bits
    }
}

impl From<Ops> for usize {
    #[inline]
    fn from(ops: Ops) -> Self {
        ops.bits()
    }
}

impl ops::BitOr for Ops {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self::Output {
        Ops::from(self.bits() | rhs.bits())
    }
}

impl ops::BitXor for Ops {
    type Output = Self;

    #[inline]
    fn bitxor(self, rhs: Self) -> Self::Output {
        Ops::from(self.bits() ^ rhs.bits())
    }
}

impl ops::BitAnd for Ops {
    type Output = Self;

    #[inline]
    fn bitand(self, rhs: Self) -> Self::Output {
        Ops::from(self.bits() & rhs.bits())
    }
}

impl ops::Sub for Ops {
    type Output = Self;

    #[inline]
    fn sub(self, rhs: Self) -> Self::Output {
        Ops::from(self.bits() & !rhs.bits())
    }
}

impl ops::Not for Ops {
    type Output = Self;

    #[inline]
    fn not(self) -> Self::Output {
        Ops::from(!self.bits())
    }
}

impl fmt::Debug for Ops {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut pipe = false;
        let ops = [(Self::read(), "read"), (Self::write(), "write")];
        write!(f, "Ops {{ ")?;
        for &(op, desc) in &ops {
            if self.contains(op) {
                if pipe {
                    write!(f, " | ")?;
                }
                write!(f, "{}", desc)?;
                pipe = true;
            }
        }
        write!(f, " }}")
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Token {
    val: usize,
}

impl From<usize> for Token {
    #[inline]
    fn from(val: usize) -> Self {
        Token { val: val }
    }
}

impl From<Token> for usize {
    #[inline]
    fn from(token: Token) -> Self {
        token.val
    }
}

pub trait Pollable: fmt::Debug {
    fn register(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()>;

    fn reregister(&self, poller: &Poller, interested_ops: Ops, token: Token) -> io::Result<()>;

    fn deregister(&self, poller: &Poller) -> io::Result<()>;
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Event {
    inner: sys::Event,
}

impl Event {
    #[inline]
    pub fn ready_ops(&self) -> Ops {
        Ops::from(self.inner.ops())
    }

    #[inline]
    pub fn token(&self) -> Token {
        Token::from(self.inner.token())
    }
}

impl fmt::Debug for Event {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Event {{ {:?}, {:?} }}", self.ready_ops(), self.token())?;
        Ok(())
    }
}

pub struct Poller {
    selector: sys::Selector,
}

impl Poller {
    #[inline]
    pub fn new() -> io::Result<Self> {
        Ok(Poller { selector: sys::Selector::new()? })
    }

    #[inline]
    pub fn poll(&self, events: &mut [Event], timeout: Option<Duration>) -> io::Result<usize> {
        Ok(self.selector
               .select(events.as_mut_ptr() as *mut sys::Event,
                       events.len(),
                       timeout)?)
    }
}

#[inline]
pub fn selector(poller: &Poller) -> &sys::Selector {
    &poller.selector
}
