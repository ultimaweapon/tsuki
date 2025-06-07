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
#[non_exhaustive]
#[derive(Debug)]
pub enum ParseError {
    ItemLimit(&'static str, c_int),
    Source(String, Option<Cow<'static, str>>, c_int),
}

impl Error for ParseError {}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ItemLimit(n, l) => write!(f, "too many {n} (limit is {l})"),
            Self::Source(r, t, l) => match t {
                Some(t) => write!(f, "{l}: {r} near {t}"),
                None => write!(f, "{l}: {r}"),
            },
        }
    }
}
