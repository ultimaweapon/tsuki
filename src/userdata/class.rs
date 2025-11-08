use crate::{Lua, Ref, Table};
use core::any::Any;

/// Provides a function to create a metatable for a userdata.
///
/// Use [Lua::register_class()] to register type that implement this trait. You can also use
/// [Lua::register_metatable()] if you need to alter the metatable before register it.
///
/// You can use [Module](crate::Module) if you need a global table for a constructor (e.g.
/// `MyClass:new`).
pub trait Class<A>: Any {
    /// Create a metatable for this type.
    fn create_metatable(lua: &Lua<A>) -> Ref<'_, Table<A>>;
}
