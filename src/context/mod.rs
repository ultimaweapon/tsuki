use crate::Thread;
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

    /// Returns a number of arguments for this call.
    #[inline(always)]
    pub fn args(&self) -> usize {
        self.args
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
