#![feature(test)]

extern crate test;
extern crate ruyi;

use ruyi::buf::ByteBuf;
use ruyi::buf::codec::*;

use test::Bencher;

const SIZE: usize = 16 * 1024;


#[bench]
fn bench_find(b: &mut Bencher) {
    let mut buf = ByteBuf::with_capacity(SIZE);
    buf.append(u8s::filling(0, SIZE), u8s::append_fill).unwrap();
    let needle = b"\r\n";
    b.iter(|| { buf.find(needle); });
}

#[bench]
fn bench_windows(b: &mut Bencher) {
    let mut buf = ByteBuf::with_capacity(SIZE);
    buf.append(u8s::filling(0, SIZE), u8s::append_fill).unwrap();
    let needle = b"\r\n";
    b.iter(|| { buf.windows(needle.len()).position(|w| w == needle); });
}

const BYTES_SIZE: usize = 1024 * 6;

#[bench]
fn bench_append_slice(b: &mut Bencher) {
    let mut bytes = Vec::with_capacity(BYTES_SIZE);
    unsafe {
        bytes.set_len(BYTES_SIZE);
    }
    b.iter(|| for _ in 0..100 {
        let bytes2 = bytes.to_vec();
        let mut buf = ByteBuf::with_capacity(SIZE);
        buf.append(bytes2.as_slice(), u8s::append).unwrap();
    });
}

#[bench]
fn bench_append_bytes(b: &mut Bencher) {
    let mut bytes = Vec::with_capacity(BYTES_SIZE);
    unsafe {
        bytes.set_len(BYTES_SIZE);
    }
    b.iter(|| for _ in 0..100 {
        let bytes2 = bytes.to_vec();
        let mut buf = ByteBuf::with_capacity(SIZE);
        buf.append(bytes2, u8s::append_bytes).unwrap();
    });
}
