#![allow(
    dead_code,
    mutable_transmutes,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unused_assignments,
    unused_mut
)]

unsafe extern "C" fn getco(mut L: *mut lua_State) -> *mut lua_State {
    let mut co: *mut lua_State = lua_tothread(L, 1 as libc::c_int);
    ((co != 0 as *mut lua_State) as libc::c_int as libc::c_long != 0
        || luaL_typeerror(
            L,
            1 as libc::c_int,
            b"thread\0" as *const u8 as *const libc::c_char,
        ) != 0) as libc::c_int;
    return co;
}
unsafe extern "C" fn luaB_auxwrap(mut L: *mut lua_State) -> libc::c_int {
    let mut co: *mut lua_State = lua_tothread(
        L,
        -(1000000 as libc::c_int) - 1000 as libc::c_int - 1 as libc::c_int,
    );
    let mut r: libc::c_int = auxresume(L, co, lua_gettop(L));
    if ((r < 0 as libc::c_int) as libc::c_int != 0 as libc::c_int) as libc::c_int as libc::c_long
        != 0
    {
        let mut stat: libc::c_int = lua_status(co);
        if stat != 0 as libc::c_int && stat != 1 as libc::c_int {
            stat = lua_closethread(co, L);
            lua_xmove(co, L, 1 as libc::c_int);
        }
        if stat != 4 as libc::c_int && lua_type(L, -(1 as libc::c_int)) == 4 as libc::c_int {
            luaL_where(L, 1 as libc::c_int);
            lua_rotate(L, -(2 as libc::c_int), 1 as libc::c_int);
            lua_concat(L, 2 as libc::c_int);
        }
        return lua_error(L);
    }
    return r;
}
unsafe extern "C" fn luaB_cowrap(mut L: *mut lua_State) -> libc::c_int {
    luaB_cocreate(L);
    lua_pushcclosure(
        L,
        Some(luaB_auxwrap as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
        1 as libc::c_int,
    );
    return 1 as libc::c_int;
}
unsafe extern "C" fn luaB_yield(mut L: *mut lua_State) -> libc::c_int {
    return lua_yieldk(L, lua_gettop(L), 0 as libc::c_int as lua_KContext, None);
}
static mut statname: [*const libc::c_char; 4] = [
    b"running\0" as *const u8 as *const libc::c_char,
    b"dead\0" as *const u8 as *const libc::c_char,
    b"suspended\0" as *const u8 as *const libc::c_char,
    b"normal\0" as *const u8 as *const libc::c_char,
];
unsafe extern "C" fn luaB_close(mut L: *mut lua_State) -> libc::c_int {
    let mut co: *mut lua_State = getco(L);
    let mut status: libc::c_int = auxstatus(L, co);
    match status {
        1 | 2 => {
            status = lua_closethread(co, L);
            if status == 0 as libc::c_int {
                lua_pushboolean(L, 1 as libc::c_int);
                return 1 as libc::c_int;
            } else {
                lua_pushboolean(L, 0 as libc::c_int);
                lua_xmove(co, L, 1 as libc::c_int);
                return 2 as libc::c_int;
            }
        }
        _ => {
            return luaL_error(
                L,
                b"cannot close a %s coroutine\0" as *const u8 as *const libc::c_char,
                statname[status as usize],
            );
        }
    };
}
static mut co_funcs: [luaL_Reg; 9] = unsafe {
    [
        {
            let mut init = luaL_Reg {
                name: b"wrap\0" as *const u8 as *const libc::c_char,
                func: Some(luaB_cowrap as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"yield\0" as *const u8 as *const libc::c_char,
                func: Some(luaB_yield as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
        {
            let mut init = luaL_Reg {
                name: b"close\0" as *const u8 as *const libc::c_char,
                func: Some(luaB_close as unsafe extern "C" fn(*mut lua_State) -> libc::c_int),
            };
            init
        },
    ]
};
