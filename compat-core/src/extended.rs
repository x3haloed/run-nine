//! Packed classic extended numbers and integer arctangent. No host FP needed.
include!("atan_constants.rs");
const ONE: u128 = 1 << 112;
const INEXACT: u16 = 0x1000;
const UNDERFLOW: u16 = 0x0200;
const INVALID: u16 = 0x0100;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Extended80 {
    pub sign_exp: u16,
    pub significand: u64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rounding {
    Nearest,
    Up,
    Down,
    TowardZero,
}
impl Extended80 {
    /// Signed word/long conversion. Native large-magnitude rejection has
    /// additional rounding-status behavior that is not implemented yet.
    /// The optional X override models the integer negation's carry flag.
    pub fn integer_sane(
        self,
        rounding: Rounding,
        bits: u32,
    ) -> Option<(u32, u16, u8, Option<bool>)> {
        assert!(matches!(bits, 16 | 32));
        let indefinite = 1u32 << (bits - 1);
        if self.sign_exp & 0x7fff == 0x7fff {
            return Some((indefinite, INVALID, 8, None));
        }
        let normalized = self.normalized();
        if normalized.significand != 0 && normalized.sign_exp & 0x7fff >= 16383 + 63 {
            return None;
        }
        let (value, flags) = normalized.integral_value(rounding);
        let e = (value.sign_exp & 0x7fff) as i32 - 16383;
        let negative = value.sign_exp & 0x8000 != 0;
        if value.significand != 0 && e >= 63 {
            return Some((indefinite, INVALID | (flags & 0x80), 8, None));
        }
        let magnitude = if value.significand == 0 {
            0
        } else {
            value.significand >> (63 - e)
        };
        let x = if negative { Some(magnitude != 0) } else { None };
        let limit = 1u64 << (bits - 1);
        if magnitude > limit || (!negative && magnitude == limit) {
            return Some((indefinite, INVALID | (flags & 0x80), 8, x));
        }
        let integer = if negative {
            (magnitude as u32).wrapping_neg()
        } else {
            magnitude as u32
        };
        Some((integer, flags, if flags & INEXACT == 0 { 4 } else { 0 }, x))
    }
    /// Integral rounding before final destination-format rounding. The first
    /// rounding does not set SANE's last-round bit for extended destinations.
    pub fn integral_sane(
        self,
        rounding: Rounding,
        precision: u32,
        truncate: bool,
    ) -> (Self, u16, u8) {
        if self.sign_exp & 0x7fff == 0x7fff {
            return self.convert_sane(rounding, precision);
        }
        let (value, flags) = self.integral_value(if truncate {
            Rounding::TowardZero
        } else {
            rounding
        });
        let (result, final_flags, condition) = value.convert_sane(rounding, precision);
        (result, (flags & !0x80) | final_flags, condition)
    }
    fn integral_value(self, rounding: Rounding) -> (Self, u16) {
        let value = self.normalized();
        if value.significand == 0 {
            return (value, 0);
        }
        let exponent = (value.sign_exp & 0x7fff) as i32 - 16383;
        if exponent >= 63 {
            return (value, 0);
        }
        let shift = 63 - exponent;
        let mag = value.significand as u128;
        let whole = if shift >= 128 { 0 } else { mag >> shift };
        let remainder = if shift >= 128 {
            mag
        } else {
            mag & ((1u128 << shift) - 1)
        };
        let increment = remainder != 0
            && match rounding {
                Rounding::Nearest => {
                    shift <= 128
                        && (remainder > 1u128 << (shift - 1)
                            || (remainder == 1u128 << (shift - 1) && whole & 1 != 0))
                }
                Rounding::Up => value.sign_exp & 0x8000 == 0,
                Rounding::Down => value.sign_exp & 0x8000 != 0,
                Rounding::TowardZero => false,
            };
        let integer = whole + u128::from(increment);
        let sign = value.sign_exp & 0x8000;
        let result = if integer == 0 {
            Self {
                sign_exp: sign,
                significand: 0,
            }
        } else {
            let top = 127 - integer.leading_zeros();
            Self {
                sign_exp: sign | (16383 + top as u16),
                significand: (integer << (63 - top)) as u64,
            }
        };
        (
            result,
            if remainder != 0 { INEXACT } else { 0 } | if increment { 0x80 } else { 0 },
        )
    }
    /// Exact source decoding for mixed-format arithmetic. Conversion-specific
    /// NaN quieting/default diagnostics and precision rounding happen later.
    pub fn from_double_bits(bits: u64) -> Self {
        let sign = ((bits >> 48) & 0x8000) as u16;
        let exp = (bits >> 52) & 0x7ff;
        let fraction = bits & 0x000fffffffffffff;
        if exp == 0x7ff {
            return Self {
                sign_exp: sign | 0x7fff,
                significand: fraction << 11,
            };
        }
        if exp == 0 {
            if fraction == 0 {
                return Self {
                    sign_exp: sign,
                    significand: 0,
                };
            }
            let top = 63 - fraction.leading_zeros();
            return Self {
                sign_exp: sign | (top as i32 - 1074 + 16383) as u16,
                significand: fraction << (63 - top),
            };
        }
        Self {
            sign_exp: sign | (exp as i32 - 1023 + 16383) as u16,
            significand: (1 << 63) | (fraction << 11),
        }
    }
    pub fn add_sane(
        self,
        mut rhs: Self,
        subtract: bool,
        rounding: Rounding,
        precision: u32,
    ) -> (Self, u16, u8) {
        assert!(matches!(precision, 24 | 53 | 64));
        if self.classify_sane().abs() <= 2 || rhs.classify_sane().abs() <= 2 {
            return self.multiply_sane(rhs, rounding, precision);
        }
        if subtract {
            rhs.sign_exp ^= 0x8000;
        }
        let a_inf = self.sign_exp & 0x7fff == 0x7fff;
        let b_inf = rhs.sign_exp & 0x7fff == 0x7fff;
        if a_inf || b_inf {
            if a_inf && b_inf && (self.sign_exp ^ rhs.sign_exp) & 0x8000 != 0 {
                return (
                    Self {
                        sign_exp: rhs.sign_exp,
                        significand: 0x4002000000000000,
                    },
                    INVALID,
                    0,
                );
            }
            let value = if a_inf { self } else { rhs };
            return (
                Self {
                    sign_exp: value.sign_exp,
                    significand: 0,
                },
                0,
                0,
            );
        }
        let zero = |sign| {
            (
                Self {
                    sign_exp: sign,
                    significand: 0,
                },
                0,
                4,
            )
        };
        let sign_a = self.sign_exp & 0x8000;
        let sign_b = rhs.sign_exp & 0x8000;
        if self.significand == 0 && rhs.significand == 0 {
            return zero(if sign_a == sign_b {
                sign_a
            } else if rounding == Rounding::Down {
                0x8000
            } else {
                0
            });
        }
        if self.significand == 0 {
            return rhs.convert_sane(rounding, precision);
        }
        if rhs.significand == 0 {
            return self.convert_sane(rounding, precision);
        }
        let normalize = |v: Self| {
            let shift = v.significand.leading_zeros();
            (
                (v.sign_exp & 0x7fff) as i32 - 16383 - shift as i32,
                v.significand << shift,
                v.sign_exp & 0x8000,
            )
        };
        let mut a = normalize(self);
        let mut b = normalize(rhs);
        if (a.0, a.1) < (b.0, b.1) {
            core::mem::swap(&mut a, &mut b);
        }
        let left = u128::from(a.1) << 63;
        let right = shift_right_jam(u128::from(b.1) << 63, (a.0 - b.0) as u32);
        let magnitude = if a.2 == b.2 {
            left + right
        } else {
            left - right
        };
        if magnitude == 0 {
            return zero(if rounding == Rounding::Down {
                0x8000
            } else {
                0
            });
        }
        let top = 127 - magnitude.leading_zeros() as i32;
        let exact_exponent = a.0 - 126 + top;
        let minimum = match precision {
            24 => -126,
            53 => -1022,
            _ => -16383,
        };
        let shift = top - (precision as i32 - 1) + (minimum - exact_exponent).max(0);
        let (truncated, remainder) = if shift >= 128 {
            (0, magnitude)
        } else if shift >= 0 {
            (magnitude >> shift, magnitude & ((1u128 << shift) - 1))
        } else {
            (magnitude << -shift, 0)
        };
        let increase = remainder != 0
            && match rounding {
                Rounding::Nearest => {
                    shift <= 128 && {
                        let half = 1u128 << (shift - 1);
                        remainder > half || (remainder == half && truncated & 1 != 0)
                    }
                }
                Rounding::Up => a.2 == 0,
                Rounding::Down => a.2 != 0,
                Rounding::TowardZero => false,
            };
        let flags = if remainder != 0 { INEXACT } else { 0 } | if increase { 0x80 } else { 0 };
        finish_sane(
            a.2,
            precision,
            exact_exponent,
            truncated + u128::from(increase),
            flags,
            rounding,
        )
    }
    pub fn sqrt_sane(self, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        assert!(matches!(precision, 24 | 53 | 64));
        let class = self.classify_sane();
        if class.abs() <= 2 {
            return self.convert_sane(rounding, precision);
        }
        if class.abs() == 4 {
            return (
                Self {
                    sign_exp: self.sign_exp & 0x8000,
                    significand: 0,
                },
                0,
                4,
            );
        }
        if class < 0 {
            return (
                Self {
                    sign_exp: 0xffff,
                    significand: 0x4001000000000000,
                },
                INVALID,
                0,
            );
        }
        if class == 3 {
            return (
                Self {
                    sign_exp: 0x7fff,
                    significand: 0,
                },
                0,
                0,
            );
        }
        let leading = self.significand.leading_zeros();
        let exponent = (self.sign_exp & 0x7fff) as i32 - 16383 - leading as i32;
        let numerator = u128::from(self.significand << leading) << (63 + exponent.rem_euclid(2));
        let mut root = 0u128;
        let mut remainder = 0u128;
        for pair in (0..64).rev() {
            remainder = (remainder << 2) | ((numerator >> (pair * 2)) & 3);
            let trial = (root << 2) | 1;
            root <<= 1;
            if remainder >= trial {
                remainder -= trial;
                root |= 1;
            }
        }
        let exact_exponent = exponent.div_euclid(2);
        let minimum = match precision {
            24 => -126,
            53 => -1022,
            _ => -16383,
        };
        let shift = 64 - precision + (minimum - exact_exponent).max(0) as u32;
        let (truncated, discarded) = if shift >= 64 {
            (0, root)
        } else {
            (root >> shift, root & ((1u128 << shift) - 1))
        };
        let inexact = discarded != 0 || remainder != 0;
        let increase = inexact
            && match rounding {
                Rounding::Nearest => {
                    if shift == 0 {
                        remainder > root
                    } else if shift <= 64 {
                        let half = 1u128 << (shift - 1);
                        discarded > half
                            || (discarded == half && (remainder != 0 || truncated & 1 != 0))
                    } else {
                        false
                    }
                }
                Rounding::Up => true,
                Rounding::Down | Rounding::TowardZero => false,
            };
        let flags = if inexact { INEXACT } else { 0 } | if increase { 0x80 } else { 0 };
        finish_sane(
            0,
            precision,
            exact_exponent,
            truncated + u128::from(increase),
            flags,
            rounding,
        )
    }
    /// Quiet FP68K comparison: signaling NaNs raise invalid; neither operand
    /// is modified. The result is the complete measured NZVC/X condition word.
    /// FCPX signals invalid for every unordered result; FCMP only signals for
    /// signaling NaNs. Neither operation quiets or rewrites its operands.
    pub fn compare_signaling_sane(self, rhs: Self) -> (u8, u16) {
        let (condition, flags) = self.compare_sane(rhs);
        (condition, flags | if condition == 2 { INVALID } else { 0 })
    }
    pub fn compare_sane(self, rhs: Self) -> (u8, u16) {
        use core::cmp::Ordering;
        let a = self.classify_sane().abs();
        let b = rhs.classify_sane().abs();
        if a <= 2 || b <= 2 {
            return (2, if a == 1 || b == 1 { INVALID } else { 0 });
        }
        if a == 4 && b == 4 {
            return (4, 0);
        }
        let negative = self.sign_exp & 0x8000 != 0;
        let rhs_negative = rhs.sign_exp & 0x8000 != 0;
        let order = if negative != rhs_negative {
            if negative {
                Ordering::Less
            } else {
                Ordering::Greater
            }
        } else {
            let key = |value: Self, class: i16| {
                if class == 3 {
                    (i32::MAX, 0)
                } else if class == 4 {
                    (i32::MIN, 0)
                } else {
                    let shift = value.significand.leading_zeros();
                    (
                        (value.sign_exp & 0x7fff) as i32 - shift as i32,
                        value.significand << shift,
                    )
                }
            };
            let result = key(self, a).cmp(&key(rhs, b));
            if negative {
                result.reverse()
            } else {
                result
            }
        };
        (
            match order {
                Ordering::Less => 25,
                Ordering::Equal => 4,
                Ordering::Greater => 0,
            },
            0,
        )
    }
    /// Classify without quieting signaling NaNs or changing the environment.
    pub fn classify_sane(self) -> i16 {
        let kind = if self.sign_exp & 0x7fff == 0x7fff {
            if self.significand & 0x7fffffffffffffff == 0 {
                3
            } else if self.significand & (1 << 62) != 0 {
                2
            } else {
                1
            }
        } else {
            let value = self.normalized();
            if value.significand == 0 {
                4
            } else if value.significand & (1 << 63) == 0 {
                6
            } else {
                5
            }
        };
        if self.sign_exp & 0x8000 != 0 {
            -kind
        } else {
            kind
        }
    }
    pub fn convert_sane(self, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        self.multiply_sane(
            Self {
                sign_exp: 0x3fff,
                significand: 1 << 63,
            },
            rounding,
            precision,
        )
    }
    /// SANE's sine is periodic in its packed extended pi, rather than an
    /// infinitely precise pi. Reduction is exact even at the largest exponent.
    /// The final byte selects X: clear=0, set=1, preserve input=2.
    pub fn sine_sane(self) -> (Self, u16, u8) {
        let exp = self.sign_exp & 0x7fff;
        let negative = self.sign_exp & 0x8000 != 0;
        if exp == 0x7fff {
            if self.significand & 0x7fffffffffffffff == 0 {
                return (
                    Self {
                        sign_exp: 0x7fff,
                        significand: 0x4021000000000000,
                    },
                    INVALID,
                    2,
                );
            }
            return (
                Self {
                    sign_exp: self.sign_exp,
                    significand: (self.significand & 0x7fffffffffffffff) | (1 << 62),
                },
                if self.significand & (1 << 62) == 0 {
                    INVALID
                } else {
                    0
                },
                2,
            );
        }
        let normal = self.normalized();
        if normal.significand == 0 {
            return (normal, 0, 2);
        }
        let leading = normal.significand.leading_zeros();
        let exponent = (normal.sign_exp & 0x7fff) as i32 - 16383 - leading as i32;
        if exponent <= -33 {
            return (
                normal,
                INEXACT
                    | if normal.significand & (1 << 63) == 0 {
                        UNDERFLOW
                    } else {
                        0
                    },
                0,
            );
        }
        let mant = u128::from(normal.significand << leading);
        let (quadrant, reduced, reduced_negative) = reduce_trig(exponent, mant);
        let cosine = quadrant & 1 != 0;
        let total = trig_series(reduced, cosine);
        // Native exact phase-zero signs depend on the quadrant, not input sign.
        let result_negative =
            (negative && total != 0) ^ (quadrant & 2 != 0) ^ (reduced_negative && !cosine);
        (
            from_fixed(total, result_negative, Rounding::Nearest),
            INEXACT,
            u8::from((reduced == 0 && cosine) || (negative && !cosine && exponent >= 0)),
        )
    }
    /// Cosine uses the shared exact SANE-period phase and reduced series.
    pub fn cosine_sane(self) -> (Self, u16, u8) {
        if self.sign_exp & 0x7fff == 0x7fff {
            return self.sine_sane();
        }
        let normal = self.normalized();
        let one = Self {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        };
        if normal.significand == 0 {
            return (one, 0, 2);
        }
        let leading = normal.significand.leading_zeros();
        let exponent = (normal.sign_exp & 0x7fff) as i32 - 16383 - leading as i32;
        if exponent <= -33 {
            return (one, INEXACT, u8::from(exponent < -64));
        }
        let input_negative = normal.sign_exp & 0x8000 != 0;
        let (quadrant, reduced, reduced_negative) =
            reduce_trig(exponent, u128::from(normal.significand << leading));
        let cosine = quadrant & 1 == 0;
        let total = trig_series(reduced, cosine);
        let negative = ((quadrant + 1) & 2 != 0)
            ^ (reduced_negative && !cosine)
            ^ (input_negative && total == 0);
        // Native X reflects exact phase centers and the first negative-angle
        // fold. Keep this status separate from the numerical result's sign.
        let x_mode = u8::from(
            (reduced == 0 && cosine) || (input_negative && quadrant == 1 && exponent <= 1),
        );
        (
            from_fixed(total, negative, Rounding::Nearest),
            INEXACT,
            x_mode,
        )
    }
    /// Tangent shares exact phase reduction with sine. The reduced sine/cosine
    /// quotient is rounded once, retaining guard bits near the packed-pi poles.
    pub fn tangent_sane(self) -> (Self, u16, u8) {
        if self.sign_exp & 0x7fff == 0x7fff {
            return self.sine_sane();
        }
        let normal = self.normalized();
        if normal.significand == 0 {
            return (normal, 0, 2);
        }
        let leading = normal.significand.leading_zeros();
        let exponent = (normal.sign_exp & 0x7fff) as i32 - 16383 - leading as i32;
        if exponent <= -33 {
            return (
                normal,
                INEXACT
                    | if normal.significand & (1 << 63) == 0 {
                        UNDERFLOW
                    } else {
                        0
                    },
                2,
            );
        }
        let (quadrant, reduced, reduced_negative) =
            reduce_trig(exponent, u128::from(normal.significand << leading));
        let sign = normal.sign_exp & 0x8000;
        if reduced == 0 {
            return (
                Self {
                    sign_exp: sign | if quadrant & 1 != 0 { 0x7fff } else { 0 },
                    significand: 0,
                },
                INEXACT | if quadrant & 1 != 0 { 0x0800 } else { 0 },
                2,
            );
        }
        let sine = trig_series(reduced, false);
        let cosine = trig_series(reduced, true);
        let negative = (sign != 0) ^ reduced_negative ^ (quadrant & 1 != 0);
        let result = if quadrant & 1 == 0 {
            ratio_extended(sine, cosine, negative)
        } else {
            ratio_extended(cosine, sine, negative)
        };
        (result, INEXACT, if reduced > ONE / 2 { 0 } else { 2 })
    }
    pub fn from_integer(value: i16) -> Self {
        let magnitude = i32::from(value).unsigned_abs();
        if magnitude == 0 {
            return Self {
                sign_exp: 0,
                significand: 0,
            };
        }
        let top = 31 - magnitude.leading_zeros();
        Self {
            sign_exp: (if value < 0 { 0x8000 } else { 0 }) | (16383 + top) as u16,
            significand: u64::from(magnitude) << (63 - top),
        }
    }
    /// Divide normalized 64-bit mantissas with a full 128-bit numerator.
    /// Keep the exact remainder through format rounding, including underflow.
    pub fn divide_sane(self, rhs: Self, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        assert!(matches!(precision, 24 | 53 | 64));
        let ea = self.sign_exp & 0x7fff;
        let eb = rhs.sign_exp & 0x7fff;
        let sign = (self.sign_exp ^ rhs.sign_exp) & 0x8000;
        if (ea == 0x7fff && self.significand & 0x7fffffffffffffff != 0)
            || (eb == 0x7fff && rhs.significand & 0x7fffffffffffffff != 0)
        {
            return self.multiply_sane(rhs, rounding, precision);
        }
        if (ea == 0x7fff && eb == 0x7fff)
            || (ea != 0x7fff && eb != 0x7fff && self.significand == 0 && rhs.significand == 0)
        {
            return (
                Self {
                    sign_exp: sign | 0x7fff,
                    significand: 0x4004000000000000,
                },
                INVALID,
                0,
            );
        }
        if ea == 0x7fff {
            return (
                Self {
                    sign_exp: sign | 0x7fff,
                    significand: 0,
                },
                0,
                0,
            );
        }
        if eb == 0x7fff || self.significand == 0 {
            return (
                Self {
                    sign_exp: sign,
                    significand: 0,
                },
                0,
                4,
            );
        }
        if rhs.significand == 0 {
            return (
                Self {
                    sign_exp: sign | 0x7fff,
                    significand: 0,
                },
                0x0800,
                0,
            );
        }
        let la = self.significand.leading_zeros();
        let lb = rhs.significand.leading_zeros();
        let ma = self.significand << la;
        let mb = rhs.significand << lb;
        let below = ma < mb;
        let exact_exponent = ea as i32 - la as i32 - eb as i32 + lb as i32 - i32::from(below);
        let numerator = (ma as u128) << (63 + u32::from(below));
        let quotient = numerator / u128::from(mb);
        let remainder = numerator % u128::from(mb);
        let minimum = match precision {
            24 => -126,
            53 => -1022,
            _ => -16383,
        };
        let shift = 64 - precision + (minimum - exact_exponent).max(0) as u32;
        let (truncated, discarded) = if shift >= 64 {
            (0, quotient)
        } else {
            (quotient >> shift, quotient & ((1u128 << shift) - 1))
        };
        let inexact = discarded != 0 || remainder != 0;
        let increase = inexact
            && match rounding {
                Rounding::Nearest => {
                    if shift == 0 {
                        remainder * 2 > u128::from(mb)
                            || (remainder * 2 == u128::from(mb) && truncated & 1 != 0)
                    } else if shift <= 64 {
                        let half = 1u128 << (shift - 1);
                        discarded > half
                            || (discarded == half && (remainder != 0 || truncated & 1 != 0))
                    } else {
                        false
                    }
                }
                Rounding::Up => sign == 0,
                Rounding::Down => sign != 0,
                Rounding::TowardZero => false,
            };
        let flags = if inexact { INEXACT } else { 0 } | if increase { 0x80 } else { 0 };
        finish_sane(
            sign,
            precision,
            exact_exponent,
            truncated + u128::from(increase),
            flags,
            rounding,
        )
    }
    /// Exact 64-by-64 product, rounded once to the selected SANE format.
    /// The condition bits describe the measured FP68K arithmetic return ABI.
    pub fn multiply_sane(self, rhs: Self, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        assert!(matches!(precision, 24 | 53 | 64));
        let (minimum, _maximum) = match precision {
            24 => (-126, 127),
            53 => (-1022, 1023),
            _ => (-16383, 16383),
        };
        let ea = self.sign_exp & 0x7fff;
        let eb = rhs.sign_exp & 0x7fff;
        let sign = (self.sign_exp ^ rhs.sign_exp) & 0x8000;
        let negative = sign != 0;
        let nan_a = ea == 0x7fff && self.significand & 0x7fffffffffffffff != 0;
        let nan_b = eb == 0x7fff && rhs.significand & 0x7fffffffffffffff != 0;
        if nan_a || nan_b {
            let invalid = (nan_a && self.significand & (1 << 62) == 0)
                || (nan_b && rhs.significand & (1 << 62) == 0);
            let chosen = if nan_a
                && (!nan_b
                    || self.significand & 0x7fffffffffffffff > rhs.significand & 0x7fffffffffffffff)
            {
                self
            } else {
                rhs
            };
            let mut significand = (chosen.significand & 0x7fffffffffffffff) | (1 << 62);
            if precision < 64 {
                significand = (significand >> (64 - precision)) << (64 - precision);
                if significand & 0x3fffffffffffffff == 0 {
                    significand |= 21 << 48;
                }
            }
            return (
                Self {
                    sign_exp: chosen.sign_exp,
                    significand,
                },
                if invalid { INVALID } else { 0 },
                0,
            );
        }
        if ea == 0x7fff || eb == 0x7fff {
            if (ea != 0x7fff && self.significand == 0) || (eb != 0x7fff && rhs.significand == 0) {
                return (
                    Self {
                        sign_exp: 0x7fff,
                        significand: 0x4008000000000000,
                    },
                    INVALID,
                    0,
                );
            }
            return (
                Self {
                    sign_exp: sign | 0x7fff,
                    significand: 0,
                },
                0,
                0,
            );
        }
        let product = self.significand as u128 * rhs.significand as u128;
        if product == 0 {
            return (
                Self {
                    sign_exp: sign,
                    significand: 0,
                },
                0,
                4,
            );
        }
        let top = 127 - product.leading_zeros() as i32;
        let exact_exponent = ea as i32 + eb as i32 - 32766 - 126 + top;
        let shift = top - (precision as i32 - 1) + (minimum - exact_exponent).max(0);
        let (mut rounded, remainder) = if shift >= 128 {
            (0, product)
        } else if shift >= 0 {
            (product >> shift, product & ((1u128 << shift) - 1))
        } else {
            (product << -shift, 0)
        };
        let increase = remainder != 0
            && match rounding {
                Rounding::Nearest => {
                    shift <= 128
                        && (remainder > 1u128 << (shift - 1)
                            || (remainder == 1u128 << (shift - 1) && rounded & 1 != 0))
                }
                Rounding::Up => !negative,
                Rounding::Down => negative,
                Rounding::TowardZero => false,
            };
        rounded += u128::from(increase);
        let flags = if remainder != 0 { INEXACT } else { 0 } | if increase { 0x80 } else { 0 };
        finish_sane(sign, precision, exact_exponent, rounded, flags, rounding)
    }
    pub fn finite_at_least_one(self) -> bool {
        let exp = self.sign_exp & 0x7fff;
        if exp == 0x7fff || self.significand == 0 || exp < 16383 {
            return false;
        }
        let power = exp - 16383;
        power >= 64 || self.significand >= 1u64 << (63 - power)
    }
    /// FP68K FX2D honors a narrower environment precision as well as double's
    /// own range. Reuse exact format rounding, then pack without a second round.
    pub fn double_sane(self, rounding: Rounding, precision: u32) -> (u64, u16, u8) {
        let one = Self {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        };
        let (value, flags, _) = self.multiply_sane(one, rounding, precision.min(53));
        let sign = u64::from(value.sign_exp & 0x8000) << 48;
        let exponent = value.sign_exp & 0x7fff;
        if exponent == 0x7fff {
            return (
                sign | 0x7ff0000000000000 | ((value.significand >> 11) & 0x000fffffffffffff),
                flags,
                1,
            );
        }
        if value.significand == 0 {
            return (sign, flags, 8);
        }
        let e = exponent as i32 - 16383;
        let bits = if e >= -1022 {
            ((e + 1023) as u64) << 52 | ((value.significand >> 11) & 0x000fffffffffffff)
        } else {
            value.significand >> (-e - 1011)
        };
        let condition = 1 | if e < -1023 {
            8
        } else if e == -1023 {
            4
        } else {
            0
        };
        (sign | bits, flags, condition)
    }
    /// Native-measured exact small-integer paths of Elems68K FXPWRY.
    /// General real exponents remain unsupported rather than using host FP.
    pub fn small_integer_power_sane(self, rhs: Self) -> Option<(Self, u16, u8)> {
        if rhs.sign_exp & 0x7fff == 0x7fff {
            return None;
        }
        let exponent = rhs.normalized();
        let degree = if exponent.significand == 0 {
            0i8
        } else if exponent.significand == 1 << 63
            && matches!(exponent.sign_exp & 0x7fff, 0x3fff | 0x4000)
        {
            let magnitude = if exponent.sign_exp & 0x7fff == 0x3fff { 1 } else { 2 };
            if exponent.sign_exp & 0x8000 != 0 { -magnitude } else { magnitude }
        } else {
            return None;
        };
        let class = self.classify_sane().abs();
        if class <= 2 {
            return Some((Self {
                sign_exp: self.sign_exp,
                significand: (self.significand & 0x7fffffffffffffff) | (1 << 62),
            }, if class == 1 { INVALID } else { 0 }, 2));
        }
        let one = Self::from_integer(1);
        if degree == 0 {
            return Some(if class == 3 || class == 4 {
                (Self { sign_exp: 0x7fff, significand: 0x4025000000000000 }, INVALID, 0)
            } else {
                (one, 0, 0)
            });
        }
        let odd = degree.unsigned_abs() == 1;
        let sign = if odd { self.sign_exp & 0x8000 } else { 0 };
        if class == 4 {
            return Some(if degree < 0 {
                (Self { sign_exp: sign | 0x7fff, significand: 0 }, 0x0800, 0)
            } else {
                (Self { sign_exp: sign, significand: 0 }, 0,
                    if sign != 0 { 0 } else { 1 })
            });
        }
        if class == 3 {
            return Some((Self { sign_exp: sign | if degree > 0 { 0x7fff } else { 0 }, significand: 0 }, 0, 1));
        }
        let (result, flags) = match degree {
            1 => (self, 0),
            -1 => {
                let (value, flags, _) = one.divide_sane(self, Rounding::Nearest, 64);
                (value, flags)
            }
            2 => {
                let (value, flags, _) = self.multiply_sane(self, Rounding::Nearest, 64);
                (value, flags)
            }
            -2 => {
                let (square, flags, _) = self.multiply_sane(self, Rounding::Nearest, 64);
                // Range-changing intermediates need a separate native power audit.
                if square.classify_sane().abs() == 3 || square.significand == 0
                    || flags & (0x0400 | UNDERFLOW) != 0 {
                    return None;
                }
                let (value, reciprocal_flags, _) = one.divide_sane(square, Rounding::Nearest, 64);
                (value, flags | reciprocal_flags)
            }
            _ => unreachable!(),
        };
        Some((result, flags & 0x1f00, 1))
    }
    /// Signed32 integers fit exactly in IEEE64 before environment rounding.
    pub fn from_long_sane(value: i32, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        let magnitude = u64::from(value.unsigned_abs());
        let bits = if magnitude == 0 {
            0
        } else {
            let top = 63 - magnitude.leading_zeros();
            (if value < 0 { 1u64 << 63 } else { 0 })
                | (u64::from(1023 + top) << 52)
                | ((magnitude ^ (1 << top)) << (52 - top))
        };
        Self::from_double_sane(bits, rounding, precision)
    }
    /// Exact integer-only IEEE32 expansion preserves signaling NaN payloads.
    /// The native-measured widening rules then share the IEEE64-to-extended path.
    pub fn from_single_sane(bits: u32, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        let sign = u64::from(bits & 0x80000000) << 32;
        let exponent = (bits >> 23) & 0xff;
        let fraction = u64::from(bits & 0x7fffff);
        let double = match exponent {
            0 if fraction == 0 => sign,
            0 => {
                let top = 63 - fraction.leading_zeros();
                sign | ((top as i32 - 149 + 1023) as u64) << 52
                    | ((fraction ^ (1 << top)) << (52 - top))
            }
            0xff => sign | 0x7ff0000000000000 | (fraction << 29),
            _ => sign | (u64::from(exponent + 896) << 52) | (fraction << 29),
        };
        Self::from_double_sane(double, rounding, precision)
    }
    pub fn from_double_sane(bits: u64, rounding: Rounding, precision: u32) -> (Self, u16, u8) {
        assert!(matches!(precision, 24 | 53 | 64));
        let sign = ((bits >> 48) & 0x8000) as u16;
        let exponent = (bits >> 52) & 0x7ff;
        let fraction = bits & 0x000fffffffffffff;
        if exponent == 0x7ff {
            if fraction == 0 {
                return (
                    Self {
                        sign_exp: sign | 0x7fff,
                        significand: 0,
                    },
                    0,
                    0,
                );
            }
            let mut significand =
                ((fraction << 11) | (1 << 62)) >> (64 - precision) << (64 - precision);
            if significand & 0x3fffffffffffffff == 0 {
                significand |= 21 << 48;
            }
            return (
                Self {
                    sign_exp: sign | 0x7fff,
                    significand,
                },
                if fraction & (1 << 51) == 0 {
                    INVALID
                } else {
                    0
                },
                0,
            );
        }
        let value = if exponent == 0 {
            if fraction == 0 {
                Self {
                    sign_exp: sign,
                    significand: 0,
                }
            } else {
                let top = 63 - fraction.leading_zeros();
                Self {
                    sign_exp: sign | (top as i32 - 1074 + 16383) as u16,
                    significand: fraction << (63 - top),
                }
            }
        } else {
            Self {
                sign_exp: sign | (exponent as i32 - 1023 + 16383) as u16,
                significand: (1 << 63) | (fraction << 11),
            }
        };
        let one = Self {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        };
        let (result, flags, _) = value.multiply_sane(one, rounding, precision);
        let condition = if result.sign_exp & 0x7fff != 0x7fff && result.significand == 0 {
            4
        } else {
            0
        };
        (result, flags, condition)
    }
    pub fn single(self, rounding: Rounding) -> (u32, u16) {
        let sign = if self.sign_exp & 0x8000 != 0 {
            0x80000000
        } else {
            0
        };
        let negative = sign != 0;
        let exp = self.sign_exp & 0x7fff;
        if exp == 0x7fff {
            if self.significand & 0x7fffffffffffffff == 0 {
                return (sign | 0x7f800000, 0);
            }
            let signaling = self.significand & (1 << 62) == 0;
            let mut fraction = ((self.significand >> 40) as u32 & 0x7fffff) | 0x400000;
            if fraction & 0xff00 == 0 {
                fraction |= 0x1500;
            }
            return (
                sign | 0x7f800000 | fraction,
                if signaling { INVALID } else { 0 },
            );
        }
        let n = self.normalized();
        if n.significand == 0 {
            return (sign, 0);
        }
        let leading = n.significand.leading_zeros();
        let mant = n.significand << leading;
        let mut e = (n.sign_exp & 0x7fff) as i32 - 16383 - leading as i32;
        let overflow = || {
            let infinite = rounding == Rounding::Nearest
                || (!negative && rounding == Rounding::Up)
                || (negative && rounding == Rounding::Down);
            (
                sign | if infinite { 0x7f800000 } else { 0x7f7fffff },
                0x1400,
            )
        };
        if e > 127 {
            return overflow();
        }
        let shift = if e >= -126 { 40 } else { (-e - 86) as u32 };
        let truncated = if shift < 64 { mant >> shift } else { 0 };
        let remainder = if shift < 64 {
            mant & ((1u64 << shift) - 1)
        } else {
            mant
        };
        let increase = match rounding {
            Rounding::Nearest => {
                if shift <= 64 {
                    let half = 1u128 << (shift - 1);
                    remainder as u128 > half || (remainder as u128 == half && truncated & 1 != 0)
                } else {
                    false
                }
            }
            Rounding::Up => !negative && remainder != 0,
            Rounding::Down => negative && remainder != 0,
            Rounding::TowardZero => false,
        };
        let mut rounded = truncated + u64::from(increase);
        let mut flags = if remainder != 0 { INEXACT } else { 0 } | if increase { 0x80 } else { 0 };
        if e < -126 {
            if remainder != 0 {
                flags |= UNDERFLOW;
            }
            return (sign | rounded as u32, flags);
        }
        if rounded == 1 << 24 {
            rounded >>= 1;
            e += 1;
        }
        if e > 127 {
            return overflow();
        }
        (
            sign | (((e + 127) as u32) << 23) | (rounded as u32 & 0x7fffff),
            flags,
        )
    }
    pub fn from_bytes(bytes: [u8; 10]) -> Self {
        Self {
            sign_exp: u16::from_be_bytes([bytes[0], bytes[1]]),
            significand: u64::from_be_bytes(bytes[2..].try_into().unwrap()),
        }
    }
    pub fn to_bytes(self) -> [u8; 10] {
        let mut b = [0; 10];
        b[..2].copy_from_slice(&self.sign_exp.to_be_bytes());
        b[2..].copy_from_slice(&self.significand.to_be_bytes());
        b
    }
    fn normalized(self) -> Self {
        let sign = self.sign_exp & 0x8000;
        let exp = self.sign_exp & 0x7fff;
        if self.significand == 0 {
            return Self {
                sign_exp: sign,
                significand: 0,
            };
        }
        let shift = self.significand.leading_zeros() as u16;
        if exp >= shift {
            Self {
                sign_exp: sign | (exp - shift),
                significand: self.significand << shift,
            }
        } else {
            Self {
                sign_exp: sign,
                significand: self.significand << exp,
            }
        }
    }
    fn magnitude_predecessor(mut self) -> Self {
        let exp = self.sign_exp & 0x7fff;
        if self.significand > 1 << 63 || exp == 0 {
            self.significand -= 1;
        } else {
            self.sign_exp -= 1;
            self.significand = u64::MAX;
        }
        self
    }
    pub fn atan(self, rounding: Rounding) -> (Self, u16) {
        let negative = self.sign_exp & 0x8000 != 0;
        let exp = self.sign_exp & 0x7fff;
        if exp == 0x7fff {
            if self.significand & 0x7fffffffffffffff != 0 {
                let signaling = self.significand & (1 << 62) == 0;
                return (
                    Self {
                        sign_exp: self.sign_exp,
                        significand: (self.significand & 0x7fffffffffffffff) | (1 << 62),
                    },
                    if signaling { INVALID } else { 0 },
                );
            }
            return (from_fixed(PI_OVER_2, negative, rounding), 0);
        }
        let normal = self.normalized();
        if normal.significand == 0 {
            return (normal, 0);
        }
        let leading = normal.significand.leading_zeros() as i32;
        let e = (normal.sign_exp & 0x7fff) as i32 - 16383 - leading;
        if e <= -33 {
            let reduce = matches!(rounding, Rounding::TowardZero)
                || (negative && rounding == Rounding::Up)
                || (!negative && rounding == Rounding::Down);
            let result = if reduce {
                normal.magnitude_predecessor()
            } else {
                normal
            };
            let tiny = result.significand & (1 << 63) == 0;
            return (result, INEXACT | if tiny { UNDERFLOW } else { 0 });
        }
        let mant = normal.significand as u128;
        let value = if e >= 0 {
            let reciprocal = if e >= 112 {
                0
            } else {
                fraction(1 << 63, mant, (112 - e) as u32)
            };
            PI_OVER_2 - atan_unit(reciprocal)
        } else {
            atan_unit(mant << (49 + e))
        };
        (from_fixed(value, negative, rounding), INEXACT)
    }
}
const HALF_PI_MANT: u128 = 0xc90fdaa22168c235;
/// Exact modular reduction by the packed SANE half-pi, retaining Q112 phase.
fn reduce_trig(exponent: i32, mant: u128) -> (u8, u128, bool) {
    debug_assert!(exponent >= -32);
    if exponent >= 0 {
        let period = 4 * HALF_PI_MANT;
        let mut residue = mant;
        for _ in 0..exponent {
            residue <<= 1;
            if residue >= period {
                residue -= period;
            }
        }
        let mut quotient = residue / HALF_PI_MANT;
        let remainder = residue % HALF_PI_MANT;
        let round_up =
            remainder * 2 > HALF_PI_MANT || (remainder * 2 == HALF_PI_MANT && quotient & 1 != 0);
        let magnitude = if round_up {
            quotient += 1;
            HALF_PI_MANT - remainder
        } else {
            remainder
        };
        ((quotient & 3) as u8, magnitude << 49, round_up)
    } else {
        let value = mant << (49 + exponent);
        if value > HALF_PI_MANT << 48 {
            (1, (HALF_PI_MANT << 49) - value, true)
        } else {
            (0, value, false)
        }
    }
}
fn trig_series(reduced: u128, cosine: bool) -> u128 {
    let square = multiply(reduced, reduced);
    let mut term = if cosine { ONE } else { reduced };
    let mut total = term;
    for k in 1..128u128 {
        term = multiply(term, square)
            / if cosine {
                (2 * k - 1) * (2 * k)
            } else {
                (2 * k) * (2 * k + 1)
            };
        if term == 0 {
            break;
        }
        if k & 1 != 0 {
            total -= term;
        } else {
            total += term;
        }
    }
    total
}
/// Round a quotient of positive Q112 series values once to extended precision.
fn ratio_extended(mut numerator: u128, mut denominator: u128, negative: bool) -> Extended80 {
    debug_assert!(numerator != 0 && denominator != 0);
    let mut exponent = denominator.leading_zeros() as i32 - numerator.leading_zeros() as i32;
    if exponent >= 0 {
        denominator <<= exponent;
    } else {
        numerator <<= -exponent;
    }
    if numerator < denominator {
        numerator <<= 1;
        exponent -= 1;
    }
    let mut remainder = numerator - denominator;
    let mut mantissa = 1u128;
    for _ in 0..63 {
        remainder <<= 1;
        mantissa <<= 1;
        if remainder >= denominator {
            remainder -= denominator;
            mantissa |= 1;
        }
    }
    if remainder * 2 > denominator || (remainder * 2 == denominator && mantissa & 1 != 0) {
        mantissa += 1;
    }
    if mantissa == 1u128 << 64 {
        mantissa >>= 1;
        exponent += 1;
    }
    Extended80 {
        sign_exp: (if negative { 0x8000 } else { 0 }) | (exponent + 16383) as u16,
        significand: mantissa as u64,
    }
}
fn shift_right_jam(value: u128, shift: u32) -> u128 {
    if shift == 0 {
        value
    } else if shift >= 128 {
        u128::from(value != 0)
    } else {
        (value >> shift) | u128::from(value & ((1u128 << shift) - 1) != 0)
    }
}
fn finish_sane(
    sign: u16,
    precision: u32,
    exact_exponent: i32,
    mut rounded: u128,
    mut flags: u16,
    rounding: Rounding,
) -> (Extended80, u16, u8) {
    let (minimum, maximum) = match precision {
        24 => (-126, 127),
        53 => (-1022, 1023),
        _ => (-16383, 16383),
    };
    let negative = sign != 0;
    if exact_exponent < minimum && flags & INEXACT != 0 {
        flags |= UNDERFLOW;
    }
    let mut exponent = exact_exponent.max(minimum);
    if rounded >= 1u128 << precision {
        rounded >>= 1;
        exponent += 1;
    }
    if exponent > maximum {
        let infinity = rounding == Rounding::Nearest
            || (!negative && rounding == Rounding::Up)
            || (negative && rounding == Rounding::Down);
        return (
            if infinity {
                Extended80 {
                    sign_exp: sign | 0x7fff,
                    significand: 0,
                }
            } else {
                Extended80 {
                    sign_exp: sign | (maximum + 16383) as u16,
                    significand: (((1u128 << precision) - 1) << (64 - precision)) as u64,
                }
            },
            flags | 0x1400,
            0,
        );
    }
    if rounded == 0 {
        return (
            Extended80 {
                sign_exp: sign,
                significand: 0,
            },
            flags,
            4,
        );
    }
    let (encoded, significand) = if precision == 64 && exponent == minimum {
        (0, rounded as u64)
    } else {
        let leading = 127 - rounded.leading_zeros();
        (
            (exponent + 16383 - (precision - 1 - leading) as i32) as u16,
            (rounded << (63 - leading)) as u64,
        )
    };
    let condition = if precision < 64 {
        0
    } else if exact_exponent + 16383 < 0 {
        8
    } else if exact_exponent + 16383 == 0 {
        4
    } else {
        0
    };
    (
        Extended80 {
            sign_exp: sign | encoded,
            significand,
        },
        flags,
        condition,
    )
}
/// The operands here are at most one Q112 unit; four-limb multiplication
/// preserves the fractional guard bits without requiring a 256-bit host type.
fn multiply(a: u128, b: u128) -> u128 {
    let a0 = a as u64 as u128;
    let a1 = a >> 64;
    let b0 = b as u64 as u128;
    let b1 = b >> 64;
    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;
    let limb1 = (p00 >> 64) + (p01 as u64 as u128) + (p10 as u64 as u128);
    let limb2 = (p01 >> 64) + (p10 >> 64) + (p11 as u64 as u128) + (limb1 >> 64);
    let limb3 = (p11 >> 64) + (limb2 >> 64);
    ((limb1 as u64 >> 48) as u128) | ((limb2 as u64 as u128) << 16) | (limb3 << 80)
}
fn fraction(numerator: u128, denominator: u128, bits: u32) -> u128 {
    debug_assert!(numerator <= denominator && bits <= 112);
    if numerator == denominator {
        return 1 << bits;
    }
    let mut remainder = numerator;
    let mut result = 0;
    for _ in 0..bits {
        remainder <<= 1;
        result <<= 1;
        if remainder >= denominator {
            remainder -= denominator;
            result |= 1;
        }
    }
    result
}
fn atan_small(x: u128) -> u128 {
    let square = multiply(x, x);
    let mut power = x;
    let mut sum = x as i128;
    for k in 1..128 {
        power = multiply(power, square);
        let term = power / (2 * k + 1);
        if term == 0 {
            break;
        }
        if k & 1 == 0 {
            sum += term as i128;
        } else {
            sum -= term as i128;
        }
    }
    sum as u128
}
fn atan_unit(x: u128) -> u128 {
    if x > ONE / 2 {
        PI_OVER_4 - atan_small(fraction(ONE - x, ONE + x, 112))
    } else {
        atan_small(x)
    }
}
fn from_fixed(value: u128, negative: bool, rounding: Rounding) -> Extended80 {
    if value == 0 {
        return Extended80 {
            sign_exp: if negative { 0x8000 } else { 0 },
            significand: 0,
        };
    }
    let top = 127 - value.leading_zeros();
    let mut exponent = top as i32 - 112 + 16383;
    let mut mant = if top > 63 {
        value >> (top - 63)
    } else {
        value << (63 - top)
    };
    if top > 63 {
        let shift = top - 63;
        let remainder = value & ((1 << shift) - 1);
        let half = 1 << (shift - 1);
        let increase = match rounding {
            Rounding::Nearest => remainder > half || (remainder == half && mant & 1 != 0),
            Rounding::Up => !negative && remainder != 0,
            Rounding::Down => negative && remainder != 0,
            Rounding::TowardZero => false,
        };
        if increase {
            mant += 1;
        }
    }
    if mant == 1 << 64 {
        mant >>= 1;
        exponent += 1;
    }
    Extended80 {
        sign_exp: exponent as u16 | if negative { 0x8000 } else { 0 },
        significand: mant as u64,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cosine_keeps_even_neighbors_and_native_opposite_root_zero_signs() {
        let half_pi = Extended80 {
            sign_exp: 0x3fff,
            significand: 0xc90fdaa22168c235,
        };
        let one = Extended80 {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        };
        for sign in [0, 0x8000] {
            assert_eq!(
                Extended80 {
                    sign_exp: sign,
                    significand: 0
                }
                .cosine_sane(),
                (one, 0, 2)
            );
            let root = Extended80 {
                sign_exp: sign | 0x3fff,
                ..half_pi
            };
            assert_eq!(
                root.cosine_sane().0,
                Extended80 {
                    sign_exp: sign ^ 0x8000,
                    significand: 0
                }
            );
            let before = Extended80 {
                significand: root.significand - 1,
                ..root
            };
            let after = Extended80 {
                significand: root.significand + 1,
                ..root
            };
            assert_eq!(
                before.cosine_sane().0,
                Extended80 {
                    sign_exp: 0x3fc0,
                    significand: 1 << 63
                }
            );
            assert_eq!(
                after.cosine_sane().0,
                Extended80 {
                    sign_exp: 0xbfc0,
                    significand: 1 << 63
                }
            );
            assert_eq!(
                Extended80 {
                    sign_exp: sign | 0x4000,
                    ..half_pi
                }
                .cosine_sane()
                .0,
                Extended80 {
                    sign_exp: 0xbfff,
                    significand: 1 << 63
                }
            );
        }
    }
    #[test]
    fn tangent_retains_phase_neighbors_pole_signs_and_quotient_rounding_carry() {
        let pole = Extended80 {
            sign_exp: 0x3fff,
            significand: 0xc90fdaa22168c235,
        };
        assert_eq!(
            pole.tangent_sane(),
            (
                Extended80 {
                    sign_exp: 0x7fff,
                    significand: 0
                },
                INEXACT | 0x0800,
                2
            )
        );
        let below = Extended80 {
            significand: pole.significand - 1,
            ..pole
        };
        let above = Extended80 {
            significand: pole.significand + 1,
            ..pole
        };
        assert_eq!(
            below.tangent_sane().0,
            Extended80 {
                sign_exp: 0x403e,
                significand: 1 << 63
            }
        );
        assert_eq!(
            above.tangent_sane().0,
            Extended80 {
                sign_exp: 0xc03e,
                significand: 1 << 63
            }
        );
        for sign in [0, 0x8000] {
            let pi = Extended80 {
                sign_exp: sign | 0x4000,
                ..pole
            };
            assert_eq!(
                pi.tangent_sane(),
                (
                    Extended80 {
                        sign_exp: sign,
                        significand: 0
                    },
                    INEXACT,
                    2
                )
            );
            let signed_pole = Extended80 {
                sign_exp: sign | 0x3fff,
                ..pole
            };
            assert_eq!(signed_pole.tangent_sane().0.sign_exp, sign | 0x7fff);
        }
        assert_eq!(
            ratio_extended(1, 3, false),
            Extended80 {
                sign_exp: 0x3ffd,
                significand: 0xaaaaaaaaaaaaaaab
            }
        );
        // A halfway quotient immediately below one rounds through a mantissa
        // carry into the next exponent, without an intermediate FP rounding.
        assert_eq!(
            ratio_extended((1u128 << 65) - 1, 1u128 << 65, false),
            Extended80 {
                sign_exp: 0x3fff,
                significand: 1 << 63
            }
        );
    }
    #[test]
    fn integer_conversion_keeps_rounding_carries_across_signed_range_checks() {
        let positive = Extended80 {
            sign_exp: 0x401d,
            significand: 0xffffffff00000000,
        };
        assert_eq!(
            positive.integer_sane(Rounding::Nearest, 32),
            Some((0x80000000, INVALID | 0x80, 8, None))
        );
        assert_eq!(
            positive.integer_sane(Rounding::Down, 32),
            Some((0x7fffffff, INEXACT, 0, None))
        );
        let negative = Extended80 {
            sign_exp: 0xc01e,
            significand: 0x8000000080000000,
        };
        assert_eq!(
            negative.integer_sane(Rounding::Nearest, 32),
            Some((0x80000000, INEXACT, 0, Some(true)))
        );
        assert_eq!(
            negative.integer_sane(Rounding::Down, 32),
            Some((0x80000000, INVALID | 0x80, 8, Some(true)))
        );
        let carry = Extended80 {
            sign_exp: 0xc03d,
            significand: u64::MAX,
        };
        assert_eq!(
            carry.integer_sane(Rounding::Down, 16),
            Some((0x8000, INVALID | 0x80, 8, None))
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x403e,
                significand: 1 << 63
            }
            .integer_sane(Rounding::Nearest, 32),
            None
        );
    }
    #[test]
    fn truncation_precedes_precision_rounding_and_keeps_its_own_zero_sign() {
        // 2^31 - 1/4 truncates to 2^31 - 1, then rounds upward to 2^31
        // at single precision. The last-round bit describes that second step.
        let value = Extended80 {
            sign_exp: 0x401d,
            significand: 0xffffffff80000000,
        };
        let (wide, flags, _) = value.integral_sane(Rounding::Nearest, 64, true);
        assert_eq!(
            wide,
            Extended80 {
                sign_exp: 0x401d,
                significand: 0xfffffffe00000000
            }
        );
        assert_eq!(flags, INEXACT);
        let (single, flags, _) = value.integral_sane(Rounding::Nearest, 24, true);
        assert_eq!(
            single,
            Extended80 {
                sign_exp: 0x401e,
                significand: 1 << 63
            }
        );
        assert_eq!(flags, INEXACT | 0x80);
        let negative_half = Extended80 {
            sign_exp: 0xbffe,
            significand: 1 << 63,
        };
        for rounding in [
            Rounding::Nearest,
            Rounding::Up,
            Rounding::Down,
            Rounding::TowardZero,
        ] {
            assert_eq!(
                negative_half.integral_sane(rounding, 64, true),
                (
                    Extended80 {
                        sign_exp: 0x8000,
                        significand: 0
                    },
                    INEXACT,
                    4
                )
            );
        }
    }
    #[test]
    fn raw_double_arithmetic_preserves_signaling_and_small_nan_payloads() {
        let signal = Extended80::from_double_bits(0x7ff0000000000001);
        assert_eq!(
            signal,
            Extended80 {
                sign_exp: 0x7fff,
                significand: 0x800
            }
        );
        let (result, flags, _) =
            Extended80::from_integer(1).multiply_sane(signal, Rounding::Nearest, 53);
        assert_eq!((result.significand, flags), (0x4000000000000800, INVALID));
        assert_eq!(
            Extended80::from_double_bits(0xfff8000000000000).significand,
            1 << 62
        );
        assert_eq!(
            Extended80::from_double_bits(1),
            Extended80 {
                sign_exp: 0x3bcd,
                significand: 1 << 63
            }
        );
        assert_eq!(
            Extended80::from_double_bits(0x8000000000000000),
            Extended80 {
                sign_exp: 0x8000,
                significand: 0
            }
        );
    }
    #[test]
    fn basic_arithmetic_keeps_sticky_tails_and_exact_root_remainders() {
        let one = Extended80::from_integer(1);
        let tiny = Extended80 {
            sign_exp: 0x3fbd,
            significand: 1 << 63,
        };
        assert_eq!(
            one.add_sane(tiny, false, Rounding::Nearest, 64),
            (one, INEXACT, 0)
        );
        assert_eq!(
            one.add_sane(tiny, false, Rounding::Up, 64),
            (
                Extended80 {
                    sign_exp: 0x3fff,
                    significand: (1 << 63) + 1
                },
                INEXACT | 0x80,
                0
            )
        );
        assert_eq!(
            one.add_sane(tiny, true, Rounding::Down, 64).0,
            Extended80 {
                sign_exp: 0x3ffe,
                significand: u64::MAX
            }
        );
        assert_eq!(
            one.add_sane(one, true, Rounding::Down, 64),
            (
                Extended80 {
                    sign_exp: 0x8000,
                    significand: 0
                },
                0,
                4
            )
        );
        assert_eq!(
            Extended80::from_integer(4).sqrt_sane(Rounding::Nearest, 64),
            (Extended80::from_integer(2), 0, 0)
        );
        assert_eq!(
            Extended80::from_integer(2).sqrt_sane(Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x3fff,
                    significand: 0xb504f333f9de6484
                },
                INEXACT,
                0
            )
        );
        assert_eq!(
            Extended80::from_integer(2)
                .sqrt_sane(Rounding::Up, 64)
                .0
                .significand,
            0xb504f333f9de6485
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0,
                significand: 1
            }
            .sqrt_sane(Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x1fe0,
                    significand: 1 << 63
                },
                0,
                0
            )
        );
        assert_eq!(one.compare_sane(Extended80::from_integer(2)), (25, 0));
        assert_eq!(
            Extended80 {
                sign_exp: 0x8000,
                significand: 0
            }
            .compare_sane(Extended80::from_integer(0)),
            (4, 0)
        );
    }
    #[test]
    fn classification_distinguishes_signals_signed_zero_and_minimum_normal() {
        assert_eq!(
            Extended80 {
                sign_exp: 0x7fff,
                significand: 1
            }
            .classify_sane(),
            1
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0xffff,
                significand: 1 << 62
            }
            .classify_sane(),
            -2
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x7fff,
                significand: 1 << 63
            }
            .classify_sane(),
            3
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x8000,
                significand: 0
            }
            .classify_sane(),
            -4
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0,
                significand: 1 << 63
            }
            .classify_sane(),
            5
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0,
                significand: 1
            }
            .classify_sane(),
            6
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x3fff,
                significand: 1
            }
            .classify_sane(),
            5
        );
    }
    #[test]
    fn extended_conversion_obeys_selected_precision_and_direction() {
        let input = Extended80 {
            sign_exp: 0x3fff,
            significand: 0x8000000000000001,
        };
        assert_eq!(input.convert_sane(Rounding::Nearest, 64), (input, 0, 0));
        assert_eq!(
            input.convert_sane(Rounding::Nearest, 53),
            (Extended80::from_integer(1), INEXACT, 0)
        );
        assert_eq!(
            input.convert_sane(Rounding::Up, 24),
            (
                Extended80 {
                    sign_exp: 0x3fff,
                    significand: 0x8000010000000000
                },
                INEXACT | 0x80,
                0
            )
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x7fff,
                significand: 1 << 63
            }
            .convert_sane(Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x7fff,
                    significand: 0
                },
                0,
                0
            )
        );
    }
    #[test]
    fn sine_reduces_exact_extended_periods_and_preserves_native_zero_signs() {
        let half_pi = Extended80 {
            sign_exp: 0x3fff,
            significand: 0xc90fdaa22168c235,
        };
        assert_eq!(
            half_pi.sine_sane(),
            (Extended80::from_integer(1), INEXACT, 1)
        );
        let pi = Extended80 {
            sign_exp: 0x4000,
            ..half_pi
        };
        assert_eq!(
            pi.sine_sane(),
            (
                Extended80 {
                    sign_exp: 0x8000,
                    significand: 0
                },
                INEXACT,
                0
            )
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0xc000,
                ..half_pi
            }
            .sine_sane(),
            (
                Extended80 {
                    sign_exp: 0x8000,
                    significand: 0
                },
                INEXACT,
                1
            )
        );
        assert_eq!(
            Extended80 {
                sign_exp: 0x7ffe,
                ..half_pi
            }
            .sine_sane(),
            (
                Extended80 {
                    sign_exp: 0,
                    significand: 0
                },
                INEXACT,
                0
            )
        );
        assert_eq!(
            Extended80 {
                significand: half_pi.significand + 1,
                ..pi
            }
            .sine_sane()
            .0,
            Extended80 {
                sign_exp: 0xbfc1,
                significand: 1 << 63
            }
        );
        let tiny = Extended80 {
            sign_exp: 0,
            significand: 1,
        };
        assert_eq!(tiny.sine_sane(), (tiny, INEXACT | UNDERFLOW, 0));
    }
    #[test]
    fn division_remainder_survives_format_and_underflow_rounding() {
        let two = Extended80 {
            sign_exp: 0x4000,
            significand: 1 << 63,
        };
        let three = Extended80 {
            sign_exp: 0x4000,
            significand: 0xc000000000000000,
        };
        assert_eq!(
            two.divide_sane(three, Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x3ffe,
                    significand: 0xaaaaaaaaaaaaaaab
                },
                INEXACT | 0x80,
                0
            )
        );
        assert_eq!(
            two.divide_sane(three, Rounding::Down, 64).0.significand,
            0xaaaaaaaaaaaaaaaa
        );
        let smallest = Extended80 {
            sign_exp: 0,
            significand: 1,
        };
        assert_eq!(
            smallest.divide_sane(two, Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0,
                    significand: 0
                },
                INEXACT | UNDERFLOW,
                4
            )
        );
        assert_eq!(
            smallest.divide_sane(two, Rounding::Up, 64),
            (smallest, INEXACT | UNDERFLOW | 0x80, 8)
        );
        let zero = Extended80 {
            sign_exp: 0,
            significand: 0,
        };
        assert_eq!(two.divide_sane(zero, Rounding::Nearest, 64).1, 0x0800);
        assert_eq!(
            zero.divide_sane(zero, Rounding::Nearest, 64).0.significand,
            0x4004000000000000
        );
    }
    #[test]
    fn signed_integer_extremes_are_exact() {
        assert_eq!(
            Extended80::from_integer(i16::MIN),
            Extended80 {
                sign_exp: 0xc00e,
                significand: 1 << 63
            }
        );
        assert_eq!(
            Extended80::from_integer(i16::MAX),
            Extended80 {
                sign_exp: 0x400d,
                significand: 0xfffe000000000000
            }
        );
        assert_eq!(
            Extended80::from_integer(-1),
            Extended80 {
                sign_exp: 0xbfff,
                significand: 1 << 63
            }
        );
        assert_eq!(
            Extended80::from_integer(0),
            Extended80 {
                sign_exp: 0,
                significand: 0
            }
        );
    }
    #[test]
    fn widen_preserves_double_subnormals_nan_payloads_and_zero_sign() {
        assert_eq!(
            Extended80::from_double_sane(1, Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x3bcd,
                    significand: 1 << 63
                },
                0,
                0
            )
        );
        assert_eq!(
            Extended80::from_double_sane(0x8000000000000000, Rounding::Nearest, 64),
            (
                Extended80 {
                    sign_exp: 0x8000,
                    significand: 0
                },
                0,
                4
            )
        );
        assert_eq!(
            Extended80::from_double_sane(0x7ff0000000000001, Rounding::Nearest, 53),
            (
                Extended80 {
                    sign_exp: 0x7fff,
                    significand: 0x4000000000000800
                },
                INVALID,
                0
            )
        );
        assert_eq!(
            Extended80::from_double_sane(0x7ff8000000000000, Rounding::Nearest, 64)
                .0
                .significand,
            0x4015000000000000
        );
        assert_eq!(
            Extended80::from_double_sane(0x3ff0000000000001, Rounding::Up, 24),
            (
                Extended80 {
                    sign_exp: 0x3fff,
                    significand: 0x8000010000000000
                },
                INEXACT | 0x80,
                0
            )
        );
    }
    #[test]
    fn double_ties_subnormal_boundary_and_environment_precision() {
        let tie = Extended80 {
            sign_exp: 0x3fff,
            significand: (1 << 63) + 1024,
        };
        assert_eq!(
            tie.double_sane(Rounding::Nearest, 64),
            (0x3ff0000000000000, INEXACT, 1)
        );
        assert_eq!(
            tie.double_sane(Rounding::Up, 64),
            (0x3ff0000000000001, INEXACT | 0x80, 1)
        );
        let boundary = Extended80 {
            sign_exp: 0x3c00,
            significand: u64::MAX,
        };
        assert_eq!(
            boundary.double_sane(Rounding::Nearest, 64),
            (0x0010000000000000, INEXACT | UNDERFLOW | 0x80, 1)
        );
        assert_eq!(
            boundary.double_sane(Rounding::Down, 64),
            (0x000fffffffffffff, INEXACT | UNDERFLOW, 5)
        );
        let tiny = Extended80 {
            sign_exp: 0x3bcd,
            significand: 1 << 63,
        };
        assert_eq!(tiny.double_sane(Rounding::Nearest, 64), (1, 0, 9));
        assert_eq!(
            tiny.double_sane(Rounding::Up, 24),
            (0x36a0000000000000, INEXACT | UNDERFLOW | 0x80, 1)
        );
    }
    #[test]
    fn products_round_once_and_obey_selected_format_range() {
        let near_one = Extended80 {
            sign_exp: 0x3fff,
            significand: u64::MAX,
        };
        let (nearest, flags, _) = near_one.multiply_sane(near_one, Rounding::Nearest, 64);
        assert_eq!(
            nearest,
            Extended80 {
                sign_exp: 0x4000,
                significand: u64::MAX - 1
            }
        );
        assert_eq!(flags, INEXACT);
        let (up, flags, _) = near_one.multiply_sane(near_one, Rounding::Up, 64);
        assert_eq!(up.significand, u64::MAX);
        assert_eq!(flags, INEXACT | 0x80);
        let large = Extended80 {
            sign_exp: 0x4063,
            significand: 1 << 63,
        };
        assert_eq!(
            large.multiply_sane(large, Rounding::Nearest, 24).0.sign_exp,
            0x7fff
        );
        assert_eq!(
            large.multiply_sane(large, Rounding::TowardZero, 24).0,
            Extended80 {
                sign_exp: 0x407e,
                significand: 0xffffff0000000000
            }
        );
        assert_eq!(large.multiply_sane(large, Rounding::Nearest, 53).1, 0);
    }
    #[test]
    fn single_ties_use_even_mantissas_and_report_increment() {
        let even = Extended80 {
            sign_exp: 0x3fff,
            significand: (1 << 63) + (1 << 39),
        };
        assert_eq!(even.single(Rounding::Nearest), (0x3f800000, 0x1000));
        let odd = Extended80 {
            sign_exp: 0x3fff,
            significand: (1 << 63) + (1 << 40) + (1 << 39),
        };
        assert_eq!(odd.single(Rounding::Nearest), (0x3f800002, 0x1080));
        assert_eq!(odd.single(Rounding::Down), (0x3f800001, 0x1000));
    }
    #[test]
    fn single_underflow_preserves_zero_sign_and_directed_neighbor() {
        let tiny = Extended80 {
            sign_exp: 0,
            significand: 1,
        };
        assert_eq!(tiny.single(Rounding::Nearest), (0, 0x1200));
        assert_eq!(tiny.single(Rounding::Up), (1, 0x1280));
        let negative = Extended80 {
            sign_exp: 0x8000,
            significand: 1,
        };
        assert_eq!(negative.single(Rounding::Nearest), (0x80000000, 0x1200));
        assert_eq!(negative.single(Rounding::Down), (0x80000001, 0x1280));
    }
    #[test]
    fn fixed_products_preserve_wide_carries() {
        assert_eq!(multiply(ONE, ONE), ONE);
        assert_eq!(multiply(ONE / 2, ONE / 2), ONE / 4);
        assert_eq!(multiply(ONE - 1, ONE - 1), ONE - 2);
        assert_eq!(fraction(1, 2, 112), ONE / 2);
    }
    #[test]
    fn exact_specials_and_normalization() {
        for sign in [0, 0x8000] {
            let zero = Extended80 {
                sign_exp: sign,
                significand: 0,
            };
            assert_eq!(zero.atan(Rounding::Nearest), (zero, 0));
        }
        let half = Extended80 {
            sign_exp: 0x3ffe,
            significand: 1 << 63,
        };
        let unnormal = Extended80 {
            sign_exp: 0x3fff,
            significand: 1 << 62,
        };
        assert_eq!(
            half.atan(Rounding::Nearest),
            unnormal.atan(Rounding::Nearest)
        );
    }
    #[test]
    fn atan_one_retains_more_than_double_precision() {
        let x = Extended80 {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        };
        let (result, flags) = x.atan(Rounding::Nearest);
        assert_eq!(
            result,
            Extended80 {
                sign_exp: 0x3ffe,
                significand: 0xc90fdaa22168c235
            }
        );
        assert_eq!(flags, INEXACT);
        assert_ne!(result.significand & 0x7ff, 0);
    }
    #[test]
    fn tiny_directed_results_have_correct_signed_neighbors() {
        for sign in [0, 0x8000] {
            let input = Extended80 {
                sign_exp: sign | 0x3fbf,
                significand: 1 << 63,
            };
            assert_eq!(input.atan(Rounding::Nearest).0, input);
            let rounded = input.atan(Rounding::TowardZero).0;
            assert_eq!(rounded.sign_exp, input.sign_exp - 1);
            assert_eq!(rounded.significand, u64::MAX);
        }
    }
}
