use crate::{Lua, Value};

/// Lua module to be register with [Builder::add_module](crate::Builder::add_module()).
pub trait Module {
    /// Name of global variable for the value returned from [`Module::register()`].
    ///
    /// Note that the user can override this name.
    const NAME: &str;

    /// Returns [`None`] if the module don't need a global variable for the module itself.
    fn register(self, lua: &Lua) -> Option<Value>;
}
