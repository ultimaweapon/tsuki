#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]
#![allow(unsafe_op_in_unsafe_fn)]

use crate::ParseError;
use alloc::alloc::handle_alloc_error;
use core::alloc::Layout;
use core::ptr::null_mut;

type c_int = i32;

pub unsafe fn luaM_growaux_<T>(
    block: *mut T,
    nelems: c_int,
    psize: *mut c_int,
    size_elems: c_int,
    limit: c_int,
    name: &'static str,
    line: c_int,
) -> Result<*mut T, ParseError> {
    let mut size: c_int = *psize;
    if nelems + 1 as c_int <= size {
        return Ok(block);
    }
    if size >= limit / 2 as c_int {
        if size >= limit {
            return Err(ParseError::ItemLimit { name, limit, line });
        }
        size = limit;
    } else {
        size *= 2 as c_int;
        if size < 4 as c_int {
            size = 4 as c_int;
        }
    }

    let newblock = luaM_realloc_(
        block,
        *psize as usize * size_elems as usize,
        size as usize * size_elems as usize,
    );
    *psize = size;
    return Ok(newblock);
}

pub unsafe fn luaM_shrinkvector_<T>(
    block: *mut T,
    size: *mut c_int,
    final_n: c_int,
    size_elem: c_int,
) -> *mut T {
    let oldsize: usize = (*size * size_elem) as usize;
    let newsize: usize = (final_n * size_elem) as usize;
    let newblock = luaM_realloc_(block, oldsize, newsize);
    *size = final_n;
    return newblock;
}

pub unsafe fn luaM_free_<T>(block: *mut T, osize: usize) {
    if osize == 0 {
        return;
    }

    alloc::alloc::dealloc(
        block.cast(),
        Layout::from_size_align(osize, align_of::<T>()).unwrap(),
    );
}

pub unsafe fn luaM_realloc_<T>(block: *mut T, osize: usize, nsize: usize) -> *mut T {
    let align = align_of::<T>();

    if nsize == 0 {
        if osize != 0 {
            alloc::alloc::dealloc(block.cast(), Layout::from_size_align(osize, align).unwrap());
        }

        null_mut()
    } else if osize != 0 {
        let v = alloc::alloc::realloc(
            block.cast(),
            Layout::from_size_align(osize, align).unwrap(),
            nsize,
        );

        if v.is_null() {
            handle_alloc_error(Layout::from_size_align(nsize, align).unwrap());
        }

        v.cast()
    } else {
        let l = Layout::from_size_align(nsize, align).unwrap();
        let v = alloc::alloc::alloc(l);

        if v.is_null() {
            handle_alloc_error(l);
        }

        v.cast()
    }
}
