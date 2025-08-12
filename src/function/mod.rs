use crate::lobject::{Proto, UpVal};
use crate::value::UnsafeValue;
use crate::{Object, Value};
use alloc::boxed::Box;
use core::cell::Cell;
use core::num::NonZero;

/// Lua function.
#[repr(C)]
pub struct LuaFn<D> {
    pub(crate) hdr: Object<D>,
    pub(crate) p: Cell<*mut Proto<D>>,
    pub(crate) upvals: Box<[Cell<*mut UpVal<D>>]>,
}

impl<D> LuaFn<D> {
    /// Set upvalue of this function. Return `v` if `i` is not a valid index.
    ///
    /// # Panics
    /// - If `i` is zero.
    /// - If `v` was created from a different [Lua](crate::Lua) instance.
    pub fn set_upvalue(
        &self,
        i: impl TryInto<NonZero<usize>>,
        v: Value<D>,
    ) -> Result<(), Value<D>> {
        // Check if index valid.
        let i = i.try_into().ok().unwrap().get() - 1;
        let u = match self.upvals.get(i) {
            Some(v) => v.get(),
            None => return Err(v),
        };

        // Check if v come from the same Lua.
        let v = UnsafeValue::from(v);

        if unsafe { v.tt_ & 1 << 6 != 0 && (*v.value_.gc).global != (*u).hdr.global } {
            panic!("attempt to set upvalue with a value created from different Lua instance");
        }

        // Set value.
        unsafe { (*u).v.get().write(v) };

        if unsafe { v.tt_ & 1 << 6 != 0 && (*u).hdr.marked.get() & 1 << 5 != 0 } {
            if unsafe { (*v.value_.gc).marked.get() & (1 << 3 | 1 << 4) != 0 } {
                unsafe { (*u).hdr.global().gc.barrier(u.cast(), v.value_.gc) };
            }
        }

        Ok(())
    }
}
