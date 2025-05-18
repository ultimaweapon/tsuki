#![allow(
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals
)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ldebug::luaG_runerror;
use crate::lstate::{global_State, lua_State};
use libc::{free, realloc};
use std::ffi::{CStr, c_void};

pub unsafe fn luaM_growaux_(
    L: *mut lua_State,
    block: *mut libc::c_void,
    nelems: libc::c_int,
    psize: *mut libc::c_int,
    size_elems: libc::c_int,
    limit: libc::c_int,
    what: *const libc::c_char,
) -> Result<*mut c_void, Box<dyn std::error::Error>> {
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

    let newblock = luaM_saferealloc_(
        L,
        block,
        *psize as usize * size_elems as usize,
        size as usize * size_elems as usize,
    );
    *psize = size;
    return Ok(newblock);
}

pub unsafe fn luaM_shrinkvector_(
    L: *mut lua_State,
    block: *mut libc::c_void,
    size: *mut libc::c_int,
    final_n: libc::c_int,
    size_elem: libc::c_int,
) -> *mut libc::c_void {
    let oldsize: usize = (*size * size_elem) as usize;
    let newsize: usize = (final_n * size_elem) as usize;
    let newblock = luaM_saferealloc_(L, block, oldsize, newsize);
    *size = final_n;
    return newblock;
}

pub unsafe fn luaM_toobig(L: *mut lua_State) -> Result<(), Box<dyn std::error::Error>> {
    luaG_runerror(L, "memory allocation error: block too big")
}

pub unsafe fn luaM_free_(L: *mut lua_State, block: *mut libc::c_void, osize: usize) {
    let g: *mut global_State = (*L).l_G;
    free(block);
    (*g).GCdebt = ((*g).GCdebt as usize).wrapping_sub(osize) as isize as isize;
}

pub unsafe fn luaM_realloc_(
    L: *mut lua_State,
    block: *mut libc::c_void,
    osize: usize,
    nsize: usize,
) -> *mut libc::c_void {
    let g: *mut global_State = (*L).l_G;
    let newblock = if nsize == 0 {
        free(block);
        0 as *mut libc::c_void
    } else {
        realloc(block, nsize)
    };

    if ((newblock.is_null() && nsize > 0 as libc::c_int as usize) as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        return 0 as *mut libc::c_void;
    }
    (*g).GCdebt = ((*g).GCdebt as usize)
        .wrapping_add(nsize)
        .wrapping_sub(osize) as isize;
    return newblock;
}

pub unsafe fn luaM_saferealloc_(
    L: *mut lua_State,
    block: *mut libc::c_void,
    osize: usize,
    nsize: usize,
) -> *mut libc::c_void {
    let newblock: *mut libc::c_void = luaM_realloc_(L, block, osize, nsize);

    if newblock.is_null() && nsize > 0 {
        todo!("invoke handle_alloc_error");
    }

    newblock
}

pub unsafe fn luaM_malloc_(L: *mut lua_State, size: usize) -> *mut c_void {
    if size == 0 {
        return 0 as *mut libc::c_void;
    } else {
        let g: *mut global_State = (*L).l_G;
        let newblock: *mut libc::c_void = realloc(0 as *mut libc::c_void, size);

        if ((newblock == 0 as *mut libc::c_void) as libc::c_int != 0 as libc::c_int) as libc::c_int
            as libc::c_long
            != 0
        {
            todo!("invoke handle_alloc_error");
        }
        (*g).GCdebt = ((*g).GCdebt as usize).wrapping_add(size) as isize as isize;
        return newblock;
    };
}
