//! QuickDraw's full32-bit seed transition, using integer arithmetic only.
//! Native observations include zero, modulus, high-bit and extreme seeds.
pub fn next(seed: u32) -> u32 {
    ((u64::from(seed) * 16807) % 2147483647) as u32
}
