use alloc::vec::Vec;

pub fn packint(b: &mut Vec<u8>, mut n: u64, little: bool, size: usize, neg: bool) {
    let mut i = 1;
    let o = b.len();

    b.push((n & 0xFF) as u8);

    while i < size {
        n >>= 8;
        b.push((n & 0xFF) as u8);
        i += 1;
    }

    if neg && size > size_of::<i64>() {
        b[size_of::<i64>()..].fill(0xFF);
    }

    if !little {
        b[o..].reverse();
    }
}

pub fn copywithendian<const L: usize>(b: &mut Vec<u8>, mut src: [u8; L], islittle: bool) {
    if islittle != cfg!(target_endian = "little") {
        src.reverse();
    }

    b.extend_from_slice(&src);
}
