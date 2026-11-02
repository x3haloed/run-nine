//! Guest pixel storage, independent of windowing, CPU and presentation APIs.
use crate::{MemoryBus, Rect};
#[derive(Clone, Copy)]
pub struct Surface {
    pub base: u32,
    pub row: u16,
    pub bounds: Rect,
    pub depth: u16,
}
impl Surface {
    pub fn address(&self, x: i16, y: i16) -> Option<(u32, u8)> {
        if !self.bounds.contains(y, x) {
            return None;
        }
        let col = (x as i32 - self.bounds.left as i32) as u32;
        let row = (y as i32 - self.bounds.top as i32) as u32;
        let (byte, mask) = match self.depth {
            1 => (col / 8, 0x80 >> (col & 7)),
            8 => (col, 255),
            _ => return None,
        };
        if byte >= self.row as u32 {
            return None;
        }
        Some((
            self.base
                .checked_add(row.checked_mul(self.row as u32)?)?
                .checked_add(byte)?,
            mask,
        ))
    }
    pub fn read<B: MemoryBus>(&self, m: &mut B, x: i16, y: i16) -> Option<u8> {
        let (address, mask) = self.address(x, y)?;
        let value = m.read_byte(address);
        Some(if self.depth == 1 {
            u8::from(value & mask != 0)
        } else {
            value
        })
    }
    pub fn write<B: MemoryBus>(
        &self,
        m: &mut B,
        x: i16,
        y: i16,
        source: u8,
        mode: u16,
    ) -> Option<()> {
        let (address, mask) = self.address(x, y)?;
        let before = m.read_byte(address);
        let dest = if self.depth == 1 {
            u8::from(before & mask != 0)
        } else {
            before
        };
        let result = match mode & 7 {
            0 => source,
            1 => source | dest,
            2 => source ^ dest,
            3 => (!source) & dest,
            4 => !source,
            5 => (!source) | dest,
            6 => (!source) ^ dest,
            7 => source & dest,
            _ => unreachable!(),
        };
        m.write_byte(
            address,
            if self.depth == 1 {
                if result & 1 != 0 {
                    before | mask
                } else {
                    before & !mask
                }
            } else {
                result
            },
        );
        Some(())
    }
}
/// Exact matches win; otherwise use squared distance in full 16-bit RGB space.
/// This declared software matcher does not emulate device inverse-table caches.
pub fn nearest(color: [u16; 3], palette: &[[u16; 3]]) -> Option<u8> {
    palette
        .iter()
        .enumerate()
        .min_by_key(|(_, p)| {
            color
                .iter()
                .zip(p.iter())
                .map(|(&a, &b)| {
                    let d = a as i64 - b as i64;
                    (d * d) as u64
                })
                .sum::<u64>()
        })
        .map(|(i, _)| i as u8)
}
/// Native-observed three/four-bit inverse grids. RGB positions concatenate
/// quantized components; gray tables use a weighted luminance path.
fn inverse_position(position: u16, palette: &[[u16; 3]], bits: u16) -> Option<u8> {
    if palette.is_empty() || palette.len() > 256 || !matches!(bits, 3 | 4) {
        return None;
    }
    let mask = (1 << bits) - 1;
    let rgb = [
        (position >> (2 * bits)) & mask,
        (position >> bits) & mask,
        position & mask,
    ];
    let gray = palette.iter().all(|p| p[0] == p[1] && p[1] == p[2]);
    let scale = 1 << (8 - bits);
    let luminance = (5 * rgb[0] * scale + 9 * rgb[1] * scale + 2 * rgb[2] * scale) / 16;
    palette
        .iter()
        .enumerate()
        .min_by_key(|(i, p)| {
            let distance = if gray {
                luminance.abs_diff(p[0] >> 8)
            } else {
                rgb.iter()
                    .zip(p.iter())
                    .map(|(&a, &b)| a.abs_diff(b >> (16 - bits)))
                    .sum()
            };
            let priority = if *i == 0 {
                0
            } else if *i == palette.len() - 1 {
                1
            } else {
                *i + 1
            };
            (distance, priority)
        })
        .map(|(i, _)| i as u8)
}
pub fn inverse_grid(palette: &[[u16; 3]], bits: u16) -> Option<alloc::vec::Vec<u8>> {
    if palette.is_empty() || palette.len() > 256 || !matches!(bits, 3 | 4) {
        return None;
    }
    (0..(1u16 << (3 * bits)))
        .map(|p| inverse_position(p, palette, bits))
        .collect()
}
pub fn inverse3(palette: &[[u16; 3]]) -> Option<alloc::vec::Vec<u8>> {
    inverse_grid(palette, 3)
}
/// Color2Index first preserves exact palette colors, then uses its device grid.
pub fn inverse_lookup(color: [u16; 3], palette: &[[u16; 3]], bits: u16) -> Option<u8> {
    if !matches!(bits, 3 | 4) {
        return None;
    }
    if let Some(i) = palette.iter().position(|p| *p == color) {
        return u8::try_from(i).ok();
    }
    let shift = 16 - bits;
    let position =
        ((color[0] >> shift) << (2 * bits)) | ((color[1] >> shift) << bits) | (color[2] >> shift);
    inverse_position(position, palette, bits)
}
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;
    #[test]
    fn inverse_cube_rejects_invalid_palettes_and_covers_single_color() {
        assert!(inverse3(&[]).is_none());
        assert!(inverse3(&alloc::vec![[0;3];257]).is_none());
        let grid = inverse3(&[[0x1234, 0x5678, 0x9abc]]).unwrap();
        assert_eq!(grid.len(), 512);
        assert!(grid.iter().all(|&index| index == 0));
    }
    struct M(Vec<u8>);
    impl MemoryBus for M {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
    }
    #[test]
    fn bounds_padding_and_boolean_operations() {
        let mut m = M(alloc::vec![0xaa;64]);
        let mut s = Surface {
            base: 8,
            row: 4,
            bounds: Rect {
                top: -2,
                left: 3,
                bottom: 0,
                right: 5,
            },
            depth: 8,
        };
        assert_eq!(s.write(&mut m, 2, -2, 1, 0), None);
        s.write(&mut m, 3, -2, 0x10, 0);
        s.write(&mut m, 3, -2, 0x11, 2);
        assert_eq!(s.read(&mut m, 3, -2), Some(1));
        assert_eq!(m.0[10], 0xaa);
        assert_eq!(m.0[7], 0xaa);
        s.depth = 1;
        s.write(&mut m, 3, -2, 1, 0);
        s.write(&mut m, 4, -2, 0, 0);
        assert_eq!(s.read(&mut m, 3, -2), Some(1));
        assert_eq!(s.read(&mut m, 4, -2), Some(0));
        assert_eq!(m.0[8] & 0x3f, 1);
        assert_eq!(s.read(&mut m, 3, 0), None);
        s.base = u32::MAX;
        assert_eq!(s.address(3, -1), None);
    }
    #[test]
    fn full_rgb_matching_is_stable_and_uses_64_bit_distance() {
        assert_eq!(nearest([65535; 3], &[[0; 3], [65535; 3]]), Some(1));
        assert_eq!(nearest([5; 3], &[[0; 3], [10; 3]]), Some(0));
        assert_eq!(nearest([0; 3], &[]), None);
    }
}

/// Classic 8-bit color cube followed by component ramps and black. This is
/// generated arithmetically; no system resource or observation file is loaded.
pub fn default_palette() -> alloc::vec::Vec<[u16; 3]> {
    let mut table = alloc::vec::Vec::with_capacity(256);
    for r in (0..6).rev() {
        for g in (0..6).rev() {
            for b in (0..6).rev() {
                if r | g | b != 0 {
                    table.push([r * 0x3333, g * 0x3333, b * 0x3333]);
                }
            }
        }
    }
    for channel in 0..4 {
        for step in [14, 13, 11, 10, 8, 7, 5, 4, 2, 1] {
            let mut rgb = [0; 3];
            for (i, v) in rgb.iter_mut().enumerate() {
                if channel == 3 || channel == i {
                    *v = step * 0x1111;
                }
            }
            table.push(rgb);
        }
    }
    table.push([0; 3]);
    table
}
