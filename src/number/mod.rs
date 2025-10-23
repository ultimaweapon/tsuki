pub use self::float::*;

mod float;

/// Helper enum to encapsulates either integer or float.
#[derive(Clone, Copy, PartialEq)]
pub enum Number {
    Int(i64),
    Float(Float),
}

impl From<i64> for Number {
    #[inline(always)]
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<f64> for Number {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self::Float(value.into())
    }
}
