#[derive(Debug, Fail)]
pub enum BufError {
    #[fail(display = "Index out of bounds")] IndexOutOfBounds,
    #[fail(display = "Buffer underflow")] Underflow,
}
