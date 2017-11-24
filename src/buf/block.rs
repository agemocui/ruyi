use std::mem;
use std::rc::Rc;
use std::slice;

#[derive(Debug)]
struct Alloc {
    ptr: *mut u8,
    cap: usize,
}

#[inline]
fn alloc(cap: usize) -> Alloc {
    let mut buf = Vec::with_capacity(cap);
    let ptr = buf.as_mut_ptr();
    mem::forget(buf);
    Alloc { ptr, cap }
}

impl Drop for Alloc {
    fn drop(&mut self) {
        drop(unsafe { Vec::from_raw_parts(self.ptr, 0, self.cap) });
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Block {
    ptr: *mut u8,
    cap: usize,
    read_pos: usize,
    write_pos: usize,
    shared: Rc<Alloc>,
}

impl From<Vec<u8>> for Block {
    fn from(mut bytes: Vec<u8>) -> Self {
        let ptr = bytes.as_mut_ptr();
        let cap = bytes.capacity();
        let len = bytes.len();
        mem::forget(bytes);
        let alloc = Alloc { ptr, cap };
        Block {
            ptr,
            cap,
            read_pos: 0,
            write_pos: len,
            shared: Rc::new(alloc),
        }
    }
}

impl Block {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let alloc = alloc(capacity);
        Block {
            ptr: alloc.ptr,
            cap: alloc.cap,
            read_pos: 0,
            write_pos: 0,
            shared: Rc::new(alloc),
        }
    }

    #[inline]
    pub fn for_prependable(capacity: usize) -> Self {
        let alloc = alloc(capacity);
        Block {
            ptr: alloc.ptr,
            cap: alloc.cap,
            read_pos: alloc.cap,
            write_pos: alloc.cap,
            shared: Rc::new(alloc),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.read_pos == self.write_pos
    }

    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.ptr
    }

    #[inline]
    pub fn ptr_at(&self, off: usize) -> *const u8 {
        unsafe { self.ptr.offset(off as isize) }
    }

    #[inline]
    pub fn as_ptr(&self) -> *const u8 {
        self.ptr
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    #[inline]
    pub fn set_capacity(&mut self, capacity: usize) {
        debug_assert!(
            capacity >= self.write_pos,
            "`capacity` out of bounds: capacity={}, write_pos={}",
            capacity,
            self.write_pos
        );
        self.cap = capacity;
    }

    #[inline]
    pub fn read_pos(&self) -> usize {
        self.read_pos
    }

    #[inline]
    pub fn set_read_pos(&mut self, read_pos: usize) {
        debug_assert!(
            read_pos <= self.write_pos,
            "`read_pos` out of bounds: read_pos={}, write_pos={}",
            read_pos,
            self.write_pos
        );
        self.read_pos = read_pos;
    }

    #[inline]
    pub fn write_pos(&self) -> usize {
        self.write_pos
    }

    #[inline]
    pub fn set_write_pos(&mut self, write_pos: usize) {
        debug_assert!(
            write_pos >= self.read_pos && write_pos <= self.cap,
            "`write_pos` out of bounds: read_pos={}, write_pos={}, capacity={}",
            self.read_pos,
            write_pos,
            self.cap
        );
        self.write_pos = write_pos;
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.write_pos() - self.read_pos()
    }

    #[inline]
    pub fn appendable(&self) -> usize {
        self.capacity() - self.write_pos()
    }

    #[inline]
    pub fn prependable(&self) -> usize {
        self.read_pos()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr_at(self.read_pos()), self.len()) }
    }

    #[inline]
    pub fn as_bytes_from(&self, from: usize) -> &[u8] {
        let i = self.read_pos() + from;
        let len = self.write_pos() - i;
        unsafe { slice::from_raw_parts(self.ptr_at(i), len) }
    }

    #[inline]
    pub fn as_bytes_to(&self, to: usize) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr_at(self.read_pos()), to) }
    }

    #[inline]
    pub fn as_bytes_range(&self, from: usize, len: usize) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr_at(from), len) }
    }

    // Splits the block into two at the given index.
    // Returns a newly allocated Self. self contains bytes [0, at),
    // and the returned Self contains bytes [at, len).
    #[inline]
    pub fn split_off(&mut self, at: usize) -> Self {
        let off = self.read_pos() + at;

        debug_assert!(
            off <= self.write_pos(),
            "`at` out of bounds: read_pos={}, at={}, write_pos={}",
            self.read_pos,
            at,
            self.write_pos
        );

        let other_ptr = unsafe { self.ptr.offset(off as isize) };

        let other_write_pos = self.write_pos() - off;
        self.set_write_pos(off);

        let other_cap = self.cap - off;
        self.set_capacity(off);

        Block {
            ptr: other_ptr,
            cap: other_cap,
            read_pos: 0,
            write_pos: other_write_pos,
            shared: self.shared.clone(),
        }
    }

    #[inline]
    pub fn starts_with(&self, needle: &[u8]) -> bool {
        self.as_bytes().starts_with(needle)
    }

    #[inline]
    pub fn ends_with(&self, needle: &[u8]) -> bool {
        self.as_bytes().ends_with(needle)
    }
}
