extern crate ruyi;

use std::io::{Read, Write};
use std::mem;

use ruyi::buf::ByteBuf;
use ruyi::buf::codec::{u8, u8s, u32, f64, str};

#[test]
fn read_write() {
    let mut data1 = Vec::with_capacity(100);
    for i in 0..data1.capacity() {
        data1.push(i as u8);
    }
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let n1 = buf.as_writer().write(&data1).unwrap();
    assert_eq!(n1, 100);

    let mut data2: Vec<u8> = Vec::with_capacity(100);
    unsafe { data2.set_len(100) };

    let n2 = buf.as_reader().read(&mut data2).unwrap();
    assert_eq!(n2, 100);

    assert_eq!(data1, data2);
}

#[test]
fn codec_u32() {
    let mut buf = ByteBuf::with_capacity(1);

    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let mut n = buf.append(0x12345678, u32::big_endian::append).unwrap();
    assert_eq!(n, 4);

    n = buf.prepend(0x98765432, u32::big_endian::prepend)
        .unwrap();
    assert_eq!(n, 4);

    let mut i = buf.get(0, u32::big_endian::get).unwrap();
    assert_eq!(i, 0x98765432);

    i = buf.get(4, u32::little_endian::get).unwrap();
    assert_eq!(i, 0x78563412);

    i = buf.read(u32::little_endian::read).unwrap();
    assert_eq!(i, 0x32547698);

    i = buf.read(u32::big_endian::read).unwrap();
    assert_eq!(i, 0x12345678);

}

#[test]
fn codec_varint() {
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let mut n = buf.append(0x23456, u32::varint::append).unwrap();
    assert_eq!(n, 3);

    n = buf.append(0xFFFFFFFF, u32::varint::append).unwrap();
    assert_eq!(n, 5);

    n = buf.prepend(0xFFFFFFFF, u32::varint::prepend).unwrap();
    assert_eq!(n, 5);;

    let mut i = buf.get(0, u32::varint::get).unwrap();
    assert_eq!(i, 0xFFFFFFFF);

    i = buf.get(5, u32::varint::get).unwrap();
    assert_eq!(i, 0x23456);

    i = buf.get(8, u32::varint::get).unwrap();
    assert_eq!(i, 0xFFFFFFFF);

    i = buf.read(u32::varint::read).unwrap();
    assert_eq!(i, 0xFFFFFFFF);

    i = buf.read(u32::varint::read).unwrap();
    assert_eq!(i, 0x23456);

    i = buf.read(u32::varint::read).unwrap();
    assert_eq!(i, 0xFFFFFFFF);

    for _ in 0..2 {
        buf.append(0xFF, u8::append).unwrap();
    }
    buf.append(0x7F, u8::append).unwrap();

    i = buf.read(u32::varint::read).unwrap();
    assert_eq!(i, 0x1FFFFF);
}

#[test]
fn codec_u8() {
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let mut n = buf.append(51, u8::append).unwrap();
    assert_eq!(n, 1);

    n = buf.prepend(41, u8::prepend).unwrap();
    assert_eq!(n, 1);

    n = buf.append(61, u8::append).unwrap();
    assert_eq!(n, 1);

    n = buf.prepend(31, u8::prepend).unwrap();
    assert_eq!(n, 1);
    assert_eq!(buf.len(), 4);

    let mut b = buf.get(3, u8::get).unwrap();
    assert_eq!(b, 61);

    b = buf.get(2, u8::get).unwrap();
    assert_eq!(b, 51);

    b = buf.get(1, u8::get).unwrap();
    assert_eq!(b, 41);

    b = buf.get(0, u8::get).unwrap();
    assert_eq!(b, 31);

    assert_eq!(1, buf.set(0, 61, u8::set).unwrap());
    assert_eq!(1, buf.set(1, 51, u8::set).unwrap());
    assert_eq!(1, buf.set(2, 41, u8::set).unwrap());
    assert_eq!(1, buf.set(3, 31, u8::set).unwrap());

    b = buf.get(0, u8::get).unwrap();
    assert_eq!(b, 61);

    b = buf.get(1, u8::get).unwrap();
    assert_eq!(b, 51);

    b = buf.get(2, u8::get).unwrap();
    assert_eq!(b, 41);

    b = buf.get(3, u8::get).unwrap();
    assert_eq!(b, 31);

    let mut b = buf.read(u8::read).unwrap();
    assert_eq!(b, 61);

    b = buf.read(u8::read).unwrap();
    assert_eq!(b, 51);

    b = buf.read(u8::read).unwrap();
    assert_eq!(b, 41);

    b = buf.read(u8::read).unwrap();
    assert_eq!(b, 31);
}

