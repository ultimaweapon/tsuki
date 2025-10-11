pub(crate) use self::value::*;

mod value;

/// Key to store value on Lua registry.
///
/// The type itself is a key, not value.
pub trait RegKey<A>: 'static {
    /// Type of the value.
    ///
    /// This can be one of the following type:
    ///
    /// - [bool].
    /// - [i8].
    /// - [i16].
    /// - [i32].
    /// - [i64].
    /// - [u8].
    /// - [u16].
    /// - [u32].
    /// - [u64].
    /// - [f32].
    /// - [f64].
    /// - [Ref](crate::Ref) of [Str](crate::Str), [Table](crate::Table), [LuaFn](crate::LuaFn),
    ///   [UserData](crate::UserData) or [Thread](crate::Thread).
    type Value<'a>
    where
        A: 'a;
}
