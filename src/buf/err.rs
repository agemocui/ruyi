#[derive(Debug, Fail)]
pub enum Error {
    #[fail(display = "Index out of bounds")] IndexOutOfBounds,
    #[fail(display = "Buffer underflow")] Underflow,
}
