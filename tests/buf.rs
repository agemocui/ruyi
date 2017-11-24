extern crate ruyi;

use std::io::{Read, Write};
use std::mem;

use ruyi::buf::ByteBuf;
use ruyi::buf::codec::{f64, u32, u8, u8s};

#[test]
fn read_write() {
    let mut data1 = Vec::with_capacity(100);
    for i in 0..data1.capacity() {
        data1.push(i as u8);
    }
    let mut buf = ByteBuf::with_growth(1);
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(0, buf.try_reserve_in_head(size));

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
    let mut buf = ByteBuf::with_growth(1);
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(0, buf.try_reserve_in_head(size));

    let mut n = buf.append(0x12345678, u32::big_endian::append).unwrap();
    assert_eq!(n, 4);

    n = buf.prepend(0x98765432, u32::big_endian::prepend).unwrap();
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
    assert_eq!(1, buf.try_reserve_in_head(2));

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
    assert_eq!(1, buf.try_reserve_in_head(5));

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
    let mut buf = ByteBuf::with_growth(1);
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(0, buf.try_reserve_in_head(size));

    let mut n = buf.append(u8s::filling(6, 20), u8s::append_fill).unwrap();
    assert_eq!(20, n);
    n = buf.prepend(u8s::filling(4, 20), u8s::prepend_fill).unwrap();
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
    let mut buf = ByteBuf::with_growth(1);
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(0, buf.try_reserve_in_head(size));

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
    use std::str::from_utf8;
    let mut buf = ByteBuf::with_growth(1);
    let size = mem::size_of::<usize>() - 1;
    assert_eq!(0, buf.try_reserve_in_head(size));

    let str1 = "bytebuf string codec test";
    assert_eq!(
        str1.len(),
        buf.append(str1.as_bytes(), u8s::append).unwrap()
    );

    let str2 = "prepend str utf8 test";
    assert_eq!(
        str2.len(),
        buf.prepend(str2.as_bytes(), u8s::prepend).unwrap()
    );

    assert_eq!(
        str2.to_string() + str1,
        from_utf8(&buf.get(0, u8s::get).unwrap()).unwrap()
    );

    assert_eq!(
        str2,
        from_utf8(&buf.read_exact(str2.len(), u8s::read_exact).unwrap()).unwrap()
    );

    assert_eq!(str1, from_utf8(&buf.read(u8s::read).unwrap()).unwrap());
}

#[test]
fn compare() {
    let mut b1 = ByteBuf::with_growth(1);
    let mut b2 = ByteBuf::with_growth(1);
    let mut b3 = ByteBuf::with_growth(1);
    let mut b4 = ByteBuf::with_growth(33);

    b1.append(&[9u8; 99] as &[u8], u8s::append).unwrap();
    b2.append(&[9u8; 100] as &[u8], u8s::append).unwrap();
    b3.append(&[9u8; 101] as &[u8], u8s::append).unwrap();
    b4.append(&[9u8; 100] as &[u8], u8s::append).unwrap();

    assert!(b1 < b2);
    assert!(b2 < b3);
    assert!(b1 < b3);
    assert_eq!(b2, b4);

    b2.append(1, u8::append).unwrap();
    b4.append(3, u8::append).unwrap();
    assert!(b2 < b4);

    b3.prepend(7, u8::prepend).unwrap();
    assert!(b3 < b1);
}

#[test]
fn starts_with() {
    let mut buf = ByteBuf::with_growth(1);
    for i in 0..100 {
        buf.append(i, u8::append).unwrap();
    }
    let bytes = [0u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12];
    assert!(buf.starts_with(&bytes[0..11]));
    assert!(!buf.starts_with(&bytes));
}

#[test]
fn ends_with() {
    let mut buf = ByteBuf::with_growth(1);
    for i in 0..100 {
        buf.append(i, u8::append).unwrap();
    }
    let bytes = [88u8, 88, 89, 90, 91, 92, 93, 94, 95, 96, 97, 98, 99];
    assert!(buf.ends_with(&bytes[1..]));
    assert!(!buf.ends_with(&bytes));
}

