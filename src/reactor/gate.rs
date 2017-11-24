use std::marker::PhantomData;

use reactor::CURRENT_LOOP;

pub struct Gate {
    _marker: PhantomData<()>,
}

impl Gate {
    pub fn new() -> Option<Self> {
        match CURRENT_LOOP.with(|eloop| unsafe { eloop.as_mut() }.enter_gate()) {
            true => Some(Gate {
                _marker: PhantomData,
            }),
            false => None,
        }
    }
}

impl Drop for Gate {
    fn drop(&mut self) {
        CURRENT_LOOP.with(|eloop| unsafe { eloop.as_mut() }.leave_gate());
    }
}
