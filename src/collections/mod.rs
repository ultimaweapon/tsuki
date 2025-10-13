//! Provides Rust collection types to store Lua values.
//!
//! # Possible types for value
//!
//! - [Str](crate::Str)
//! - [Table](crate::Table)
//! - [LuaFn](crate::LuaFn)
//! - [UserData](crate::UserData)
//! - [Thread](crate::Thread)
//! - [Dynamic](crate::Dynamic)
pub use self::btree_map::*;

pub(crate) use self::value::*;

use crate::gc::Object;

mod btree_map;
mod value;

/// Header of all collection type.
#[repr(C)]
pub(crate) struct Header<A> {
    pub obj: Object<A>,
    pub ptr: *const dyn Collection,
}

/// Provides methods to mark Lua values in a collection.
pub(crate) trait Collection {
    fn mark_items(&self);
}
