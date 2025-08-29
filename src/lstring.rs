#![allow(non_camel_case_types, non_snake_case, unused_assignments)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::Str;
use core::ffi::c_char;

type c_int = i32;
type c_uint = u32;

pub unsafe fn luaS_eqlngstr<D>(a: *const Str<D>, b: *const Str<D>) -> c_int {
    (a == b || (*a).as_bytes() == (*b).as_bytes()).into()
}

pub unsafe fn luaS_hash(str: *const c_char, mut l: usize, seed: c_uint) -> c_uint {
    let mut h: c_uint = seed ^ l as c_uint;
    while l > 0 as c_int as usize {
        h ^= (h << 5 as c_int)
            .wrapping_add(h >> 2 as c_int)
            .wrapping_add(
                *str.offset(l.wrapping_sub(1 as c_int as usize) as isize) as u8 as c_uint,
            );
        l = l.wrapping_sub(1);
    }
    return h;
}

pub unsafe fn luaS_hashlongstr<D>(ts: *mut Str<D>) -> c_uint {
    if (*ts).extra.get() as c_int == 0 as c_int {
        let s = (*ts).as_bytes();

        (*ts)
            .hash
            .set(luaS_hash(s.as_ptr().cast(), s.len(), (*ts).hash.get()));
        (*ts).extra.set(1 as c_int as u8);
    }
    return (*ts).hash.get();
}
