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

use crate::ldebug::luaG_runerror;
use crate::lgc::luaC_fullgc;
use crate::lstate::{global_State, lua_State};
use libc::{free, realloc};
use std::ffi::{CStr, c_void};

pub unsafe fn luaM_growaux_(
    mut L: *mut lua_State,
    mut block: *mut libc::c_void,
    mut nelems: libc::c_int,
    mut psize: *mut libc::c_int,
    mut size_elems: libc::c_int,
    mut limit: libc::c_int,
    mut what: *const libc::c_char,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
    let mut newblock: *mut libc::c_void = 0 as *mut libc::c_void;
    let mut size: libc::c_int = *psize;
    if nelems + 1 as libc::c_int <= size {
        return Ok(block);
    }
    if size >= limit / 2 as libc::c_int {
        if ((size >= limit) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        {
            luaG_runerror(
                L,
                format!(
                    "too many {} (limit is {})",
                    CStr::from_ptr(what).to_string_lossy(),
                    limit
                ),
            )?;
        }
        size = limit;
    } else {
        size *= 2 as libc::c_int;
        if size < 4 as libc::c_int {
            size = 4 as libc::c_int;
        }
    }
    newblock = luaM_saferealloc_(
        L,
        block,
        *psize as usize * size_elems as usize,
        size as usize * size_elems as usize,
    );
    *psize = size;
    return Ok(newblock);
}

pub unsafe fn luaM_shrinkvector_(
    mut L: *mut lua_State,
    mut block: *mut libc::c_void,
    mut size: *mut libc::c_int,
    mut final_n: libc::c_int,
    mut size_elem: libc::c_int,
) -> *mut libc::c_void {
    let mut newblock: *mut libc::c_void = 0 as *mut libc::c_void;
    let mut oldsize: usize = (*size * size_elem) as usize;
    let mut newsize: usize = (final_n * size_elem) as usize;
    newblock = luaM_saferealloc_(L, block, oldsize, newsize);
    *size = final_n;
    return newblock;
}

pub unsafe fn luaM_toobig(mut L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    luaG_runerror(L, "memory allocation error: block too big")
}

pub unsafe fn luaM_free_(mut L: *mut lua_State, mut block: *mut libc::c_void, mut osize: usize) {
    let mut g: *mut global_State = (*L).l_G;
    free(block);
    (*g).GCdebt = ((*g).GCdebt as usize).wrapping_sub(osize) as isize as isize;
}

unsafe fn tryagain(
    mut L: *mut lua_State,
    mut block: *mut libc::c_void,
    mut nsize: usize,
) -> *mut libc::c_void {
    let mut g: *mut global_State = (*L).l_G;

    if (*g).nilvalue.tt_ as libc::c_int & 0xf as libc::c_int == 0 as libc::c_int
        && (*g).gcstopem == 0
    {
        luaC_fullgc(L, 1 as libc::c_int);
        return realloc(block, nsize);
    } else {
        return 0 as *mut libc::c_void;
    };
}

pub unsafe fn luaM_realloc_(
    mut L: *mut lua_State,
    mut block: *mut libc::c_void,
    mut osize: usize,
    mut nsize: usize,
) -> *mut libc::c_void {
    let mut g: *mut global_State = (*L).l_G;
    let mut newblock = if nsize == 0 {
        free(block);
        0 as *mut libc::c_void
    } else {
        realloc(block, nsize)
    };

    if ((newblock.is_null() && nsize > 0 as libc::c_int as usize) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        newblock = tryagain(L, block, nsize);
        if newblock.is_null() {
            return 0 as *mut libc::c_void;
        }
    }
    (*g).GCdebt = ((*g).GCdebt as usize)
        .wrapping_add(nsize)
        .wrapping_sub(osize) as isize;
    return newblock;
}

pub unsafe fn luaM_saferealloc_(
    mut L: *mut lua_State,
    mut block: *mut libc::c_void,
    mut osize: usize,
    mut nsize: usize,
) -> *mut libc::c_void {
    let mut newblock: *mut libc::c_void = luaM_realloc_(L, block, osize, nsize);

    if ((newblock.is_null() && nsize > 0 as libc::c_int as usize) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        todo!("invoke handle_alloc_error");
    }

    newblock
}

pub unsafe fn luaM_malloc_(mut L: *mut lua_State, mut size: usize) -> *mut c_void {
    if size == 0 {
        return 0 as *mut libc::c_void;
    } else {
        let mut g: *mut global_State = (*L).l_G;
        let mut newblock: *mut libc::c_void = realloc(0 as *mut libc::c_void, size);

        if ((newblock == 0 as *mut libc::c_void) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
        {
            newblock = tryagain(L, 0 as *mut libc::c_void, size);
            if newblock.is_null() {
                todo!("invoke handle_alloc_error");
            }
        }
        (*g).GCdebt = ((*g).GCdebt as usize).wrapping_add(size) as isize as isize;
        return newblock;
    };
}
