pub use self::arg::*;

use crate::Thread;
use core::num::NonZero;
use core::ops::Deref;

mod arg;

/// Context to invoke Rust function.
pub struct Context {
    th: *const Thread,
    args: usize,
}

impl Context {
    #[inline(always)]
    pub(crate) unsafe fn new(th: *const Thread, args: usize) -> Self {
        Self { th, args }
    }

    /// Returns a number of arguments for this call.
    #[inline(always)]
    pub fn args(&self) -> usize {
        self.args
    }

    /// # Panics
    /// If `n` is zero.
    #[inline(always)]
    pub fn arg(&self, n: impl TryInto<NonZero<usize>>) -> Arg {
        let n = match n.try_into() {
            Ok(v) => v,
            Err(_) => panic!("zero is not a valid argument index"),
        };

        Arg::new(self, n)
    }

    #[inline(always)]
    fn thread(&self) -> &Thread {
        unsafe { &*self.th }
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
