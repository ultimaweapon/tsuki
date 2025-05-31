#![allow(
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use libc::memcpy;
use std::ffi::{c_char, c_void};

pub type ZIO = Zio;

#[repr(C)]
pub struct Zio {
    pub n: usize,
    pub p: *const c_char,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mbuffer {
    pub buffer: *mut libc::c_char,
    pub n: usize,
    pub buffsize: usize,
}

pub unsafe fn luaZ_read(
    mut z: *mut ZIO,
    mut b: *mut c_void,
    mut n: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    while n != 0 {
        let mut m: usize = 0;
        if (*z).n == 0 as libc::c_int as usize {
            return Ok(n);
        }
        m = if n <= (*z).n { n } else { (*z).n };
        memcpy(b, (*z).p as *const libc::c_void, m);
        (*z).n = ((*z).n).wrapping_sub(m);
        (*z).p = ((*z).p).offset(m as isize);
        b = (b as *mut libc::c_char).offset(m as isize) as *mut libc::c_void;
        n = n.wrapping_sub(m);
    }
    return Ok(0 as libc::c_int as usize);
}
