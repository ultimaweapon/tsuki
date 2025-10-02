use crate::{Lua, Value};
use alloc::boxed::Box;

/// Provides interface for Rust to create a Lua module.
pub trait Module<A> {
    /// Open this module on `lua`.
    ///
    /// If the return value is [Value::Nil] no global variable will be created for this module.
    fn open(self: Box<Self>, lua: &Lua<A>) -> Result<Value<'_, A>, Box<dyn core::error::Error>>;
}
