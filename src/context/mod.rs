use crate::Thread;

/// Context to invoke Rust function.
pub struct Context {
    th: *const Thread,
}

impl Context {
    #[inline(always)]
    pub(crate) fn new(th: *const Thread) -> Self {
        Self { th }
    }
}

/// Context to invoke Rust yield function.
pub struct YieldContext<'a>(&'a mut Context);

/// Context to invoke Rust async function.
pub struct AsyncContext<'a>(&'a mut Context);
