use crate::{Lua, Module, Ref, Table, fp};
use alloc::boxed::Box;
use core::ops::Deref;

pub mod base;
pub mod math;
pub mod string;
pub mod table;

/// [Module] implementation for [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
///
/// Note that `print` only available with `std` feature.
pub struct BaseModule;

impl<A> Module<A> for BaseModule {
    const NAME: &str = "_G";

    type Instance<'a>
        = &'a Table<A>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let g = lua.global();

        g.set_str_key("assert", fp!(self::base::assert));
        g.set_str_key("error", fp!(self::base::error));
        g.set_str_key("getmetatable", fp!(self::base::getmetatable));
        g.set_str_key("load", fp!(self::base::load));
        g.set_str_key("next", fp!(self::base::next));
        g.set_str_key("pcall", fp!(self::base::pcall));
        #[cfg(feature = "std")]
        g.set_str_key("print", fp!(self::base::print));
        g.set_str_key("rawget", fp!(self::base::rawget));
        g.set_str_key("rawset", fp!(self::base::rawset));
        g.set_str_key("select", fp!(self::base::select));
        g.set_str_key("setmetatable", fp!(self::base::setmetatable));
        g.set_str_key("tostring", fp!(self::base::tostring));
        g.set_str_key("type", fp!(self::base::r#type));

        Ok(g)
    }
}

/// [Module] implementation for [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
///
/// Note that [Self::open()] will **overwrite** string metatable.
pub struct StringModule;

impl<A> Module<A> for StringModule {
    const NAME: &str = "string";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        // Set up module table.
        let g = lua.create_table();

        g.set_str_key("format", fp!(self::string::format));
        g.set_str_key("sub", fp!(self::string::sub));

        // Set up metatable.
        let mt = lua.create_table();

        mt.set_str_key("__add", fp!(self::string::add));
        mt.set_str_key("__index", g.deref());
        mt.set_str_key("__sub", fp!(self::string::subtract));

        lua.set_str_metatable(&mt);

        Ok(g)
    }
}
