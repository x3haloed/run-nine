//! Monotonic Color QuickDraw identity state, independent of a host clock/OS.
use core::cell::Cell;
/// Shared by managers belonging to one emulated QuickDraw environment.
/// The initial dynamic offset is profile-local; native prior activity varies.
pub struct ColorSeeds {
    last: Cell<u32>,
}
impl Default for ColorSeeds {
    fn default() -> Self {
        Self {
            last: Cell::new(0x400),
        }
    }
}
impl ColorSeeds {
    pub fn next(&self) -> u32 {
        let value = self.last.get().wrapping_add(1);
        self.last.set(value);
        value
    }
}