#[test]
fn find_and_rfind_from() {
    let mut v = Vec::with_capacity(100);
    for i in 0..100 {
        v.push(i);
    }
    let mut buf = ByteBuf::with_growth(1);
    buf.append(&v[..], u8s::append).unwrap();
    buf.skip(13);
    let bytes = &v[13..];

    let mut needle = &bytes[30..57];
    assert_eq!(buf.find_from(needle, 0), Some(30));
    assert_eq!(buf.find_from(needle, 23), Some(30));
    assert_eq!(buf.find_from(needle, 30), Some(30));
    assert_eq!(buf.find_from(needle, 33), None);
    assert_eq!(buf.find_from(needle, 61), None);

    assert_eq!(buf.rfind_from(needle, 0), None);
    assert_eq!(buf.rfind_from(needle, 23), None);
    assert_eq!(buf.rfind_from(needle, 30), Some(30));
    assert_eq!(buf.rfind_from(needle, 33), Some(30));
    assert_eq!(buf.rfind_from(needle, 61), Some(30));

    needle = &bytes[11..12];
    assert_eq!(buf.find_from(needle, 0), Some(11));
    assert_eq!(buf.find_from(needle, 9), Some(11));
    assert_eq!(buf.find_from(needle, 11), Some(11));
    assert_eq!(buf.find_from(needle, 12), None);
    assert_eq!(buf.find_from(needle, 13), None);

    assert_eq!(buf.rfind_from(needle, 0), None);
    assert_eq!(buf.rfind_from(needle, 9), None);
    assert_eq!(buf.rfind_from(needle, 11), Some(11));
    assert_eq!(buf.rfind_from(needle, 12), Some(11));
    assert_eq!(buf.rfind_from(needle, 13), Some(11));

    needle = &bytes[25..28];
    assert_eq!(buf.find_from(needle, 0), Some(25));
    assert_eq!(buf.find_from(needle, 11), Some(25));
    assert_eq!(buf.find_from(needle, 25), Some(25));
    assert_eq!(buf.find_from(needle, 27), None);
    assert_eq!(buf.find_from(needle, 29), None);

    assert_eq!(buf.rfind_from(needle, 0), None);
    assert_eq!(buf.rfind_from(needle, 11), None);
    assert_eq!(buf.rfind_from(needle, 25), Some(25));
    assert_eq!(buf.rfind_from(needle, 27), Some(25));
    assert_eq!(buf.rfind_from(needle, 29), Some(25));
}

#[test]
fn windows() {
    let mut v = Vec::with_capacity(100);
    for i in 0..100 {
        v.push(i);
    }
    let mut buf = ByteBuf::with_growth(1);
    buf.append(&v[..], u8s::append).unwrap();
    buf.skip(13);
    let bytes = &v[13..];

    let mut needle = &bytes[30..57];
    assert_eq!(
        buf.windows(needle.len()).position(|w| w == needle),
        Some(30)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(23).position(|w| needle == w),
        Some(30 - 23)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(30).position(|w| w == needle),
        Some(30 - 30)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(33).position(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len()).skip(61).position(|w| w == needle),
        None
    );

    assert_eq!(
        buf.windows(needle.len()).rposition(|w| w == needle),
        Some(30)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(23)
            .rposition(|w| needle == w),
        Some(30 - 23)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(30)
            .rposition(|w| w == needle),
        Some(30 - 30)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(33)
            .rposition(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(61)
            .rposition(|w| w == needle),
        None
    );

    needle = &bytes[11..12];
    assert_eq!(
        buf.windows(needle.len()).position(|w| w == needle),
        Some(11)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(9).position(|w| w == needle),
        Some(11 - 9)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(11).position(|w| w == needle),
        Some(11 - 11)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(12).position(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len()).skip(13).position(|w| w == needle),
        None
    );

    assert_eq!(
        buf.windows(needle.len()).rposition(|w| w == needle),
        Some(11)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(9).rposition(|w| w == needle),
        Some(11 - 9)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(11)
            .rposition(|w| w == needle),
        Some(11 - 11)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(12)
            .rposition(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(13)
            .rposition(|w| w == needle),
        None
    );

    needle = &bytes[25..28];
    assert_eq!(
        buf.windows(needle.len()).position(|w| needle == w),
        Some(25)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(21).position(|w| w == needle),
        Some(25 - 21)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(25).position(|w| w == needle),
        Some(25 - 25)
    );
    assert_eq!(
        buf.windows(needle.len()).skip(27).position(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len()).skip(29).position(|w| needle == w),
        None
    );

    assert_eq!(
        buf.windows(needle.len()).rposition(|w| needle == w),
        Some(25)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(21)
            .rposition(|w| w == needle),
        Some(25 - 21)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(25)
            .rposition(|w| w == needle),
        Some(25 - 25)
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(27)
            .rposition(|w| w == needle),
        None
    );
    assert_eq!(
        buf.windows(needle.len())
            .skip(29)
            .rposition(|w| needle == w),
        None
    );
}
