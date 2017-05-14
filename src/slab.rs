use std::fmt;
use std::marker;
use std::mem;
use std::ops::{Index, IndexMut};
use std::usize::MAX;

use super::unreachable;

const INVALID_INDEX: usize = MAX;

#[inline]
pub fn invalid_index<I: From<usize>>() -> I {
    I::from(INVALID_INDEX)
}

#[inline]
pub fn new<T, I: From<usize> + Into<usize>>() -> Slab<T, I> {
    Slab::new()
}

#[inline]
pub fn with_capacity<T, I: From<usize> + Into<usize>>(capacity: usize) -> Slab<T, I> {
    Slab::with_capacity(capacity)
}

enum Object<T> {
    Free(usize),
    Used(T),
}

impl<T> Object<T> {
    #[inline]
    unsafe fn unchecked_unwrap(&self) -> &T {
        match *self {
            Object::Used(ref elem) => elem,
            Object::Free(_) => unreachable(),
        }
    }

    #[inline]
    unsafe fn unchecked_unwrap_mut(&mut self) -> &mut T {
        match *self {
            Object::Used(ref mut elem) => elem,
            Object::Free(_) => unreachable(),
        }
    }

    #[inline]
    unsafe fn unchecked_next_free(&self) -> usize {
        match *self {
            Object::Free(idx) => idx,
            Object::Used(_) => unreachable(),
        }
    }
}

impl<T> fmt::Debug for Object<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Object::Free(idx) => write!(f, "Free({})", idx),
            Object::Used(_) => write!(f, "Used"),
        }
    }
}

#[derive(Debug)]
pub struct Slab<T, I = usize> {
    objs: Vec<Object<T>>,
    len: usize,
    hfree: usize, // index to the head of the free list
    _marker: marker::PhantomData<I>,
}

unsafe impl<T: Send, I> Send for Slab<T, I> {}


impl<T, I> Slab<T, I> {
    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }
}

impl<T, I: From<usize> + Into<usize>> Slab<T, I> {
    #[inline]
    fn new() -> Self {
        Slab {
            objs: Vec::new(),
            len: 0,
            hfree: INVALID_INDEX,
            _marker: marker::PhantomData,
        }
    }

    #[inline]
    fn with_capacity(capacity: usize) -> Self {
        Slab {
            objs: Vec::with_capacity(capacity),
            len: 0,
            hfree: INVALID_INDEX,
            _marker: marker::PhantomData,
        }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.objs.capacity()
    }

    #[inline]
    pub fn clear(&mut self) {
        if self.len > 0 {
            self.objs.clear();
            self.len = 0;
            self.hfree = INVALID_INDEX;
        }
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let n = self.objs.capacity() - self.len;
        if additional > n {
            self.objs.reserve(additional - n);
        }
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        let n = self.objs.capacity() - self.len;
        if additional > n {
            self.objs.reserve_exact(additional - n);
        }
    }

    pub fn insert(&mut self, elem: T) -> I {
        let obj_idx;
        if self.hfree != INVALID_INDEX {
            obj_idx = usize::from(self.hfree);
            let obj = unsafe { self.objs.get_unchecked_mut(obj_idx) };
            self.hfree = unsafe { obj.unchecked_next_free() };
            *obj = Object::Used(elem);
        } else {
            obj_idx = usize::from(self.objs.len());
            self.objs.push(Object::Used(elem));
        }
        self.len += 1;
        I::from(obj_idx)
    }

    pub fn remove(&mut self, idx: I) -> Option<T> {
        let i = idx.into();
        if let Some(obj) = self.objs.get_mut(i) {
            if let Object::Used(_) = *obj {
                let old_obj = mem::replace(obj, Object::Free(self.hfree));
                self.hfree = i;
                self.len -= 1;
                if let Object::Used(elem) = old_obj {
                    return Some(elem);
                }
            }
        }
        None
    }

    pub fn get(&self, idx: I) -> Option<&T> {
        if let Some(obj) = self.objs.get(idx.into()) {
            if let Object::Used(ref elem) = *obj {
                return Some(elem);
            }
        }
        None
    }

    pub fn get_mut(&mut self, idx: I) -> Option<&mut T> {
        if let Some(obj) = self.objs.get_mut(idx.into()) {
            if let Object::Used(ref mut elem) = *obj {
                return Some(elem);
            }
        }
        None
    }

    #[inline]
    pub unsafe fn get_unchecked(&self, idx: I) -> &T {
        self.objs.get_unchecked(idx.into()).unchecked_unwrap()
    }

    #[inline]
    pub unsafe fn get_unchecked_mut(&mut self, idx: I) -> &mut T {
        self.objs
            .get_unchecked_mut(idx.into())
            .unchecked_unwrap_mut()
    }
}

impl<T, I: From<usize> + Into<usize>> Index<I> for Slab<T, I> {
    type Output = T;

    #[inline]
    fn index(&self, idx: I) -> &T {
        self.get(idx).expect("Invalid ID")
    }
}

impl<T, I: From<usize> + Into<usize>> IndexMut<I> for Slab<T, I> {
    #[inline]
    fn index_mut(&mut self, idx: I) -> &mut T {
        self.get_mut(idx).expect("Invalid ID")
    }
}
