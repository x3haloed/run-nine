//! Software indexed-color device services. No native reference data is loaded.
use crate::{
    managers::Heap,
    quickdraw::{read_rect, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
fn rgb<B: AddressBus>(m: &mut B, p: u32) -> [u16; 3] {
    [m.read_word(p), m.read_word(p + 2), m.read_word(p + 4)]
}
fn put_rgb<B: AddressBus>(m: &mut B, p: u32, color: [u16; 3]) {
    for (i, v) in color.iter().enumerate() {
        m.write_word(p + i as u32 * 2, *v);
    }
}
fn set_rgb<B: AddressBus>(
    q: &QuickDraw,
    m: &mut B,
    port: u32,
    foreground: bool,
    color: [u16; 3],
) -> Result<(), String> {
    if m.read_word(port + 6) & 0xc000 != 0xc000 {
        return Err("RGB assignment to legacy GrafPort is outside current profile".into());
    }
    let index = q.color_index(m, color)?;
    put_rgb(m, port + if foreground { 36 } else { 42 }, color);
    m.write_long(port + if foreground { 80 } else { 84 }, index as u32);
    Ok(())
}
pub fn dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut CpuCore,
    m: &mut B,
    heap: &mut Heap,
    q: &mut QuickDraw,
) -> Result<bool, String> {
    let sp = cpu.dar[15];
    let pop = match op {
        0xa861 => {
            let value = q.random(m)?;
            m.write_word(sp, value as u16);
            cpu.dar[0] = value;
            cpu.set_ccr(
                16 | if value as i16 == 0 {
                    4
                } else if (value as i16) < 0 {
                    8
                } else {
                    0
                },
            );
            0
        }
        0xaa19 | 0xaa1a => {
            let destination = m.read_long(sp);
            let port = q.get_port(m)?;
            let color = q.port_rgb(m, port, op == 0xaa19);
            put_rgb(m, destination, color);
            cpu.set_ccr(0);
            4
        }
        0xa862 | 0xa863 => {
            let value = m.read_long(sp);
            let color = classic_core::legacy_colors::rgb(value)
                .ok_or("legacy QuickDraw color outside documented eight-color profile")?;
            let port = q.get_port(m)?;
            let foreground = op == 0xa862;
            if m.read_word(port + 6) & 0xc000 == 0xc000 {
                let index = q.color_index(m, color)?;
                put_rgb(m, port + if foreground { 36 } else { 42 }, color);
                m.write_long(port + if foreground { 80 } else { 84 }, u32::from(index));
            } else {
                m.write_long(port + if foreground { 80 } else { 84 }, value);
            }
            cpu.set_ccr(0);
            4
        }
        0xaa39 => {
            let res = m.read_word(sp);
            let it = m.read_long(sp + 2);
            let ct = m.read_long(sp + 6);
            q.make_inverse(m, heap, ct, it, res)?;
            // MakeITable is void; D0/NZVC are caller-scratch, unlike its
            // documented table contents, stack layout and QDError result.
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            10
        }
        0xaa40 => {
            m.write_word(sp, q.draw_error as u16);
            0
        }
        0xaa00 => {
            let port = m.read_long(sp);
            q.open_color_port(m, heap, port)?;
            4
        }
        0xaa02 => {
            let port = m.read_long(sp);
            q.close_port(m, heap, port)?;
            4
        }
        0xaaa2 => {
            let selector = cpu.dar[0] as u16;
            if !matches!(selector, 0x0a13 | 0x0a14) {
                return Ok(false);
            }
            let flags = m.read_word(sp);
            let which = m.read_word(sp + 2);
            let depth = m.read_word(sp + 4);
            let device = m.read_long(sp + 6);
            let own = q.device(m, heap)?;
            if device != own {
                return Err("depth operation on foreign graphics device".into());
            }
            if flags > 1 || which > 1 {
                return Err("depth flags outside measured profile".into());
            }
            let result = if selector == 0x0a14 {
                q.has_depth(depth) as i16
            } else {
                q.set_depth(m, heap, depth).err().unwrap_or(0)
            };
            m.write_word(sp + 10, result as u16);
            cpu.set_ccr(if selector == 0x0a14 {
                if result == 0 {
                    25
                } else {
                    0
                }
            } else if result == 0 {
                4
            } else {
                16
            });
            10
        }
        0xaa14 | 0xaa15 => {
            let p = m.read_long(sp);
            let color = rgb(m, p);
            let port = q.get_port(m)?;
            set_rgb(q, m, port, op == 0xaa14, color)?;
            4
        }
        0xaa33 => {
            let p = m.read_long(sp);
            let color = rgb(m, p);
            let index = q.color_index(m, color)?;
            m.write_long(sp + 4, index as u32);
            4
        }
        0xaa34 => {
            let target = m.read_long(sp);
            let index = m.read_long(sp + 4) as usize;
            let b = q.device_bitmap(m)?;
            let colors = q.palette(m, b)?;
            let color = *colors.get(index).ok_or("Index2Color outside table")?;
            put_rgb(m, target, color);
            8
        }
        0xaa3f => {
            let source = m.read_long(sp);
            let count = m.read_word(sp + 4) as i16;
            let start = m.read_word(sp + 6) as i16;
            if q.depth != 8 || count < 0 || count > 255 || start < -1 {
                return Err("SetEntries outside indexed profile".into());
            }
            let b = q.device_bitmap(m)?;
            let h = m.read_long(b + 42);
            let table = m.read_long(h);
            let mut entries = Vec::new();
            for i in 0..=count as u32 {
                let index = if start == -1 {
                    m.read_word(source + i * 8) as u32
                } else {
                    start as u32 + i
                };
                if index > 255 {
                    return Err("SetEntries outside table".into());
                }
                entries.push((index, rgb(m, source + i * 8 + 2)));
            }
            for (index, color) in entries {
                m.write_word(table + 8 + index * 8, 0);
                put_rgb(m, table + 10 + index * 8, color);
            }
            let seed = q.next_color_seed();
            m.write_long(table, seed);
            8
        }
        0xaa16 | 0xaa17 => {
            let pointer = m.read_long(sp);
            let y = m.read_word(sp + 4) as i16;
            let x = m.read_word(sp + 6) as i16;
            let port = q.get_port(m)?;
            let surface = q.port_surface(m, port)?;
            if op == 0xaa16 {
                let color = rgb(m, pointer);
                let index = if surface.depth == 8 {
                    q.color_index(m, color)?
                } else {
                    u8::from(color.iter().map(|&v| v as u32).sum::<u32>() < 3 * 32768)
                };
                let vis = m.read_long(port + 24);
                let clip = m.read_long(port + 28);
                if crate::regions::contains(m, vis, y, x)?
                    && crate::regions::contains(m, clip, y, x)?
                {
                    surface.write(m, x, y, index, 0);
                }
            } else {
                let index = surface.read(m, x, y).ok_or("GetCPixel outside bounds")?;
                let color = if surface.depth == 1 {
                    if index == 0 {
                        [65535; 3]
                    } else {
                        [0; 3]
                    }
                } else {
                    let b = q.device_bitmap(m)?;
                    q.palette(m, b)?[index as usize]
                };
                put_rgb(m, pointer, color);
            }
            8
        }
        0xa8ec => {
            copy_bits(q, m, sp)?;
            22
        }
        _ => return Ok(false),
    };
    cpu.dar[15] += pop;
    cpu.pc += 2;
    Ok(true)
}
fn resolve<B: AddressBus>(m: &mut B, p: u32) -> u32 {
    if m.read_word(p + 4) & 0xc000 == 0xc000 {
        let h = m.read_long(p);
        m.read_long(h)
    } else {
        p
    }
}
fn copy_bits<B: AddressBus>(q: &QuickDraw, m: &mut B, sp: u32) -> Result<(), String> {
    let mask = m.read_long(sp);
    let mode = m.read_word(sp + 4);
    let drp = m.read_long(sp + 6);
    let srp = m.read_long(sp + 10);
    let db = m.read_long(sp + 14);
    let sb = m.read_long(sp + 18);
    if mode != 0 {
        return Err(format!(
            "CopyBits transfer mode {mode} is outside current profile"
        ));
    }
    let db = resolve(m, db);
    let sb = resolve(m, sb);
    let dest = q.surface(m, db)?;
    let source = q.surface(m, sb)?;
    let dr = read_rect(m, drp);
    let sr = read_rect(m, srp);
    let w = dr.right as i32 - dr.left as i32;
    let h = dr.bottom as i32 - dr.top as i32;
    let sw = sr.right as i32 - sr.left as i32;
    let sh = sr.bottom as i32 - sr.top as i32;
    if w <= 0 || h <= 0 || sw <= 0 || sh <= 0 {
        return Ok(());
    }
    if w as u64 * h as u64 > 2_000_000 {
        return Err("CopyBits exceeds bounded software surface".into());
    }
    if source.depth == 8 && dest.depth != 8 {
        return Err("indexed-to-monochrome CopyBits outside current profile".into());
    }
    let port = q.get_port(m)?;
    let fg = q.port_rgb(m, port, true);
    let bg = q.port_rgb(m, port, false);
    let palette = if source.depth == 8 {
        q.palette(m, sb)?
    } else {
        vec![[65535; 3], [0; 3]]
    };
    let target = if dest.depth == 8 {
        q.palette(m, db)?
    } else {
        vec![[65535; 3], [0; 3]]
    };
    let neutral = fg == [0; 3] && bg == [65535; 3];
    let mapping = if neutral && source.depth == dest.depth && palette == target {
        (0..palette.len()).map(|i| i as u8).collect::<Vec<_>>()
    } else {
        palette
            .iter()
            .map(|color| {
                let rgb = core::array::from_fn(|i| (color[i] & bg[i]) | (!color[i] & fg[i]));
                classic_core::pixels::nearest(rgb, &target).ok_or("empty destination table")
            })
            .collect::<Result<Vec<_>, _>>()?
    };
    let mut pixels = Vec::with_capacity(w as usize * h as usize);
    for y in 0..h {
        for x in 0..w {
            let sx = (sr.left as i32 + x * sw / w) as i16;
            let sy = (sr.top as i32 + y * sh / h) as i16;
            pixels.push(source.read(m, sx, sy));
        }
    }
    let vis = m.read_long(port + 24);
    let clip = m.read_long(port + 28);
    for y in 0..h {
        for x in 0..w {
            let dx = (dr.left as i32 + x) as i16;
            let dy = (dr.top as i32 + y) as i16;
            if mask != 0 && !crate::regions::contains(m, mask, dy, dx)? {
                continue;
            }
            if !crate::regions::contains(m, vis, dy, dx)?
                || !crate::regions::contains(m, clip, dy, dx)?
            {
                continue;
            }
            if let Some(index) = pixels[(y * w + x) as usize] {
                let value = *mapping
                    .get(index as usize)
                    .ok_or("source index outside color table")?;
                dest.write(m, dx, dy, value, 0);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct M(Vec<u8>);
    impl AddressBus for M {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
        fn read_word(&mut self, p: u32) -> u16 {
            u16::from_be_bytes([self.read_byte(p), self.read_byte(p + 1)])
        }
        fn read_long(&mut self, p: u32) -> u32 {
            ((self.read_word(p) as u32) << 16) | self.read_word(p + 2) as u32
        }
        fn write_word(&mut self, p: u32, v: u16) {
            let b = v.to_be_bytes();
            self.write_byte(p, b[0]);
            self.write_byte(p + 1, b[1]);
        }
        fn write_long(&mut self, p: u32, v: u32) {
            self.write_word(p, (v >> 16) as u16);
            self.write_word(p + 2, v as u16);
        }
    }
    fn scene() -> (M, Heap, QuickDraw) {
        let mut m = M(vec![0; 0x800000]);
        let mut heap = Heap::new(0x20000, 0x500000);
        let mut q = QuickDraw::new();
        q.color_capable = true;
        q.init(&mut m, 0x100000).unwrap();
        q.device(&mut m, &mut heap).unwrap();
        q.set_depth(&mut m, &mut heap, 8).unwrap();
        q.open_port(&mut m, &mut heap, 0x1c000).unwrap();
        q.make_color(&mut m, &mut heap, 0x1c000).unwrap();
        (m, heap, q)
    }
    #[test]
    fn set_entries_invalidates_rgb_match_cache() {
        let (mut m, mut heap, mut q) = scene();
        assert_eq!(q.color_index(&mut m, [65535, 0, 0]).unwrap(), 35);
        for (p, index, color) in [(0x1a000, 17, [65535, 0, 0]), (0x1a008, 35, [0, 0, 65535])] {
            m.write_word(p, index);
            put_rgb(&mut m, p + 2, color);
        }
        m.write_long(0x700000, 0x1a000);
        m.write_word(0x700004, 1);
        m.write_word(0x700006, 65535);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        assert!(dispatch(0xaa3f, &mut cpu, &mut m, &mut heap, &mut q).unwrap());
        assert_eq!(q.color_index(&mut m, [65535, 0, 0]).unwrap(), 17);
    }
    #[test]
    fn overlapping_copy_preserves_unread_source_and_destination_guards() {
        let (mut m, _heap, q) = scene();
        let bitmap = q.device_bitmap(&mut m).unwrap();
        let base = q.screen_base;
        for (i, v) in [16, 17, 18, 19, 20, 0xaa].into_iter().enumerate() {
            m.write_byte(base + i as u32, v);
        }
        crate::quickdraw::write_rect(
            &mut m,
            0x1a000,
            crate::managers::Rect {
                top: 0,
                left: 0,
                bottom: 1,
                right: 4,
            },
        );
        crate::quickdraw::write_rect(
            &mut m,
            0x1a010,
            crate::managers::Rect {
                top: 0,
                left: 1,
                bottom: 1,
                right: 5,
            },
        );
        m.write_long(0x700000, 0);
        m.write_word(0x700004, 0);
        m.write_long(0x700006, 0x1a010);
        m.write_long(0x70000a, 0x1a000);
        m.write_long(0x70000e, bitmap);
        m.write_long(0x700012, bitmap);
        copy_bits(&q, &mut m, 0x700000).unwrap();
        assert_eq!(
            &m.0[base as usize..base as usize + 6],
            &[16, 16, 17, 18, 19, 0xaa]
        );
    }
}
