use std::ptr;

fn reverse(p: *mut u8, len: usize) {
    let j = len - 1;
    let n = len >> 1;
    for i in 0..n {
        unsafe {
            let ptr_x = p.offset(i as isize);
            let ptr_y = p.offset((j - i) as isize);
            ptr::swap(ptr_x, ptr_y);
        }
    }
}

pub mod i8;
pub mod u8;
pub mod u8s;

pub mod i16;
pub mod u16;

pub mod i32;
pub mod u32;

pub mod i64;
pub mod u64;

pub mod f32;

pub mod f64;
