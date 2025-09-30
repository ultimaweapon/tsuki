use alloc::borrow::{Cow, ToOwned};
use alloc::string::String;
use core::error::Error;
use core::ffi::c_int;
use core::fmt::{Display, Formatter};

/// Contains information for Lua chunk.
#[derive(Clone)]
pub struct ChunkInfo {
    pub(crate) name: String,
}

impl ChunkInfo {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into() }
    }

    /// Returns name of the chunk.
    #[inline(always)]
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl From<&str> for ChunkInfo {
    fn from(value: &str) -> Self {
        Self {
            name: value.to_owned(),
        }
    }
}

impl From<String> for ChunkInfo {
    #[inline(always)]
    fn from(value: String) -> Self {
        Self { name: value }
    }
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
