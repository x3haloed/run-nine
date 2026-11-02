//! Reserved compatibility module.
//!
//! Register accessors are inherent methods on [`CpuCore`](super::cpu::CpuCore).
#[cfg(not(feature = "std"))]
#[allow(unused_imports)]
use crate::freestanding::prelude::*;
