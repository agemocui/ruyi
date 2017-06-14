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
