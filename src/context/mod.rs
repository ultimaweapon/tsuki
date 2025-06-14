use crate::{Ref, Str, Thread};
use alloc::boxed::Box;
use core::ops::Deref;

/// Context to invoke Rust function.
pub struct Context {
    th: *const Thread,
    args: usize,
}

impl Context {
    #[inline(always)]
    pub(crate) fn new(th: *const Thread, args: usize) -> Self {
        Self { th, args }
    }

    /// Returns `true` if this call has no arguments.
    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        self.args == 0
    }

    /// Returns a number of arguments for this call.
    #[inline(always)]
    pub fn len(&self) -> usize {
        self.args
    }

    /// Converts argument `i` to Lua string and return it.
    ///
    /// Note that `i` is **zero-based**, not one.
    ///
    /// This has the same semantic as `luaL_tolstring`.
    ///
    /// # Panics
    /// If `i` greater or equal [`Self::len()`].
    pub fn to_str(&self, i: usize) -> Result<Ref<Str>, Box<dyn core::error::Error>> {
        todo!()
    }
}

/// Context to invoke Rust yield function.
pub struct YieldContext<'a>(&'a Context);

impl<'a> Deref for YieldContext<'a> {
    type Target = Context;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

/// Context to invoke Rust async function.
pub struct AsyncContext<'a>(&'a Context);

impl<'a> Deref for AsyncContext<'a> {
    type Target = Context;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.0
    }
}
