//! One stereo sample: what the engine and the synth render. The same shape and arithmetic as
//! kira's `Frame`, so a backend converts with `.into()` (the `kira` feature) or reads `left` /
//! `right` itself.

use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign};

/// A stereo audio sample.
#[derive(Debug, Copy, Clone, PartialEq, Default)]
pub struct Frame {
    pub left: f32,
    pub right: f32,
}

impl Frame {
    pub const ZERO: Frame = Frame { left: 0.0, right: 0.0 };

    pub fn new(left: f32, right: f32) -> Self {
        Self { left, right }
    }

    /// Both channels set to `value`.
    pub fn from_mono(value: f32) -> Self {
        Self::new(value, value)
    }
}

impl Add for Frame {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self::new(self.left + rhs.left, self.right + rhs.right)
    }
}

impl AddAssign for Frame {
    fn add_assign(&mut self, rhs: Self) {
        *self = *self + rhs;
    }
}

impl Sub for Frame {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self::new(self.left - rhs.left, self.right - rhs.right)
    }
}

impl SubAssign for Frame {
    fn sub_assign(&mut self, rhs: Self) {
        *self = *self - rhs;
    }
}

impl Mul<f32> for Frame {
    type Output = Self;
    fn mul(self, rhs: f32) -> Self {
        Self::new(self.left * rhs, self.right * rhs)
    }
}

impl MulAssign<f32> for Frame {
    fn mul_assign(&mut self, rhs: f32) {
        *self = *self * rhs;
    }
}

impl Div<f32> for Frame {
    type Output = Self;
    fn div(self, rhs: f32) -> Self {
        Self::new(self.left / rhs, self.right / rhs)
    }
}

impl DivAssign<f32> for Frame {
    fn div_assign(&mut self, rhs: f32) {
        *self = *self / rhs;
    }
}

impl Neg for Frame {
    type Output = Self;
    fn neg(self) -> Self {
        Self::new(-self.left, -self.right)
    }
}

#[cfg(feature = "kira")]
impl From<Frame> for kira::Frame {
    fn from(f: Frame) -> Self {
        kira::Frame { left: f.left, right: f.right }
    }
}

#[cfg(feature = "kira")]
impl From<kira::Frame> for Frame {
    fn from(f: kira::Frame) -> Self {
        Frame { left: f.left, right: f.right }
    }
}
