pub trait Borrow<Borrowed>
where
    Borrowed: ?Sized,
{
    fn borrow(&self) -> &Borrowed;
}

impl<T> Borrow<T> for T
where
    T: ?Sized,
{
    #[inline]
    fn borrow(&self) -> &T {
        self
    }
}

pub trait BorrowMut<Borrowed>: Borrow<Borrowed>
where
    Borrowed: ?Sized,
{
    fn borrow_mut(&mut self) -> &mut Borrowed;
}

impl<T> BorrowMut<T> for T
where
    T: ?Sized,
{
    #[inline]
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}
