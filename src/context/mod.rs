/// Context to invoke Rust function.
pub struct Context {}

/// Context to invoke Rust yield function.
pub struct YieldContext<'a>(&'a mut Context);

/// Context to invoke Rust async function.
pub struct AsyncContext<'a>(&'a mut Context);
