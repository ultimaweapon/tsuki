use crate::{Lua, Module, Ref, Table, fp};
use alloc::boxed::Box;
use core::ops::Deref;

pub mod base;
pub mod io;
pub mod math;
pub mod string;
pub mod table;

/// [Module] implementation for [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
///
/// Note that `print` only available with `std` feature.
pub struct BaseLib;

impl<A> Module<A> for BaseLib {
    const NAME: &str = "_G";

    type Instance<'a>
        = &'a Table<A>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let m = lua.global();

        m.set_str_key("assert", fp!(self::base::assert));
        m.set_str_key("error", fp!(self::base::error));
        m.set_str_key("getmetatable", fp!(self::base::getmetatable));
        m.set_str_key("load", fp!(self::base::load));
        m.set_str_key("next", fp!(self::base::next));
        m.set_str_key("pcall", fp!(self::base::pcall));
        #[cfg(feature = "std")]
        m.set_str_key("print", fp!(self::base::print));
        m.set_str_key("rawget", fp!(self::base::rawget));
        m.set_str_key("rawset", fp!(self::base::rawset));
        m.set_str_key("select", fp!(self::base::select));
        m.set_str_key("setmetatable", fp!(self::base::setmetatable));
        m.set_str_key("tostring", fp!(self::base::tostring));
        m.set_str_key("type", fp!(self::base::r#type));

        Ok(m)
    }
}

/// [Module] implementation for [coroutine library](https://www.lua.org/manual/5.4/manual.html#6.2).
pub struct CoroLib;

impl<A> Module<A> for CoroLib {
    const NAME: &str = "coroutine";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}

/// [Module] implementation for [I/O library](https://www.lua.org/manual/5.4/manual.html#6.8).
pub struct IoLib;

impl<A> Module<A> for IoLib {
    const NAME: &str = "io";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}

/// [Module] implementation for
/// [mathematical library](https://www.lua.org/manual/5.4/manual.html#6.7).
pub struct MathLib;

impl<A> Module<A> for MathLib {
    const NAME: &str = "math";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        m.set_str_key("floor", fp!(self::math::floor));
        m.set_str_key("log", fp!(self::math::log));
        m.set_str_key("max", fp!(self::math::max));
        m.set_str_key("maxinteger", i64::MAX);
        m.set_str_key("mininteger", i64::MIN);
        m.set_str_key("sin", fp!(self::math::sin));
        m.set_str_key("type", fp!(self::math::r#type));

        Ok(m)
    }
}

/// [Module] implementation for [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
///
/// Note that [Self::open()] will **overwrite** string metatable.
pub struct StringLib;

impl<A> Module<A> for StringLib {
    const NAME: &str = "string";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        // Set up module table.
        let m = lua.create_table();

        m.set_str_key("format", fp!(self::string::format));
        m.set_str_key("sub", fp!(self::string::sub));

        // Set up metatable.
        let mt = lua.create_table();

        mt.set_str_key("__add", fp!(self::string::add));
        mt.set_str_key("__index", m.deref());
        mt.set_str_key("__sub", fp!(self::string::subtract));

        lua.set_str_metatable(&mt);

        Ok(m)
    }
}

/// [Module] implementation for [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
pub struct TableLib;

impl<A> Module<A> for TableLib {
    const NAME: &str = "table";

    type Instance<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Instance<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        m.set_str_key("unpack", fp!(self::table::unpack));

        Ok(m)
    }
}
