#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::{Lua, ParseError};
use core::ffi::c_void;
use core::ptr::null_mut;
use libc::{free, realloc};

pub unsafe fn luaM_growaux_(
    g: &Lua,
    block: *mut libc::c_void,
    nelems: libc::c_int,
    psize: *mut libc::c_int,
    size_elems: libc::c_int,
    limit: libc::c_int,
    what: &'static str,
    line: libc::c_int,
) -> Result<*mut c_void, ParseError> {
    let mut size: libc::c_int = *psize;
    if nelems + 1 as libc::c_int <= size {
        return Ok(block);
    }
    if size >= limit / 2 as libc::c_int {
        if size >= limit {
            return Err(ParseError::ItemLimit(what, limit, line));
        }
        size = limit;
    } else {
        size *= 2 as libc::c_int;
        if size < 4 as libc::c_int {
            size = 4 as libc::c_int;
        }
    }

    let newblock = luaM_saferealloc_(
        g,
        block,
        *psize as usize * size_elems as usize,
        size as usize * size_elems as usize,
    );
    *psize = size;
    return Ok(newblock);
}

pub unsafe fn luaM_shrinkvector_(
    g: *const Lua,
    block: *mut libc::c_void,
    size: *mut libc::c_int,
    final_n: libc::c_int,
    size_elem: libc::c_int,
) -> *mut libc::c_void {
    let oldsize: usize = (*size * size_elem) as usize;
    let newsize: usize = (final_n * size_elem) as usize;
    let newblock = luaM_saferealloc_(g, block, oldsize, newsize);
    *size = final_n;
    return newblock;
}

pub unsafe fn luaM_free_(g: *const Lua, block: *mut libc::c_void, osize: usize) {
    free(block);
    (*g).gc.decrease_debt(osize);
}

pub unsafe fn luaM_realloc_(
    g: *const Lua,
    block: *mut libc::c_void,
    osize: usize,
    nsize: usize,
) -> *mut libc::c_void {
    let newblock = if nsize == 0 {
        free(block);
        0 as *mut libc::c_void
    } else {
        realloc(block, nsize)
    };

    if newblock.is_null() && nsize > 0 {
        return 0 as *mut libc::c_void;
    }

    (*g).gc.increase_debt(nsize);
    (*g).gc.decrease_debt(osize);

    return newblock;
}

pub unsafe fn luaM_saferealloc_(
    g: *const Lua,
    block: *mut libc::c_void,
    osize: usize,
    nsize: usize,
) -> *mut libc::c_void {
    let newblock: *mut libc::c_void = luaM_realloc_(g, block, osize, nsize);

    if newblock.is_null() && nsize > 0 {
        todo!("invoke handle_alloc_error");
    }

    newblock
}

pub unsafe fn luaM_malloc_(g: *const Lua, size: usize) -> *mut c_void {
    if size == 0 {
        null_mut()
    } else {
        let newblock: *mut libc::c_void = realloc(0 as *mut libc::c_void, size);

        if newblock == 0 as *mut libc::c_void {
            todo!("invoke handle_alloc_error");
        }

        (*g).gc.increase_debt(size);

        newblock
    }
}
