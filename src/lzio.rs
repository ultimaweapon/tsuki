#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::lstate::{lua_Reader, lua_State};
use libc::memcpy;

pub type ZIO = Zio;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Zio {
    pub n: usize,
    pub p: *const libc::c_char,
    pub reader: lua_Reader,
    pub data: *mut libc::c_void,
    pub L: *mut lua_State,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct Mbuffer {
    pub buffer: *mut libc::c_char,
    pub n: usize,
    pub buffsize: usize,
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaZ_fill(mut z: *mut ZIO) -> libc::c_int {
    let mut size: usize = 0;
    let mut L: *mut lua_State = (*z).L;
    let mut buff: *const libc::c_char = 0 as *const libc::c_char;
    buff = ((*z).reader).expect("non-null function pointer")(L, (*z).data, &mut size);
    if buff.is_null() || size == 0 as libc::c_int as usize {
        return -(1 as libc::c_int);
    }
    (*z).n = size.wrapping_sub(1 as libc::c_int as usize);
    (*z).p = buff;
    let fresh0 = (*z).p;
    (*z).p = ((*z).p).offset(1);
    return *fresh0 as libc::c_uchar as libc::c_int;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaZ_init(
    mut L: *mut lua_State,
    mut z: *mut ZIO,
    mut reader: lua_Reader,
    mut data: *mut libc::c_void,
) {
    (*z).L = L;
    (*z).reader = reader;
    (*z).data = data;
    (*z).n = 0 as libc::c_int as usize;
    (*z).p = 0 as *const libc::c_char;
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn luaZ_read(
    mut z: *mut ZIO,
    mut b: *mut libc::c_void,
    mut n: usize,
) -> usize {
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
