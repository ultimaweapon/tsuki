#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]

pub type __uint32_t = libc::c_uint;
pub type __int64_t = libc::c_longlong;
pub type __darwin_ct_rune_t = libc::c_int;
pub type __darwin_wchar_t = libc::c_int;
pub type __darwin_rune_t = __darwin_wchar_t;
pub type __darwin_off_t = __int64_t;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct _RuneEntry {
    pub __min: __darwin_rune_t,
    pub __max: __darwin_rune_t,
    pub __map: __darwin_rune_t,
    pub __types: *mut __uint32_t,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct _RuneRange {
    pub __nranges: libc::c_int,
    pub __ranges: *mut _RuneEntry,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct _RuneCharClass {
    pub __name: [libc::c_char; 14],
    pub __mask: __uint32_t,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct _RuneLocale {
    pub __magic: [libc::c_char; 8],
    pub __encoding: [libc::c_char; 32],
    pub __sgetrune: Option<
        unsafe extern "C" fn(
            *const libc::c_char,
            usize,
            *mut *const libc::c_char,
        ) -> __darwin_rune_t,
    >,
    pub __sputrune: Option<
        unsafe extern "C" fn(
            __darwin_rune_t,
            *mut libc::c_char,
            usize,
            *mut *mut libc::c_char,
        ) -> libc::c_int,
    >,
    pub __invalid_rune: __darwin_rune_t,
    pub __runetype: [__uint32_t; 256],
    pub __maplower: [__darwin_rune_t; 256],
    pub __mapupper: [__darwin_rune_t; 256],
    pub __runetype_ext: _RuneRange,
    pub __maplower_ext: _RuneRange,
    pub __mapupper_ext: _RuneRange,
    pub __variable: *mut libc::c_void,
    pub __variable_len: libc::c_int,
    pub __ncharclasses: libc::c_int,
    pub __charclasses: *mut _RuneCharClass,
}
#[derive(Copy, Clone)]
#[repr(C)]
pub struct lconv {
    pub decimal_point: *mut libc::c_char,
    pub thousands_sep: *mut libc::c_char,
    pub grouping: *mut libc::c_char,
    pub int_curr_symbol: *mut libc::c_char,
    pub currency_symbol: *mut libc::c_char,
    pub mon_decimal_point: *mut libc::c_char,
    pub mon_thousands_sep: *mut libc::c_char,
    pub mon_grouping: *mut libc::c_char,
    pub positive_sign: *mut libc::c_char,
    pub negative_sign: *mut libc::c_char,
    pub int_frac_digits: libc::c_char,
    pub frac_digits: libc::c_char,
    pub p_cs_precedes: libc::c_char,
    pub p_sep_by_space: libc::c_char,
    pub n_cs_precedes: libc::c_char,
    pub n_sep_by_space: libc::c_char,
    pub p_sign_posn: libc::c_char,
    pub n_sign_posn: libc::c_char,
    pub int_p_cs_precedes: libc::c_char,
    pub int_n_cs_precedes: libc::c_char,
    pub int_p_sep_by_space: libc::c_char,
    pub int_n_sep_by_space: libc::c_char,
    pub int_p_sign_posn: libc::c_char,
    pub int_n_sign_posn: libc::c_char,
}

pub type fpos_t = __darwin_off_t;
pub type off_t = __darwin_off_t;

#[derive(Copy, Clone)]
#[repr(C)]
pub struct luaL_Stream {
    pub f: *mut FILE,
    pub closef: lua_CFunction,
}
pub type LStream = luaL_Stream;
#[derive(Copy, Clone)]
#[repr(C)]
pub struct RN {
    pub f: *mut FILE,
    pub c: libc::c_int,
    pub n: libc::c_int,
    pub buff: [libc::c_char; 201],
}
#[inline]
unsafe extern "C" fn isascii(mut _c: libc::c_int) -> libc::c_int {
    return (_c & !(0x7f as libc::c_int) == 0 as libc::c_int) as libc::c_int;
}
#[inline]
unsafe extern "C" fn __istype(mut _c: __darwin_ct_rune_t, mut _f: libc::c_ulong) -> libc::c_int {
    return if isascii(_c) != 0 {
        (_DefaultRuneLocale.__runetype[_c as usize] as libc::c_ulong & _f != 0) as libc::c_int
    } else {
        (__maskrune(_c, _f) != 0) as libc::c_int
    };
}
#[inline]
unsafe extern "C" fn __isctype(
    mut _c: __darwin_ct_rune_t,
    mut _f: libc::c_ulong,
) -> __darwin_ct_rune_t {
    return if _c < 0 as libc::c_int || _c >= (1 as libc::c_int) << 8 as libc::c_int {
        0 as libc::c_int
    } else {
        (_DefaultRuneLocale.__runetype[_c as usize] as libc::c_ulong & _f != 0) as libc::c_int
    };
}
#[no_mangle]
#[inline]
#[linkage = "external"]
pub unsafe extern "C" fn isdigit(mut _c: libc::c_int) -> libc::c_int {
    return __isctype(_c, 0x400 as libc::c_long as libc::c_ulong);
}
#[no_mangle]
#[inline]
#[linkage = "external"]
pub unsafe extern "C" fn isspace(mut _c: libc::c_int) -> libc::c_int {
    return __istype(_c, 0x4000 as libc::c_long as libc::c_ulong);
}
#[no_mangle]
#[inline]
#[linkage = "external"]
pub unsafe extern "C" fn isxdigit(mut _c: libc::c_int) -> libc::c_int {
    return __isctype(_c, 0x10000 as libc::c_long as libc::c_ulong);
}
unsafe extern "C" fn l_checkmode(mut mode: *const libc::c_char) -> libc::c_int {
    return (*mode as libc::c_int != '\0' as i32
        && {
            let fresh0 = mode;
            mode = mode.offset(1);
            !(strchr(
                b"rwa\0" as *const u8 as *const libc::c_char,
                *fresh0 as libc::c_int,
            ))
            .is_null()
        }
        && (*mode as libc::c_int != '+' as i32 || {
            mode = mode.offset(1);
            mode;
            1 as libc::c_int != 0
        })
        && strspn(mode, b"b\0" as *const u8 as *const libc::c_char) == strlen(mode))
        as libc::c_int;
}
unsafe extern "C" fn io_type(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = 0 as *mut LStream;
    luaL_checkany(L, 1 as libc::c_int);
    p = luaL_testudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    if p.is_null() {
        lua_pushnil(L);
    } else if ((*p).closef).is_none() {
        lua_pushstring(L, b"closed file\0" as *const u8 as *const libc::c_char);
    } else {
        lua_pushstring(L, b"file\0" as *const u8 as *const libc::c_char);
    }
    return 1 as libc::c_int;
}
unsafe extern "C" fn f_tostring(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    if ((*p).closef).is_none() {
        lua_pushstring(L, b"file (closed)\0" as *const u8 as *const libc::c_char);
    } else {
        lua_pushfstring(
            L,
            b"file (%p)\0" as *const u8 as *const libc::c_char,
            (*p).f,
        );
    }
    return 1 as libc::c_int;
}
unsafe extern "C" fn tofile(mut L: *mut lua_State) -> *mut FILE {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    if (((*p).closef).is_none() as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            b"attempt to use a closed file\0" as *const u8 as *const libc::c_char,
        );
    }
    return (*p).f;
}
unsafe extern "C" fn newprefile(mut L: *mut lua_State) -> *mut LStream {
    let mut p: *mut LStream = lua_newuserdatauv(
        L,
        ::core::mem::size_of::<LStream>() as libc::c_ulong,
        0 as libc::c_int,
    ) as *mut LStream;
    (*p).closef = None;
    luaL_setmetatable(L, b"FILE*\0" as *const u8 as *const libc::c_char);
    return p;
}
unsafe extern "C" fn aux_close(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    let mut cf: lua_CFunction = (*p).closef;
    (*p).closef = None;
    return (Some(cf.expect("non-null function pointer"))).expect("non-null function pointer")(L);
}
unsafe extern "C" fn f_close(mut L: *mut lua_State) -> libc::c_int {
    tofile(L);
    return aux_close(L);
}
unsafe extern "C" fn io_close(mut L: *mut lua_State) -> libc::c_int {
    if lua_type(L, 1 as libc::c_int) == -(1 as libc::c_int) {
        lua_getfield(
            L,
            -(1000000 as libc::c_int) - 1000 as libc::c_int,
            b"_IO_output\0" as *const u8 as *const libc::c_char,
        );
    }
    return f_close(L);
}
unsafe extern "C" fn f_gc(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    if ((*p).closef).is_some() && !((*p).f).is_null() {
        aux_close(L);
    }
    return 0 as libc::c_int;
}
unsafe extern "C" fn io_fclose(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    *__error() = 0 as libc::c_int;
    return luaL_fileresult(
        L,
        (fclose((*p).f) == 0 as libc::c_int) as libc::c_int,
        0 as *const libc::c_char,
    );
}
unsafe extern "C" fn newfile(mut L: *mut lua_State) -> *mut LStream {
    let mut p: *mut LStream = newprefile(L);
    (*p).f = 0 as *mut FILE;
    (*p).closef = Some(io_fclose as unsafe extern "C" fn(*mut lua_State) -> libc::c_int);
    return p;
}
unsafe extern "C" fn opencheck(
    mut L: *mut lua_State,
    mut fname: *const libc::c_char,
    mut mode: *const libc::c_char,
) {
    let mut p: *mut LStream = newfile(L);
    (*p).f = fopen(fname, mode);
    if (((*p).f == 0 as *mut libc::c_void as *mut FILE) as libc::c_int != 0 as libc::c_int)
        as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            b"cannot open file '%s' (%s)\0" as *const u8 as *const libc::c_char,
            fname,
            strerror(*__error()),
        );
    }
}
unsafe extern "C" fn io_open(mut L: *mut lua_State) -> libc::c_int {
    let mut filename: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize);
    let mut mode: *const libc::c_char = luaL_optlstring(
        L,
        2 as libc::c_int,
        b"r\0" as *const u8 as *const libc::c_char,
        0 as *mut usize,
    );
    let mut p: *mut LStream = newfile(L);
    let mut md: *const libc::c_char = mode;
    ((l_checkmode(md) != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(
            L,
            2 as libc::c_int,
            b"invalid mode\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    *__error() = 0 as libc::c_int;
    (*p).f = fopen(filename, mode);
    return if ((*p).f).is_null() {
        luaL_fileresult(L, 0 as libc::c_int, filename)
    } else {
        1 as libc::c_int
    };
}
unsafe extern "C" fn io_pclose(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    *__error() = 0 as libc::c_int;
    return luaL_execresult(L, pclose((*p).f));
}
unsafe extern "C" fn io_popen(mut L: *mut lua_State) -> libc::c_int {
    let mut filename: *const libc::c_char = luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize);
    let mut mode: *const libc::c_char = luaL_optlstring(
        L,
        2 as libc::c_int,
        b"r\0" as *const u8 as *const libc::c_char,
        0 as *mut usize,
    );
    let mut p: *mut LStream = newprefile(L);
    ((((*mode.offset(0 as libc::c_int as isize) as libc::c_int == 'r' as i32
        || *mode.offset(0 as libc::c_int as isize) as libc::c_int == 'w' as i32)
        && *mode.offset(1 as libc::c_int as isize) as libc::c_int == '\0' as i32)
        as libc::c_int
        != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(
            L,
            2 as libc::c_int,
            b"invalid mode\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    *__error() = 0 as libc::c_int;
    fflush(0 as *mut FILE);
    (*p).f = popen(filename, mode);
    (*p).closef = Some(io_pclose as unsafe extern "C" fn(*mut lua_State) -> libc::c_int);
    return if ((*p).f).is_null() {
        luaL_fileresult(L, 0 as libc::c_int, filename)
    } else {
        1 as libc::c_int
    };
}
unsafe extern "C" fn io_tmpfile(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = newfile(L);
    *__error() = 0 as libc::c_int;
    (*p).f = tmpfile();
    return if ((*p).f).is_null() {
        luaL_fileresult(L, 0 as libc::c_int, 0 as *const libc::c_char)
    } else {
        1 as libc::c_int
    };
}
unsafe extern "C" fn getiofile(
    mut L: *mut lua_State,
    mut findex: *const libc::c_char,
) -> *mut FILE {
    let mut p: *mut LStream = 0 as *mut LStream;
    lua_getfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, findex);
    p = lua_touserdata(L, -(1 as libc::c_int)) as *mut LStream;
    if (((*p).closef).is_none() as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        luaL_error(
            L,
            b"default %s file is closed\0" as *const u8 as *const libc::c_char,
            findex.offset(
                (::core::mem::size_of::<[libc::c_char; 5]>() as libc::c_ulong)
                    .wrapping_div(::core::mem::size_of::<libc::c_char>() as libc::c_ulong)
                    .wrapping_sub(1 as libc::c_int as libc::c_ulong) as isize,
            ),
        );
    }
    return (*p).f;
}
unsafe extern "C" fn g_iofile(
    mut L: *mut lua_State,
    mut f: *const libc::c_char,
    mut mode: *const libc::c_char,
) -> libc::c_int {
    if !(lua_type(L, 1 as libc::c_int) <= 0 as libc::c_int) {
        let mut filename: *const libc::c_char = lua_tolstring(L, 1 as libc::c_int, 0 as *mut usize);
        if !filename.is_null() {
            opencheck(L, filename, mode);
        } else {
            tofile(L);
            lua_pushvalue(L, 1 as libc::c_int);
        }
        lua_setfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, f);
    }
    lua_getfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, f);
    return 1 as libc::c_int;
}
unsafe extern "C" fn io_input(mut L: *mut lua_State) -> libc::c_int {
    return g_iofile(
        L,
        b"_IO_input\0" as *const u8 as *const libc::c_char,
        b"r\0" as *const u8 as *const libc::c_char,
    );
}
unsafe extern "C" fn io_output(mut L: *mut lua_State) -> libc::c_int {
    return g_iofile(
        L,
        b"_IO_output\0" as *const u8 as *const libc::c_char,
        b"w\0" as *const u8 as *const libc::c_char,
    );
}
unsafe extern "C" fn aux_lines(mut L: *mut lua_State, mut toclose: libc::c_int) {
    let mut n: libc::c_int = lua_gettop(L) - 1 as libc::c_int;
    (((n <= 250 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
        || luaL_argerror(
            L,
            250 as libc::c_int + 2 as libc::c_int,
            b"too many arguments\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    lua_pushvalue(L, 1 as libc::c_int);
    lua_pushinteger(L, n as i64);
    lua_pushboolean(L, toclose);
    lua_rotate(L, 2 as libc::c_int, 3 as libc::c_int);
    lua_pushcclosure(
        L,
        Some(io_readline as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
        3 as libc::c_int + n,
    );
}
unsafe extern "C" fn f_lines(mut L: *mut lua_State) -> libc::c_int {
    tofile(L);
    aux_lines(L, 0 as libc::c_int);
    return 1 as libc::c_int;
}
unsafe extern "C" fn io_lines(mut L: *mut lua_State) -> libc::c_int {
    let mut toclose: libc::c_int = 0;
    if lua_type(L, 1 as libc::c_int) == -(1 as libc::c_int) {
        lua_pushnil(L);
    }
    if lua_type(L, 1 as libc::c_int) == 0 as libc::c_int {
        lua_getfield(
            L,
            -(1000000 as libc::c_int) - 1000 as libc::c_int,
            b"_IO_input\0" as *const u8 as *const libc::c_char,
        );
        lua_copy(L, -(1 as libc::c_int), 1 as libc::c_int);
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        tofile(L);
        toclose = 0 as libc::c_int;
    } else {
        let mut filename: *const libc::c_char =
            luaL_checklstring(L, 1 as libc::c_int, 0 as *mut usize);
        opencheck(L, filename, b"r\0" as *const u8 as *const libc::c_char);
        lua_copy(L, -(1 as libc::c_int), 1 as libc::c_int);
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        toclose = 1 as libc::c_int;
    }
    aux_lines(L, toclose);
    if toclose != 0 {
        lua_pushnil(L);
        lua_pushnil(L);
        lua_pushvalue(L, 1 as libc::c_int);
        return 4 as libc::c_int;
    } else {
        return 1 as libc::c_int;
    };
}
unsafe extern "C" fn nextc(mut rn: *mut RN) -> libc::c_int {
    if (((*rn).n >= 200 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int
        as libc::c_long
        != 0
    {
        (*rn).buff[0 as libc::c_int as usize] = '\0' as i32 as libc::c_char;
        return 0 as libc::c_int;
    } else {
        let fresh1 = (*rn).n;
        (*rn).n = (*rn).n + 1;
        (*rn).buff[fresh1 as usize] = (*rn).c as libc::c_char;
        (*(*rn).f)._r -= 1;
        (*rn).c = if (*(*rn).f)._r < 0 as libc::c_int {
            __srget((*rn).f)
        } else {
            let fresh2 = (*(*rn).f)._p;
            (*(*rn).f)._p = ((*(*rn).f)._p).offset(1);
            *fresh2 as libc::c_int
        };
        return 1 as libc::c_int;
    };
}
unsafe extern "C" fn test2(mut rn: *mut RN, mut set: *const libc::c_char) -> libc::c_int {
    if (*rn).c == *set.offset(0 as libc::c_int as isize) as libc::c_int
        || (*rn).c == *set.offset(1 as libc::c_int as isize) as libc::c_int
    {
        return nextc(rn);
    } else {
        return 0 as libc::c_int;
    };
}
unsafe extern "C" fn readdigits(mut rn: *mut RN, mut hex: libc::c_int) -> libc::c_int {
    let mut count: libc::c_int = 0 as libc::c_int;
    while (if hex != 0 {
        isxdigit((*rn).c)
    } else {
        isdigit((*rn).c)
    }) != 0
        && nextc(rn) != 0
    {
        count += 1;
        count;
    }
    return count;
}
unsafe extern "C" fn read_number(mut L: *mut lua_State, mut f: *mut FILE) -> libc::c_int {
    let mut rn: RN = RN {
        f: 0 as *mut FILE,
        c: 0,
        n: 0,
        buff: [0; 201],
    };
    let mut count: libc::c_int = 0 as libc::c_int;
    let mut hex: libc::c_int = 0 as libc::c_int;
    let mut decp: [libc::c_char; 2] = [0; 2];
    rn.f = f;
    rn.n = 0 as libc::c_int;
    decp[0 as libc::c_int as usize] =
        *((*localeconv()).decimal_point).offset(0 as libc::c_int as isize);
    decp[1 as libc::c_int as usize] = '.' as i32 as libc::c_char;
    flockfile(rn.f);
    loop {
        (*rn.f)._r -= 1;
        rn.c = if (*rn.f)._r < 0 as libc::c_int {
            __srget(rn.f)
        } else {
            let fresh3 = (*rn.f)._p;
            (*rn.f)._p = ((*rn.f)._p).offset(1);
            *fresh3 as libc::c_int
        };
        if !(isspace(rn.c) != 0) {
            break;
        }
    }
    test2(&mut rn, b"-+\0" as *const u8 as *const libc::c_char);
    if test2(&mut rn, b"00\0" as *const u8 as *const libc::c_char) != 0 {
        if test2(&mut rn, b"xX\0" as *const u8 as *const libc::c_char) != 0 {
            hex = 1 as libc::c_int;
        } else {
            count = 1 as libc::c_int;
        }
    }
    count += readdigits(&mut rn, hex);
    if test2(&mut rn, decp.as_mut_ptr()) != 0 {
        count += readdigits(&mut rn, hex);
    }
    if count > 0 as libc::c_int
        && test2(
            &mut rn,
            (if hex != 0 {
                b"pP\0" as *const u8 as *const libc::c_char
            } else {
                b"eE\0" as *const u8 as *const libc::c_char
            }),
        ) != 0
    {
        test2(&mut rn, b"-+\0" as *const u8 as *const libc::c_char);
        readdigits(&mut rn, 0 as libc::c_int);
    }
    ungetc(rn.c, rn.f);
    funlockfile(rn.f);
    rn.buff[rn.n as usize] = '\0' as i32 as libc::c_char;
    if (lua_stringtonumber(L, (rn.buff).as_mut_ptr()) != 0 as libc::c_int as usize) as libc::c_int
        as libc::c_long
        != 0
    {
        return 1 as libc::c_int;
    } else {
        lua_pushnil(L);
        return 0 as libc::c_int;
    };
}
unsafe extern "C" fn test_eof(mut L: *mut lua_State, mut f: *mut FILE) -> libc::c_int {
    let mut c: libc::c_int = getc(f);
    ungetc(c, f);
    lua_pushstring(L, b"\0" as *const u8 as *const libc::c_char);
    return (c != -(1 as libc::c_int)) as libc::c_int;
}
unsafe extern "C" fn read_line(
    mut L: *mut lua_State,
    mut f: *mut FILE,
    mut chop: libc::c_int,
) -> libc::c_int {
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
        init: C2RustUnnamed { n: 0. },
    };
    let mut c: libc::c_int = 0;
    luaL_buffinit(L, &mut b);
    loop {
        let mut buff: *mut libc::c_char = luaL_prepbuffsize(
            &mut b,
            (16 as libc::c_int as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong)
                as libc::c_int as usize,
        );
        let mut i: libc::c_int = 0 as libc::c_int;
        flockfile(f);
        while i
            < (16 as libc::c_int as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong)
                as libc::c_int
            && {
                (*f)._r -= 1;
                c = (if (*f)._r < 0 as libc::c_int {
                    __srget(f)
                } else {
                    let fresh4 = (*f)._p;
                    (*f)._p = ((*f)._p).offset(1);
                    *fresh4 as libc::c_int
                });
                c != -(1 as libc::c_int)
            }
            && c != '\n' as i32
        {
            let fresh5 = i;
            i = i + 1;
            *buff.offset(fresh5 as isize) = c as libc::c_char;
        }
        funlockfile(f);
        b.n = (b.n).wrapping_add(i as usize);
        if !(c != -(1 as libc::c_int) && c != '\n' as i32) {
            break;
        }
    }
    if chop == 0 && c == '\n' as i32 {
        (b.n < b.size || !(luaL_prepbuffsize(&mut b, 1 as libc::c_int as usize)).is_null())
            as libc::c_int;
        let fresh6 = b.n;
        b.n = (b.n).wrapping_add(1);
        *(b.b).offset(fresh6 as isize) = c as libc::c_char;
    }
    luaL_pushresult(&mut b);
    return (c == '\n' as i32 || lua_rawlen(L, -(1 as libc::c_int)) > 0 as libc::c_int as u64)
        as libc::c_int;
}
unsafe extern "C" fn read_all(mut L: *mut lua_State, mut f: *mut FILE) {
    let mut nr: usize = 0;
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
        init: C2RustUnnamed { n: 0. },
    };
    luaL_buffinit(L, &mut b);
    loop {
        let mut p: *mut libc::c_char = luaL_prepbuffsize(
            &mut b,
            (16 as libc::c_int as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong)
                as libc::c_int as usize,
        );
        nr = fread(
            p as *mut libc::c_void,
            ::core::mem::size_of::<libc::c_char>() as libc::c_ulong,
            (16 as libc::c_int as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong)
                as libc::c_int as libc::c_ulong,
            f,
        );
        b.n = (b.n).wrapping_add(nr);
        if !(nr
            == (16 as libc::c_int as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
                .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong)
                as libc::c_int as usize)
        {
            break;
        }
    }
    luaL_pushresult(&mut b);
}
unsafe extern "C" fn read_chars(
    mut L: *mut lua_State,
    mut f: *mut FILE,
    mut n: usize,
) -> libc::c_int {
    let mut nr: usize = 0;
    let mut p: *mut libc::c_char = 0 as *mut libc::c_char;
    let mut b: luaL_Buffer = luaL_Buffer {
        b: 0 as *mut libc::c_char,
        size: 0,
        n: 0,
        L: 0 as *mut lua_State,
        init: C2RustUnnamed { n: 0. },
    };
    luaL_buffinit(L, &mut b);
    p = luaL_prepbuffsize(&mut b, n);
    nr = fread(
        p as *mut libc::c_void,
        ::core::mem::size_of::<libc::c_char>() as libc::c_ulong,
        n,
        f,
    );
    b.n = (b.n).wrapping_add(nr);
    luaL_pushresult(&mut b);
    return (nr > 0 as libc::c_int as usize) as libc::c_int;
}
unsafe extern "C" fn g_read(
    mut L: *mut lua_State,
    mut f: *mut FILE,
    mut first: libc::c_int,
) -> libc::c_int {
    let mut nargs: libc::c_int = lua_gettop(L) - 1 as libc::c_int;
    let mut n: libc::c_int = 0;
    let mut success: libc::c_int = 0;
    clearerr(f);
    *__error() = 0 as libc::c_int;
    if nargs == 0 as libc::c_int {
        success = read_line(L, f, 1 as libc::c_int);
        n = first + 1 as libc::c_int;
    } else {
        luaL_checkstack(
            L,
            nargs + 20 as libc::c_int,
            b"too many arguments\0" as *const u8 as *const libc::c_char,
        );
        success = 1 as libc::c_int;
        n = first;
        loop {
            let fresh7 = nargs;
            nargs = nargs - 1;
            if !(fresh7 != 0 && success != 0) {
                break;
            }
            if lua_type(L, n) == 3 as libc::c_int {
                let mut l: usize = luaL_checkinteger(L, n) as usize;
                success = if l == 0 as libc::c_int as usize {
                    test_eof(L, f)
                } else {
                    read_chars(L, f, l)
                };
            } else {
                let mut p: *const libc::c_char = luaL_checklstring(L, n, 0 as *mut usize);
                if *p as libc::c_int == '*' as i32 {
                    p = p.offset(1);
                    p;
                }
                match *p as libc::c_int {
                    110 => {
                        success = read_number(L, f);
                    }
                    108 => {
                        success = read_line(L, f, 1 as libc::c_int);
                    }
                    76 => {
                        success = read_line(L, f, 0 as libc::c_int);
                    }
                    97 => {
                        read_all(L, f);
                        success = 1 as libc::c_int;
                    }
                    _ => {
                        return luaL_argerror(
                            L,
                            n,
                            b"invalid format\0" as *const u8 as *const libc::c_char,
                        );
                    }
                }
            }
            n += 1;
            n;
        }
    }
    if ferror(f) != 0 {
        return luaL_fileresult(L, 0 as libc::c_int, 0 as *const libc::c_char);
    }
    if success == 0 {
        lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
        lua_pushnil(L);
    }
    return n - first;
}
unsafe extern "C" fn io_read(mut L: *mut lua_State) -> libc::c_int {
    return g_read(
        L,
        getiofile(L, b"_IO_input\0" as *const u8 as *const libc::c_char),
        1 as libc::c_int,
    );
}
unsafe extern "C" fn f_read(mut L: *mut lua_State) -> libc::c_int {
    return g_read(L, tofile(L), 2 as libc::c_int);
}
unsafe extern "C" fn io_readline(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = lua_touserdata(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
    ) as *mut LStream;
    let mut i: libc::c_int = 0;
    let mut n: libc::c_int = lua_tointegerx(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 2 as libc::c_int,
        0 as *mut libc::c_int,
    ) as libc::c_int;
    if ((*p).closef).is_none() {
        return luaL_error(
            L,
            b"file is already closed\0" as *const u8 as *const libc::c_char,
        );
    }
    lua_settop(L, 1 as libc::c_int);
    luaL_checkstack(
        L,
        n,
        b"too many arguments\0" as *const u8 as *const libc::c_char,
    );
    i = 1 as libc::c_int;
    while i <= n {
        lua_pushvalue(
            L,
            -(1000000 as libc::c_int) - 1000 as libc::c_int - (3 as libc::c_int + i),
        );
        i += 1;
        i;
    }
    n = g_read(L, (*p).f, 2 as libc::c_int);
    if lua_toboolean(L, -n) != 0 {
        return n;
    } else {
        if n > 1 as libc::c_int {
            return luaL_error(
                L,
                b"%s\0" as *const u8 as *const libc::c_char,
                lua_tolstring(L, -n + 1 as libc::c_int, 0 as *mut usize),
            );
        }
        if lua_toboolean(
            L,
            -(1000000 as libc::c_int) - 1000 as libc::c_int - 3 as libc::c_int,
        ) != 0
        {
            lua_settop(L, 0 as libc::c_int);
            lua_pushvalue(
                L,
                -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
            );
            aux_close(L);
        }
        return 0 as libc::c_int;
    };
}
unsafe extern "C" fn g_write(
    mut L: *mut lua_State,
    mut f: *mut FILE,
    mut arg: libc::c_int,
) -> libc::c_int {
    let mut nargs: libc::c_int = lua_gettop(L) - arg;
    let mut status: libc::c_int = 1 as libc::c_int;
    *__error() = 0 as libc::c_int;
    loop {
        let fresh8 = nargs;
        nargs = nargs - 1;
        if !(fresh8 != 0) {
            break;
        }
        if lua_type(L, arg) == 3 as libc::c_int {
            let mut len: libc::c_int = if lua_isinteger(L, arg) != 0 {
                fprintf(
                    f,
                    b"%lld\0" as *const u8 as *const libc::c_char,
                    lua_tointegerx(L, arg, 0 as *mut libc::c_int),
                )
            } else {
                fprintf(
                    f,
                    b"%.14g\0" as *const u8 as *const libc::c_char,
                    lua_tonumberx(L, arg, 0 as *mut libc::c_int),
                )
            };
            status = (status != 0 && len > 0 as libc::c_int) as libc::c_int;
        } else {
            let mut l: usize = 0;
            let mut s: *const libc::c_char = luaL_checklstring(L, arg, &mut l);
            status = (status != 0
                && fwrite(
                    s as *const libc::c_void,
                    ::core::mem::size_of::<libc::c_char>() as libc::c_ulong,
                    l,
                    f,
                ) == l) as libc::c_int;
        }
        arg += 1;
        arg;
    }
    if (status != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        return 1 as libc::c_int;
    } else {
        return luaL_fileresult(L, status, 0 as *const libc::c_char);
    };
}
unsafe extern "C" fn io_write(mut L: *mut lua_State) -> libc::c_int {
    return g_write(
        L,
        getiofile(L, b"_IO_output\0" as *const u8 as *const libc::c_char),
        1 as libc::c_int,
    );
}
unsafe extern "C" fn f_write(mut L: *mut lua_State) -> libc::c_int {
    let mut f: *mut FILE = tofile(L);
    lua_pushvalue(L, 1 as libc::c_int);
    return g_write(L, f, 2 as libc::c_int);
}
unsafe extern "C" fn f_seek(mut L: *mut lua_State) -> libc::c_int {
    static mut mode: [libc::c_int; 3] = [0 as libc::c_int, 1 as libc::c_int, 2 as libc::c_int];
    static mut modenames: [*const libc::c_char; 4] = [
        b"set\0" as *const u8 as *const libc::c_char,
        b"cur\0" as *const u8 as *const libc::c_char,
        b"end\0" as *const u8 as *const libc::c_char,
        0 as *const libc::c_char,
    ];
    let mut f: *mut FILE = tofile(L);
    let mut op: libc::c_int = luaL_checkoption(
        L,
        2 as libc::c_int,
        b"cur\0" as *const u8 as *const libc::c_char,
        modenames.as_ptr(),
    );
    let mut p3: i64 = luaL_optinteger(L, 3 as libc::c_int, 0 as libc::c_int as i64);
    let mut offset: off_t = p3;
    (((offset == p3) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long != 0
        || luaL_argerror(
            L,
            3 as libc::c_int,
            b"not an integer in proper range\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    *__error() = 0 as libc::c_int;
    op = fseeko(f, offset, mode[op as usize]);
    if (op != 0 as libc::c_int) as libc::c_int as libc::c_long != 0 {
        return luaL_fileresult(L, 0 as libc::c_int, 0 as *const libc::c_char);
    } else {
        lua_pushinteger(L, ftello(f));
        return 1 as libc::c_int;
    };
}
unsafe extern "C" fn f_setvbuf(mut L: *mut lua_State) -> libc::c_int {
    static mut mode: [libc::c_int; 3] = [2 as libc::c_int, 0 as libc::c_int, 1 as libc::c_int];
    static mut modenames: [*const libc::c_char; 4] = [
        b"no\0" as *const u8 as *const libc::c_char,
        b"full\0" as *const u8 as *const libc::c_char,
        b"line\0" as *const u8 as *const libc::c_char,
        0 as *const libc::c_char,
    ];
    let mut f: *mut FILE = tofile(L);
    let mut op: libc::c_int = luaL_checkoption(
        L,
        2 as libc::c_int,
        0 as *const libc::c_char,
        modenames.as_ptr(),
    );
    let mut sz: i64 = luaL_optinteger(
        L,
        3 as libc::c_int,
        (16 as libc::c_int as libc::c_ulong)
            .wrapping_mul(::core::mem::size_of::<*mut libc::c_void>() as libc::c_ulong)
            .wrapping_mul(::core::mem::size_of::<f64>() as libc::c_ulong) as libc::c_int
            as i64,
    );
    let mut res: libc::c_int = 0;
    *__error() = 0 as libc::c_int;
    res = setvbuf(f, 0 as *mut libc::c_char, mode[op as usize], sz as usize);
    return luaL_fileresult(
        L,
        (res == 0 as libc::c_int) as libc::c_int,
        0 as *const libc::c_char,
    );
}
unsafe extern "C" fn io_flush(mut L: *mut lua_State) -> libc::c_int {
    let mut f: *mut FILE = getiofile(L, b"_IO_output\0" as *const u8 as *const libc::c_char);
    *__error() = 0 as libc::c_int;
    return luaL_fileresult(
        L,
        (fflush(f) == 0 as libc::c_int) as libc::c_int,
        0 as *const libc::c_char,
    );
}
unsafe extern "C" fn f_flush(mut L: *mut lua_State) -> libc::c_int {
    let mut f: *mut FILE = tofile(L);
    *__error() = 0 as libc::c_int;
    return luaL_fileresult(
        L,
        (fflush(f) == 0 as libc::c_int) as libc::c_int,
        0 as *const libc::c_char,
    );
}
static mut iolib: [luaL_Reg; 12] = unsafe {
    [
        {
            let mut init = luaL_Reg {
                name: b"close\0" as *const u8 as *const libc::c_char,
                func: Some(io_close as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"flush\0" as *const u8 as *const libc::c_char,
                func: Some(io_flush as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"input\0" as *const u8 as *const libc::c_char,
                func: Some(io_input as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"lines\0" as *const u8 as *const libc::c_char,
                func: Some(io_lines as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"open\0" as *const u8 as *const libc::c_char,
                func: Some(io_open as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"output\0" as *const u8 as *const libc::c_char,
                func: Some(io_output as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"popen\0" as *const u8 as *const libc::c_char,
                func: Some(io_popen as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"read\0" as *const u8 as *const libc::c_char,
                func: Some(io_read as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"tmpfile\0" as *const u8 as *const libc::c_char,
                func: Some(io_tmpfile as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"type\0" as *const u8 as *const libc::c_char,
                func: Some(io_type as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"write\0" as *const u8 as *const libc::c_char,
                func: Some(io_write as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: 0 as *const libc::c_char,
                func: None,
            };
            init
        },
    ]
};
static mut meth: [luaL_Reg; 8] = unsafe {
    [
        {
            let mut init = luaL_Reg {
                name: b"read\0" as *const u8 as *const libc::c_char,
                func: Some(f_read as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"write\0" as *const u8 as *const libc::c_char,
                func: Some(f_write as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"lines\0" as *const u8 as *const libc::c_char,
                func: Some(f_lines as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"flush\0" as *const u8 as *const libc::c_char,
                func: Some(f_flush as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"seek\0" as *const u8 as *const libc::c_char,
                func: Some(f_seek as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"close\0" as *const u8 as *const libc::c_char,
                func: Some(f_close as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"setvbuf\0" as *const u8 as *const libc::c_char,
                func: Some(f_setvbuf as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: 0 as *const libc::c_char,
                func: None,
            };
            init
        },
    ]
};
static mut metameth: [luaL_Reg; 5] = unsafe {
    [
        {
            let mut init = luaL_Reg {
                name: b"__index\0" as *const u8 as *const libc::c_char,
                func: None,
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"__gc\0" as *const u8 as *const libc::c_char,
                func: Some(f_gc as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"__close\0" as *const u8 as *const libc::c_char,
                func: Some(f_gc as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"__tostring\0" as *const u8 as *const libc::c_char,
                func: Some(f_tostring as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: 0 as *const libc::c_char,
                func: None,
            };
            init
        },
    ]
};
unsafe extern "C" fn createmeta(mut L: *mut lua_State) {
    luaL_newmetatable(L, b"FILE*\0" as *const u8 as *const libc::c_char);
    luaL_setfuncs(L, metameth.as_ptr(), 0 as libc::c_int);
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 8]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    );
    luaL_setfuncs(L, meth.as_ptr(), 0 as libc::c_int);
    lua_setfield(
        L,
        -(2 as libc::c_int),
        b"__index\0" as *const u8 as *const libc::c_char,
    );
    lua_settop(L, -(1 as libc::c_int) - 1 as libc::c_int);
}
unsafe extern "C" fn io_noclose(mut L: *mut lua_State) -> libc::c_int {
    let mut p: *mut LStream = luaL_checkudata(
        L,
        1 as libc::c_int,
        b"FILE*\0" as *const u8 as *const libc::c_char,
    ) as *mut LStream;
    (*p).closef = Some(io_noclose as unsafe extern "C" fn(*mut lua_State) -> libc::c_int);
    lua_pushnil(L);
    lua_pushstring(
        L,
        b"cannot close standard file\0" as *const u8 as *const libc::c_char,
    );
    return 2 as libc::c_int;
}
unsafe extern "C" fn createstdfile(
    mut L: *mut lua_State,
    mut f: *mut FILE,
    mut k: *const libc::c_char,
    mut fname: *const libc::c_char,
) {
    let mut p: *mut LStream = newprefile(L);
    (*p).f = f;
    (*p).closef = Some(io_noclose as unsafe extern "C" fn(*mut lua_State) -> libc::c_int);
    if !k.is_null() {
        lua_pushvalue(L, -(1 as libc::c_int));
        lua_setfield(L, -(1000000 as libc::c_int) - 1000 as libc::c_int, k);
    }
    lua_setfield(L, -(2 as libc::c_int), fname);
}
#[no_mangle]
pub unsafe extern "C" fn luaopen_io(mut L: *mut lua_State) -> libc::c_int {
    luaL_checkversion_(
        L,
        504 as libc::c_int as f64,
        (::core::mem::size_of::<i64>() as libc::c_ulong)
            .wrapping_mul(16 as libc::c_int as libc::c_ulong)
            .wrapping_add(::core::mem::size_of::<f64>() as libc::c_ulong),
    );
    lua_createtable(
        L,
        0 as libc::c_int,
        (::core::mem::size_of::<[luaL_Reg; 12]>() as libc::c_ulong)
            .wrapping_div(::core::mem::size_of::<luaL_Reg>() as libc::c_ulong)
            .wrapping_sub(1 as libc::c_int as libc::c_ulong) as libc::c_int,
    );
    luaL_setfuncs(L, iolib.as_ptr(), 0 as libc::c_int);
    createmeta(L);
    createstdfile(
        L,
        __stdinp,
        b"_IO_input\0" as *const u8 as *const libc::c_char,
        b"stdin\0" as *const u8 as *const libc::c_char,
    );
    createstdfile(
        L,
        __stdoutp,
        b"_IO_output\0" as *const u8 as *const libc::c_char,
        b"stdout\0" as *const u8 as *const libc::c_char,
    );
    createstdfile(
        L,
        __stderrp,
        0 as *const libc::c_char,
        b"stderr\0" as *const u8 as *const libc::c_char,
    );
    return 1 as libc::c_int;
}
