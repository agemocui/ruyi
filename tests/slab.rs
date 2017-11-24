extern crate ruyi;

use ruyi::slab;

struct Counter {
    count: usize,
    origin: usize,
}

impl Counter {
    fn new(count: usize) -> Self {
        Counter {
            count,
            origin: count,
        }
    }

    fn count(&self) -> usize {
        self.count
    }

    fn set_count(&mut self, count: usize) {
        self.count = count;
        self.origin = count;
    }
}

impl Drop for Counter {
    fn drop(&mut self) {
        assert_eq!(self.count, self.origin);
        self.count += 1;
    }
}

#[derive(Clone, Copy)]
struct CntIdx(usize);

impl From<usize> for CntIdx {
    fn from(idx: usize) -> Self {
        CntIdx(idx)
    }
}

impl From<CntIdx> for usize {
    fn from(idx: CntIdx) -> Self {
        idx.0
    }
}

#[test]
fn slab_insert() {
    let mut slab = slab::with_capacity::<Counter, CntIdx>(1);
    slab.insert(Counter::new(10));
    assert_eq!(slab.len(), 1);

    slab.insert(Counter::new(20));
    assert_eq!(slab.len(), 2);

    slab.insert(Counter::new(30));
    assert_eq!(slab.len(), 3);
}

#[test]
fn slab_remove() {
    let mut slab = slab::new::<Counter, CntIdx>();
    let a1 = slab.insert(Counter::new(10));
    let a2 = slab.insert(Counter::new(20));
    {
        let f1 = slab.remove(a1);
        assert!(f1.is_some());
        assert_eq!(f1.unwrap().count(), 10);
    }

    slab.insert(Counter::new(30));

    {
        slab.remove(a2);
    }

    assert_eq!(slab.len(), 1);
}

#[test]
fn slab_get() {
    let mut slab = slab::new::<Counter, CntIdx>();
    let a1 = slab.insert(Counter::new(10));
    let a2 = slab.insert(Counter::new(20));

    assert_eq!(slab[a1].count(), 10);
    assert_eq!(slab[a2].count(), 20);

    slab.get_mut(a2).unwrap().set_count(200);
    assert_eq!(slab[a2].count(), 200);
    (&mut slab[a2]).set_count(40);
    assert_eq!(slab.remove(a2).unwrap().count(), 40);

    let a3 = slab.insert(Counter::new(30));
    assert_eq!(slab[a3].count(), 30);

    unsafe {
        assert_eq!(slab.get_unchecked(a3).count(), 30);
        slab.get_unchecked_mut(a3).set_count(300);
    }
    assert_eq!(slab[a3].count(), 300);

    assert_eq!(slab.remove(a3).unwrap().count(), 300);
    assert_eq!(slab.remove(a1).unwrap().count(), 10);
}

#[test]
fn slab_invalid_idx() {
    let mut slab = slab::new::<Counter, CntIdx>();
    let invalid_idx = slab::invalid_index();

    assert!(slab.get(invalid_idx).is_none());
    assert!(slab.get_mut(invalid_idx).is_none());
    assert!(slab.remove(invalid_idx).is_none());
}
