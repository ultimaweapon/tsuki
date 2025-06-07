use thiserror::Error;

/// Represents an error when Tsuki API fails.
#[non_exhaustive]
#[derive(Debug, Error)]
pub enum Error {}
