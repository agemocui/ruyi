use std::io;

pub(super) trait ErrRes {
    fn err_res() -> Self;
}

impl ErrRes for i32 {
    #[inline]
    fn err_res() -> Self {
        -1
    }
}

impl ErrRes for isize {
    #[inline]
    fn err_res() -> Self {
        -1
    }
}

#[inline]
pub(super) fn cvt<V: ErrRes + PartialEq<V>>(v: V) -> io::Result<V> {
    if v != V::err_res() {
        Ok(v)
    } else {
        Err(io::Error::last_os_error())
    }
}
