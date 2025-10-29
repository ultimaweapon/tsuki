//! Implementation of Lua standard libraries.
use crate::{Lua, Module, Ref, Table, fp};
use alloc::boxed::Box;
use core::ops::Deref;

pub mod base;
pub mod coroutine;
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub mod io;
pub mod math;
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub mod os;
pub mod string;
pub mod table;
pub mod utf8;

/// [Module] implementation for [basic library](https://www.lua.org/manual/5.4/manual.html#6.1).
///
/// Note that `print` only available with `std` feature.
pub struct BaseLib;

impl<A> Module<A> for BaseLib {
    const NAME: &str = "_G";

    type Inst<'a>
        = &'a Table<A>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.global();

        m.set_str_key("assert", fp!(self::base::assert));
        m.set_str_key("error", fp!(self::base::error));
        m.set_str_key("getmetatable", fp!(self::base::getmetatable));
        m.set_str_key("load", fp!(self::base::load));
        m.set_str_key("next", fp!(self::base::next));
        m.set_str_key("pairs", fp!(self::base::pairs));
        m.set_str_key("pcall", fp!(self::base::pcall));
        #[cfg(feature = "std")]
        m.set_str_key("print", fp!(self::base::print));
        m.set_str_key("rawequal", fp!(self::base::rawequal));
        m.set_str_key("rawget", fp!(self::base::rawget));
        m.set_str_key("rawlen", fp!(self::base::rawlen));
        m.set_str_key("rawset", fp!(self::base::rawset));
        m.set_str_key("select", fp!(self::base::select));
        m.set_str_key("setmetatable", fp!(self::base::setmetatable));
        m.set_str_key("tonumber", fp!(self::base::tonumber));
        m.set_str_key("tostring", fp!(self::base::tostring));
        m.set_str_key("type", fp!(self::base::r#type));

        Ok(m)
    }
}

/// [Module] implementation for [coroutine library](https://www.lua.org/manual/5.4/manual.html#6.2).
pub struct CoroLib;

impl<A> Module<A> for CoroLib {
    const NAME: &str = "coroutine";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}

/// [Module] implementation for [I/O library](https://www.lua.org/manual/5.4/manual.html#6.8).
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub struct IoLib;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<A> Module<A> for IoLib {
    const NAME: &str = "io";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}

/// [Module] implementation for
/// [mathematical library](https://www.lua.org/manual/5.4/manual.html#6.7).
pub struct MathLib;

impl<A> Module<A> for MathLib {
    const NAME: &str = "math";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        m.set_str_key("abs", fp!(self::math::abs));
        m.set_str_key("cos", fp!(self::math::cos));
        m.set_str_key("floor", fp!(self::math::floor));
        m.set_str_key("huge", f64::INFINITY);
        m.set_str_key("log", fp!(self::math::log));
        m.set_str_key("max", fp!(self::math::max));
        m.set_str_key("maxinteger", i64::MAX);
        m.set_str_key("mininteger", i64::MIN);
        m.set_str_key("modf", fp!(self::math::modf));
        m.set_str_key("pi", core::f64::consts::PI);
        m.set_str_key("sin", fp!(self::math::sin));
        m.set_str_key("type", fp!(self::math::r#type));
        m.set_str_key("ult", fp!(self::math::ult));

        Ok(m)
    }
}

/// [Module] implementation for
/// [operating system library](https://www.lua.org/manual/5.4/manual.html#6.9).
#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
pub struct OsLib;

#[cfg(feature = "std")]
#[cfg_attr(docsrs, doc(cfg(feature = "std")))]
impl<A> Module<A> for OsLib {
    const NAME: &str = "os";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}

/// [Module] implementation for [string library](https://www.lua.org/manual/5.4/manual.html#6.4).
///
/// Note that [Self::open()] will **overwrite** string metatable.
pub struct StrLib;

impl<A> Module<A> for StrLib {
    const NAME: &str = "string";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        // Set up module table.
        let m = lua.create_table();

        m.set_str_key("byte", fp!(self::string::byte));
        m.set_str_key("char", fp!(self::string::char));
        m.set_str_key("find", fp!(self::string::find));
        m.set_str_key("format", fp!(self::string::format));
        m.set_str_key("gsub", fp!(self::string::gsub));
        m.set_str_key("len", fp!(self::string::len));
        m.set_str_key("rep", fp!(self::string::rep));
        m.set_str_key("sub", fp!(self::string::sub));

        // Set up metatable.
        let mt = lua.create_table();

        mt.set_str_key("__add", fp!(self::string::add));
        mt.set_str_key("__index", m.deref());
        mt.set_str_key("__mod", fp!(self::string::rem));
        mt.set_str_key("__pow", fp!(self::string::pow));
        mt.set_str_key("__sub", fp!(self::string::subtract));
        mt.set_str_key("__unm", fp!(self::string::negate));

        lua.set_str_metatable(&mt);

        Ok(m)
    }
}

/// [Module] implementation for [table library](https://www.lua.org/manual/5.4/manual.html#6.6).
pub struct TableLib;

impl<A> Module<A> for TableLib {
    const NAME: &str = "table";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        m.set_str_key("unpack", fp!(self::table::unpack));

        Ok(m)
    }
}

/// [Module] implementation for [UTF-8 library](https://www.lua.org/manual/5.4/manual.html#6.5).
pub struct Utf8Lib;

impl<A> Module<A> for Utf8Lib {
    const NAME: &str = "utf8";

    type Inst<'a>
        = Ref<'a, Table<A>>
    where
        A: 'a;

    fn open(self, lua: &Lua<A>) -> Result<Self::Inst<'_>, Box<dyn core::error::Error>> {
        let m = lua.create_table();

        Ok(m)
    }
}
