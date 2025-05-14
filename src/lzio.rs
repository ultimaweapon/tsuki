#![allow(
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lstate::lua_Reader;
use libc::memcpy;
use std::ffi::{c_char, c_int, c_void};
use std::ptr::null;

pub type ZIO = Zio;

#[repr(C)]
pub struct Zio {
    pub n: usize,
    pub p: *const c_char,
    reader: lua_Reader,
    data: *mut c_void,
}

impl Zio {
    pub unsafe fn new(reader: lua_Reader, data: *mut c_void) -> Self {
        Self {
            n: 0,
            p: null(),
            reader,
            data,
        }
    }
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mbuffer {
    pub buffer: *mut libc::c_char,
    pub n: usize,
    pub buffsize: usize,
}

pub unsafe fn luaZ_fill(mut z: *mut ZIO) -> c_int {
    let mut size = 0;
    let buff = ((*z).reader)((*z).data, &mut size);

    if buff.is_null() || size == 0 {
        return -1;
    }

    (*z).n = size.wrapping_sub(1);
    (*z).p = buff;
    let fresh0 = (*z).p;
    (*z).p = ((*z).p).offset(1);

    return *fresh0 as libc::c_uchar as libc::c_int;
}

pub unsafe fn luaZ_read(mut z: *mut ZIO, mut b: *mut c_void, mut n: usize) -> usize {
    while n != 0 {
        let mut m: usize = 0;
        if (*z).n == 0 as libc::c_int as usize {
            if luaZ_fill(z) == -(1 as libc::c_int) {
                return n;
            } else {
                (*z).n = ((*z).n).wrapping_add(1);
                (*z).n;
                (*z).p = ((*z).p).offset(-1);
                (*z).p;
            }
        }
        m = if n <= (*z).n { n } else { (*z).n };
        memcpy(b, (*z).p as *const libc::c_void, m);
        (*z).n = ((*z).n).wrapping_sub(m);
        (*z).p = ((*z).p).offset(m as isize);
        b = (b as *mut libc::c_char).offset(m as isize) as *mut libc::c_void;
        n = n.wrapping_sub(m);
    }
    return 0 as libc::c_int as usize;
}
