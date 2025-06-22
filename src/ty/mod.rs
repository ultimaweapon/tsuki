use core::fmt::{Display, Formatter};

/// Type of Lua value.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Type {
    Nil,
    Boolean,
    Fp,
    Number,
    String,
    Table,
    Fn,
    UserData,
    Thread,
}

impl Type {
    /// # Panics
    /// If `v` is upvalue, proto or invalid.
    pub(crate) const fn from_tt(v: u8) -> Self {
        // TODO: Verify if this optimized away since the value of each variant is the same. If not
        // we need to transmute.
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
            9 => panic!("upvalue cannot expose to external"),
            10 => panic!("function prototype cannot expose to external"),
            _ => panic!("unknown type"),
        }
    }
}

impl Display for Type {
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
