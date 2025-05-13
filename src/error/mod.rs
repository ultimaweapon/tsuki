use std::fmt::{Display, Formatter};

/// Represents an error when Tsuki API fails.
#[non_exhaustive]
#[derive(Debug)]
pub enum Error {}

impl std::error::Error for Error {}

impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            _ => unreachable!(),
        }
    }
}
