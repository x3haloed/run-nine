//! Allocation accounting for the owned logical store, independent of host paths.
//! Catalog entries occupy256-byte logical slots;16 blocks reserve volume metadata.
pub const BLOCK_BYTES: u32 = 4096;
pub const RESERVED_BLOCKS: u32 = 16;
pub const DEFAULT_BLOCKS: u32 = 65535;
pub fn fork_blocks(bytes: usize) -> u64 {
    (bytes as u64).div_ceil(u64::from(BLOCK_BYTES))
}
pub fn catalog_blocks(entries: usize) -> u64 {
    (entries as u64)
        .saturating_mul(256)
        .div_ceil(u64::from(BLOCK_BYTES))
}
pub fn used_blocks(entries: usize, forks: u64) -> u64 {
    u64::from(RESERVED_BLOCKS)
        .saturating_add(catalog_blocks(entries))
        .saturating_add(forks)
}
/// The classic16-bit block-count view coarsens units when a larger store needs it.
pub fn legacy_geometry(total: u32, free: u32) -> (u16, u32, u16) {
    let factor = total.div_ceil(u32::from(u16::MAX)).max(1);
    (
        (total / factor) as u16,
        BLOCK_BYTES * factor,
        (free / factor) as u16,
    )
}
