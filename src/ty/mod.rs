use core::fmt::{Display, Formatter};

/// Type of Lua value.
///
/// [Display] implementation on this type produce the same value as `lua_typename`.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    /// `nil`.
    Nil,
    /// `boolean`.
    Boolean,
    /// `function` that implemented by a function pointer.
    Fp,
    /// `number`.
    Number,
    /// `string`.
    String,
    /// `table`.
    Table,
    /// `function` that implemented in Lua or Rust closure.
    Fn,
    /// `userdata`.
    UserData,
    /// `thread`.
    Thread,
}

impl Type {
    /// # Panics
    /// If `v` is non-public type or invalid.
    #[inline(always)]
    pub(crate) const fn from_tt(v: u8) -> Self {
        // The match will optimized away since the value of each variant is the same. The generated
        // code is it will compare if the value greater than 8.
        match v & 0xf {
            0 => Self::Nil,
            1 => Self::Boolean,
            2 => Self::Fp,
            3 => Self::Number,
            4 => Self::String,
            5 => Self::Table,
            6 => Self::Fn,
            7 => Self::UserData,
            8 => Self::Thread,
            v => Self::invalid_tt(v),
        }
    }

    #[cold]
    #[inline(never)]
    const fn invalid_tt(v: u8) -> ! {
        match v {
            9 => panic!("upvalue cannot expose to external"),
            10 => panic!("function prototype cannot expose to external"),
            _ => panic!("unknown type"),
        }
    }
}

impl Display for Type {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let v = match self {
            Self::Nil => "nil",
            Self::Boolean => "boolean",
            Self::Fp | Self::Fn => "function",
            Self::Number => "number",
            Self::String => "string",
            Self::Table => "table",
            Self::UserData => "userdata",
            Self::Thread => "thread",
        };

        f.write_str(v)
    }
}