#[test]
fn codec_u8s() {
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let mut n = buf.append(u8s::filling(6, 20), u8s::append_fill)
        .unwrap();
    assert_eq!(20, n);
    n = buf.prepend(u8s::filling(4, 20), u8s::prepend_fill)
        .unwrap();
    assert_eq!(20, n);

    let data1 = [4; 20];
    let data2 = [6; 20];

    let mut data3 = data1.to_vec();
    data3.append(&mut data2.to_vec());
    assert_eq!(data3, buf.get(0, u8s::get).unwrap());

    assert_eq!(data1.to_vec(), buf.read_exact(20, u8s::read_exact).unwrap());
    assert_eq!(data2.to_vec(), buf.read(u8s::read).unwrap());
}

#[test]
fn codec_f64() {
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    const F_V1: f64 = ::std::f64::consts::PI;
    const F_V2: f64 = F_V1 * 123456.789f64;
    let mut n = buf.append(F_V1, f64::big_endian::append).unwrap();
    assert_eq!(n, 8);

    n = buf.append(F_V2, f64::little_endian::append).unwrap();
    assert_eq!(n, 8);

    n = buf.prepend(F_V2, f64::big_endian::prepend).unwrap();
    assert_eq!(n, 8);

    n = buf.prepend(F_V1, f64::little_endian::prepend).unwrap();
    assert_eq!(n, 8);

    let mut f = buf.get(0, f64::little_endian::get).unwrap();
    assert_eq!(f, F_V1);

    f = buf.get(8, f64::big_endian::get).unwrap();
    assert_eq!(f, F_V2);

    f = buf.get(16, f64::big_endian::get).unwrap();
    assert_eq!(f, F_V1);

    f = buf.get(24, f64::little_endian::get).unwrap();
    assert_eq!(f, F_V2);

    f = buf.read(f64::little_endian::read).unwrap();
    assert_eq!(f, F_V1);

    f = buf.read(f64::big_endian::read).unwrap();
    assert_eq!(f, F_V2);

    f = buf.read(f64::big_endian::read).unwrap();
    assert_eq!(f, F_V1);

    f = buf.read(f64::little_endian::read).unwrap();
    assert_eq!(f, F_V2);
}

#[test]
fn codec_str_utf8() {
    let mut buf = ByteBuf::with_capacity(1);
    // min capacity is mem::size_of::<usize>()
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(size, buf.try_reserve_in_head(size));

    let str1 = "bytebuf string codec test";
    assert_eq!(str1.len(), buf.append(str1, str::utf8::append).unwrap());

    let str2 = "prepend str utf8 test";
    assert_eq!(str2.len(), buf.prepend(str2, str::utf8::prepend).unwrap());

    assert_eq!(str2.to_string() + str1, buf.get(0, str::utf8::get).unwrap());

    assert_eq!(str2,
               buf.read_exact(str2.len(), str::utf8::read_exact)
                   .unwrap());

    assert_eq!(str1, buf.read(str::utf8::read).unwrap());
}

#[test]
fn compare() {
    let mut b1 = ByteBuf::with_capacity(1);
    let mut b2 = ByteBuf::with_capacity(1);
    let mut b3 = ByteBuf::with_capacity(1);
    let mut b4 = ByteBuf::with_capacity(33);

    b1.append(&[9u8; 99] as &[u8], u8s::append).unwrap();
    b2.append(&[9u8; 100] as &[u8], u8s::append).unwrap();
    b3.append(&[9u8; 101] as &[u8], u8s::append).unwrap();
    b4.append(&[9u8; 100] as &[u8], u8s::append).unwrap();

    assert_eq!(b1 < b2, true);
    assert_eq!(b2 < b3, true);
    assert_eq!(b1 < b3, true);
    assert_eq!(b2, b4);

    b2.append(1, u8::append).unwrap();
    b4.append(3, u8::append).unwrap();
    assert_eq!(b2 < b4, true);

    b3.prepend(7, u8::prepend).unwrap();
    assert_eq!(b3 < b1, true);
}
