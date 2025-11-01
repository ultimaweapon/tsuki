use crate::value::UnsafeValue;
use crate::{Str, Table, luaH_getstr};
use core::convert::identity;

/// Type that allowed as a key on userdata properties.
///
/// This trait **MUST** never exposed to the outside.
pub trait PropertyKey {
    unsafe fn get<A>(self, t: *const Table<A>) -> *const UnsafeValue<A>;
    unsafe fn set<A>(self, t: *const Table<A>, v: UnsafeValue<A>);
}

impl PropertyKey for &str {
    #[inline(always)]
    unsafe fn get<A>(self, t: *const Table<A>) -> *const UnsafeValue<A> {
        let g = unsafe { (*t).hdr.global() };
        let k = unsafe { Str::from_str(g, self) };
        let v = unsafe { luaH_getstr(t, k.unwrap_or_else(identity)) };

        if k.is_ok() {
            g.gc.step();
        }

        v
    }

    #[inline(always)]
    unsafe fn set<A>(self, t: *const Table<A>, v: UnsafeValue<A>) {
        let g = unsafe { (*t).hdr.global() };
        let s = unsafe { Str::from_str(g, self) };
        let k = unsafe { UnsafeValue::from_obj(s.unwrap_or_else(identity).cast()) };

        // SAFETY: Key was created from the same Lua on the above.
        // SAFETY: Key is a string so error is not possible.
        unsafe { (*t).set_unchecked(k, v).unwrap_unchecked() };

        if s.is_ok() {
            g.gc.step();
        }
    }
}
