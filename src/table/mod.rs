use thiserror::Error;

/// Represents an error when the operation on a table is fails.
#[derive(Debug, Error)]
pub enum TableError {
    #[error("key is nil")]
    NilKey,

    #[error("key is NaN")]
    NanKey,
}
