//! Minimal fixed-point Q16.16 arithmetic (zero-dependency)

pub type Fixed = i32;
pub const FRACTIONAL_BITS: i32 = 16;
pub const SCALE: Fixed = 1 << FRACTIONAL_BITS;

#[inline]
pub fn to_fixed(x: f32) -> Fixed {
    (x * SCALE as f32) as Fixed
}

#[inline]
pub fn from_fixed(x: Fixed) -> f32 {
    (x as f32) / SCALE as f32
}

#[inline]
pub fn fixed_mul(a: Fixed, b: Fixed) -> Fixed {
    ((a as i64 * b as i64) >> FRACTIONAL_BITS) as Fixed
}