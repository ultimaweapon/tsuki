use alloc::borrow::Cow;
use alloc::string::String;
use core::error::Error;
use core::ffi::c_int;
use core::fmt::{Display, Formatter};

/// Contains information for Lua chunk.
#[derive(Default, Clone)]
pub struct ChunkInfo {
    pub name: String,
}

/// Represents an error when failed to parse Lua source.
#[derive(Debug)]
pub enum ParseError {
    ItemLimit {
        name: &'static str,
        limit: c_int,
        line: c_int,
    },
    Source {
        reason: String,
        token: Option<Cow<'static, str>>,
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
