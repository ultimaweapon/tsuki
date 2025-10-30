use core::cmp::Ordering;
use core::fmt::{Display, Formatter};
use core::ops::{Add, AddAssign, Div, Mul, Neg, Sub};

/// Lua floating-point number.
///
/// This type provides [Display] implementation to match with Lua behavior when converting float to
/// string without fractional part. [Display] implementation on [f64] will omit fractional part if
/// it is zero but Lua requires this. Note that Tsuki **do not** truncate the precision while Lua
/// limit this to 14 digits by default.
#[repr(transparent)]
#[derive(Default, Clone, Copy, PartialEq, PartialOrd)]
pub struct Float(pub f64);

impl Float {
    /// Computes the absolute value of `self`.
    ///
    /// See [f64::abs()] for more details.
    #[inline(always)]
    pub const fn abs(self) -> Self {
        Self(self.0.abs())
    }

    /// Returns the largest integer less than or equal to `self`.
    ///
    /// See [f64::floor()] for more details.
    #[inline(always)]
    pub const fn floor(self) -> Self {
        Self(self.0.floor())
    }

    /// Returns the smallest integer greater than or equal to `self`.
    ///
    /// See [f64::ceil()] for more details.
    #[inline(always)]
    pub const fn ceil(self) -> Self {
        Self(self.0.ceil())
    }

    /// Returns the logarithm of the number with respect to an arbitrary base.
    ///
    /// See [f64::log()] for more details.
    #[inline(always)]
    pub fn log(self, base: Self) -> Self {
        Self(self.0.log(base.0))
    }

    /// Returns the base 2 logarithm of the number.
    ///
    /// See [f64::log2()] for more details.
    #[inline(always)]
    pub fn log2(self) -> Self {
        Self(self.0.log2())
    }

    /// Returns the base 10 logarithm of the number.
    ///
    /// See [f64::log10()] for more details.
    #[inline(always)]
    pub fn log10(self) -> Self {
        Self(self.0.log10())
    }

    /// Computes the sine of a number (in radians).
    ///
    /// See [f64::sin()] for more details.
    #[inline(always)]
    pub fn sin(self) -> Self {
        Self(self.0.sin())
    }

    /// Computes the cosine of a number (in radians).
    ///
    /// See [f64::cos()] for more details.
    #[inline(always)]
    pub fn cos(self) -> Self {
        Self(self.0.cos())
    }

    /// Computes the tangent of a number (in radians).
    ///
    /// See [f64::tan()] for more details.
    #[inline(always)]
    pub fn tan(self) -> Self {
        Self(self.0.tan())
    }

    /// Computes the four quadrant arctangent of `self` (`y`) and `x` in radians.
    ///
    /// See [f64::atan2()] for more details.
    #[inline(always)]
    pub fn atan2(self, x: Self) -> Self {
        Self(self.0.atan2(x.0))
    }

    /// Raises a number to a floating point power.
    ///
    /// See [f64::powf()] for more details.
    #[inline(always)]
    pub fn pow(self, n: Self) -> Self {
        Self(self.0.powf(n.0))
    }
}

impl PartialEq<f64> for Float {
    #[inline(always)]
    fn eq(&self, other: &f64) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<f64> for Float {
    #[inline(always)]
    fn partial_cmp(&self, other: &f64) -> Option<Ordering> {
        self.0.partial_cmp(other)
    }
}

impl Add for Float {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<f64> for Float {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: f64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl AddAssign<f64> for Float {
    #[inline(always)]
    fn add_assign(&mut self, rhs: f64) {
        self.0 += rhs;
    }
}

impl Sub for Float {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl Mul for Float {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl Mul<f64> for Float {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: f64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Div for Float {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0)
    }
}

impl Neg for Float {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self::Output {
        Self(-self.0)
    }
}

impl From<i32> for Float {
    #[inline(always)]
    fn from(value: i32) -> Self {
        Self(value.into())
    }
}

impl From<f32> for Float {
    #[inline(always)]
    fn from(value: f32) -> Self {
        Self(value.into())
    }
}

impl From<f64> for Float {
    #[inline(always)]
    fn from(value: f64) -> Self {
        Self(value)
    }
}

impl Display for Float {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        if self.0.fract() == 0.0 {
            write!(f, "{:.1}", self.0)?;
        } else {
            write!(f, "{}", self.0)?;
        }

        Ok(())
    }
}

impl From<Float> for f64 {
    #[inline(always)]
    fn from(value: Float) -> Self {
        value.0
    }
}
