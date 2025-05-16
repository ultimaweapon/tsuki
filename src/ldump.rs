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
#![allow(path_statements)]

use crate::lobject::{Proto, TString, TValue};
use crate::lstate::{GCUnion, lua_State, lua_Writer};
use std::ffi::{c_int, c_void};
use std::ptr::null_mut;

#[repr(C)]
struct DumpState {
    L: *mut lua_State,
    writer: lua_Writer,
    data: *mut c_void,
    strip: c_int,
    status: c_int,
}

unsafe fn dumpBlock(
    D: *mut DumpState,
    b: *const c_void,
    size: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*D).status == 0 && size > 0 {
        (*D).status = ((*D).writer)((*D).L, b, size, (*D).data)?;
    }
    Ok(())
}

unsafe fn dumpByte(
    mut D: *mut DumpState,
    mut y: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut x: u8 = y as u8;
    dumpBlock(
        D,
        &mut x as *mut u8 as *const libc::c_void,
        1usize.wrapping_mul(::core::mem::size_of::<u8>()),
    )
}

unsafe fn dumpSize(mut D: *mut DumpState, mut x: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut buff: [u8; 10] = [0; 10];
    let mut n: libc::c_int = 0 as libc::c_int;
    loop {
        n += 1;
        buff[(::core::mem::size_of::<usize>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_add(6 as libc::c_int as libc::c_ulong)
            .wrapping_div(7 as libc::c_int as libc::c_ulong)
            .wrapping_sub(n as libc::c_ulong) as usize] = (x & 0x7f as libc::c_int as usize) as u8;
        x >>= 7 as libc::c_int;
        if !(x != 0 as libc::c_int as usize) {
            break;
        }
    }
    buff[(::core::mem::size_of::<usize>() as libc::c_ulong)
        .wrapping_mul(8 as libc::c_int as libc::c_ulong)
        .wrapping_add(6 as libc::c_int as libc::c_ulong)
        .wrapping_div(7 as libc::c_int as libc::c_ulong)
        .wrapping_sub(1 as libc::c_int as libc::c_ulong) as usize] =
        (buff[(::core::mem::size_of::<usize>() as libc::c_ulong)
            .wrapping_mul(8 as libc::c_int as libc::c_ulong)
            .wrapping_add(6 as libc::c_int as libc::c_ulong)
            .wrapping_div(7 as libc::c_int as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as usize] as libc::c_int
            | 0x80 as libc::c_int) as u8;
    dumpBlock(
        D,
        buff.as_mut_ptr()
            .offset(
                (::core::mem::size_of::<usize>() as libc::c_ulong)
                    .wrapping_mul(8 as libc::c_int as libc::c_ulong)
                    .wrapping_add(6 as libc::c_int as libc::c_ulong)
                    .wrapping_div(7 as libc::c_int as libc::c_ulong) as isize,
            )
            .offset(-(n as isize)) as *const libc::c_void,
        (n as usize).wrapping_mul(::core::mem::size_of::<u8>()),
    )
}

unsafe fn dumpInt(
    mut D: *mut DumpState,
    mut x: libc::c_int,
) -> Result<(), Box<dyn std::error::Error>> {
    dumpSize(D, x as usize)
}

unsafe fn dumpNumber(mut D: *mut DumpState, mut x: f64) -> Result<(), Box<dyn std::error::Error>> {
    dumpBlock(
        D,
        &mut x as *mut f64 as *const libc::c_void,
        1usize.wrapping_mul(::core::mem::size_of::<f64>()),
    )
}

unsafe fn dumpInteger(mut D: *mut DumpState, mut x: i64) -> Result<(), Box<dyn std::error::Error>> {
    dumpBlock(
        D,
        &mut x as *mut i64 as *const libc::c_void,
        1usize.wrapping_mul(::core::mem::size_of::<i64>()),
    )
}

unsafe fn dumpString(
    mut D: *mut DumpState,
    mut s: *const TString,
) -> Result<(), Box<dyn std::error::Error>> {
    if s.is_null() {
        dumpSize(D, 0 as libc::c_int as usize)?;
    } else {
        let mut size: usize = if (*s).shrlen as libc::c_int != 0xff as libc::c_int {
            (*s).shrlen as usize
        } else {
            (*s).u.lnglen
        };
        let mut str: *const libc::c_char = ((*s).contents).as_ptr();
        dumpSize(D, size.wrapping_add(1 as libc::c_int as usize))?;
        dumpBlock(
            D,
            str as *const libc::c_void,
            size.wrapping_mul(::core::mem::size_of::<libc::c_char>()),
        )?;
    };
    Ok(())
}

unsafe fn dumpCode(
    mut D: *mut DumpState,
    mut f: *const Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    dumpInt(D, (*f).sizecode)?;
    dumpBlock(
        D,
        (*f).code as *const libc::c_void,
        ((*f).sizecode as usize).wrapping_mul(::core::mem::size_of::<u32>()),
    )
}

unsafe fn dumpConstants(
    mut D: *mut DumpState,
    mut f: *const Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = (*f).sizek;
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        let mut o: *const TValue = &mut *((*f).k).offset(i as isize) as *mut TValue;
        let mut tt: libc::c_int = (*o).tt_ as libc::c_int & 0x3f as libc::c_int;
        dumpByte(D, tt)?;
        match tt {
            19 => dumpNumber(D, (*o).value_.n)?,
            3 => dumpInteger(D, (*o).value_.i)?,
            4 | 20 => dumpString(D, &mut (*((*o).value_.gc as *mut GCUnion)).ts)?,
            _ => {}
        }
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn dumpProtos(
    mut D: *mut DumpState,
    mut f: *const Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = (*f).sizep;
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        dumpFunction(D, *((*f).p).offset(i as isize), (*f).source)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn dumpUpvalues(
    mut D: *mut DumpState,
    mut f: *const Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = (*f).sizeupvalues;
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        dumpByte(
            D,
            (*((*f).upvalues).offset(i as isize)).instack as libc::c_int,
        )?;
        dumpByte(D, (*((*f).upvalues).offset(i as isize)).idx as libc::c_int)?;
        dumpByte(D, (*((*f).upvalues).offset(i as isize)).kind as libc::c_int)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn dumpDebug(
    mut D: *mut DumpState,
    mut f: *const Proto,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = 0;
    n = if (*D).strip != 0 {
        0 as libc::c_int
    } else {
        (*f).sizelineinfo
    };
    dumpInt(D, n)?;
    dumpBlock(
        D,
        (*f).lineinfo as *const libc::c_void,
        (n as usize).wrapping_mul(::core::mem::size_of::<i8>()),
    )?;
    n = if (*D).strip != 0 {
        0 as libc::c_int
    } else {
        (*f).sizeabslineinfo
    };
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        dumpInt(D, (*((*f).abslineinfo).offset(i as isize)).pc)?;
        dumpInt(D, (*((*f).abslineinfo).offset(i as isize)).line)?;
        i += 1;
        i;
    }
    n = if (*D).strip != 0 {
        0 as libc::c_int
    } else {
        (*f).sizelocvars
    };
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        dumpString(D, (*((*f).locvars).offset(i as isize)).varname)?;
        dumpInt(D, (*((*f).locvars).offset(i as isize)).startpc)?;
        dumpInt(D, (*((*f).locvars).offset(i as isize)).endpc)?;
        i += 1;
        i;
    }
    n = if (*D).strip != 0 {
        0 as libc::c_int
    } else {
        (*f).sizeupvalues
    };
    dumpInt(D, n)?;
    i = 0 as libc::c_int;
    while i < n {
        dumpString(D, (*((*f).upvalues).offset(i as isize)).name)?;
        i += 1;
        i;
    }
    Ok(())
}

unsafe fn dumpFunction(
    mut D: *mut DumpState,
    mut f: *const Proto,
    mut psource: *mut TString,
) -> Result<(), Box<dyn std::error::Error>> {
    if (*D).strip != 0 || (*f).source == psource {
        dumpString(D, 0 as *const TString)?;
    } else {
        dumpString(D, (*f).source)?;
    }
    dumpInt(D, (*f).linedefined)?;
    dumpInt(D, (*f).lastlinedefined)?;
    dumpByte(D, (*f).numparams as libc::c_int)?;
    dumpByte(D, (*f).is_vararg as libc::c_int)?;
    dumpByte(D, (*f).maxstacksize as libc::c_int)?;
    dumpCode(D, f)?;
    dumpConstants(D, f)?;
    dumpUpvalues(D, f)?;
    dumpProtos(D, f)?;
    dumpDebug(D, f)
}

unsafe fn dumpHeader(mut D: *mut DumpState) -> Result<(), Box<dyn std::error::Error>> {
    dumpBlock(
        D,
        b"\x1BLua\0" as *const u8 as *const libc::c_char as *const libc::c_void,
        ::core::mem::size_of::<[libc::c_char; 5]>()
            .wrapping_sub(::core::mem::size_of::<libc::c_char>()),
    )?;
    dumpByte(
        D,
        504 as libc::c_int / 100 as libc::c_int * 16 as libc::c_int
            + 504 as libc::c_int % 100 as libc::c_int,
    )?;
    dumpByte(D, 0 as libc::c_int)?;
    dumpBlock(
        D,
        b"\x19\x93\r\n\x1A\n\0" as *const u8 as *const libc::c_char as *const libc::c_void,
        ::core::mem::size_of::<[libc::c_char; 7]>()
            .wrapping_sub(::core::mem::size_of::<libc::c_char>()),
    )?;
    dumpByte(
        D,
        ::core::mem::size_of::<u32>() as libc::c_ulong as libc::c_int,
    )?;
    dumpByte(
        D,
        ::core::mem::size_of::<i64>() as libc::c_ulong as libc::c_int,
    )?;
    dumpByte(
        D,
        ::core::mem::size_of::<f64>() as libc::c_ulong as libc::c_int,
    )?;
    dumpInteger(D, 0x5678 as libc::c_int as i64)?;
    dumpNumber(D, 370.5f64)
}

pub unsafe fn luaU_dump(
    L: *mut lua_State,
    f: *const Proto,
    writer: lua_Writer,
    data: *mut c_void,
    strip: c_int,
) -> Result<c_int, Box<dyn std::error::Error>> {
    let mut D = DumpState {
        L,
        writer,
        data,
        strip,
        status: 0,
    };

    dumpHeader(&mut D)?;
    dumpByte(&mut D, (*f).sizeupvalues)?;
    dumpFunction(&mut D, f, null_mut())?;

    return Ok(D.status);
}
