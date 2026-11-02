//! Host-independent allocation, one-time initialization and floating point helpers.
#[allow(unused_imports)]
pub(crate) mod prelude {
    pub(crate) use super::FloatMath;
    pub(crate) use alloc::{boxed::Box, format, string::{String, ToString}, vec, vec::Vec};
}
pub(crate) struct OnceLock<T>(spin::Once<T>);
impl<T> OnceLock<T> {
    pub(crate) const fn new() -> Self {Self(spin::Once::new())}
    pub(crate) fn get_or_init(&self, f: impl FnOnce()->T)->&T {self.0.call_once(f)}
}

pub(crate) trait FloatMath {
 fn log10(self)->Self;
 fn floor(self)->Self;
 fn round(self)->Self;
}
impl FloatMath for f64 {
 fn log10(self)->Self {libm::log10(self)}
 fn floor(self)->Self {libm::floor(self)}
 fn round(self)->Self {libm::round(self)}
}
