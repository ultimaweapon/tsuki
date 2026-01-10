use alloc::boxed::Box;
use alloc::format;
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

pub fn unpackint(
    str: &[u8],
    islittle: bool,
    issigned: bool,
) -> Result<i64, Box<dyn core::error::Error>> {
    let size = str.len();
    let limit = if size <= size_of::<i64>() {
        size
    } else {
        size_of::<i64>()
    };

    // Read.
    let mut res = 0u64;

    for i in (0..limit).rev() {
        res <<= 8;
        res |= u64::from(str[if islittle { i } else { size - 1 - i }]);
    }

    if size < size_of::<i64>() {
        if issigned {
            let mask = 1u64 << size * 8 - 1;

            res = (res ^ mask).wrapping_sub(mask);
        }
    } else if size > size_of::<i64>() {
        let mask = if !issigned || res as i64 >= 0 {
            0
        } else {
            0xFF
        };

        for i in limit..size {
            if str[if islittle { i } else { size - 1 - i }] != mask {
                return Err(format!("{size}-byte integer does not fit into Lua Integer").into());
            }
        }
    }

    Ok(res as i64)
}

pub fn extend_with_endian<const L: usize>(b: &mut Vec<u8>, mut src: [u8; L], islittle: bool) {
    if islittle != cfg!(target_endian = "little") {
        src.reverse();
    }

    b.extend_from_slice(&src);
}
