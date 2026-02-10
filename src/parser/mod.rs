use alloc::borrow::Cow;
use alloc::string::String;
use core::error::Error;
use core::ffi::c_int;
use core::fmt::{Display, Formatter};

/// Represents an error when failed to parse Lua source.
#[derive(Debug)]
pub enum ParseError {
    /// Limit has been reached on an item (e.g. local variables).
    ItemLimit {
        /// Name of the item.
        name: &'static str,
        /// Limit of the item.
        limit: c_int,
        /// Line number that cause the limit to exceed.
        line: c_int,
    },
    /// Source error (e.g. syntax error).
    Source {
        /// Reason of the error.
        reason: String,
        /// Token that cause the error.
        token: Option<Cow<'static, str>>,
        /// Line number that cause the error.
        line: c_int,
    },
}

impl ParseError {
    pub fn line(&self) -> c_int {
        match self {
            Self::ItemLimit {
                name: _,
                limit: _,
                line,
            } => *line,
            Self::Source {
                reason: _,
                token: _,
                line,
            } => *line,
        }
    }
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ItemLimit {
                name,
                limit,
                line: _,
            } => {
                write!(f, "too many {} (limit is {})", name, limit)
            }
            Self::Source {
                reason,
                token,
                line: _,
            } => match token {
                Some(t) => write!(f, "{reason} near {t}"),
                None => write!(f, "{reason}"),
            },
        }
    }
}
