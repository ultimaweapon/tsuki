use crate::Lua;
use alloc::boxed::Box;

/// Provides interface for Rust to create a Lua module.
pub trait Module<A> {
    /// Type of module instance.
    ///
    /// The value of this type is the value that will be returned from `require` and also the value
    /// of global variable if user choose to create one for this module.
    ///
    /// This can be anything that can be converted to [UnsafeValue](crate::UnsafeValue). If
    /// [Nil](crate::Nil) the module will not be available on both `require` and global variable. In
    /// this case the module with the same name is allowed.
    type Instance<'a>
    where
        A: 'a;

    /// Open this module on `lua`.
    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>>;
}
