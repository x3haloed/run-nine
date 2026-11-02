//! Rounded-rectangle spans transfer through the common port/pattern renderer.
use crate::{
    managers::Rect,
    quickdraw::{read_rect, QuickDraw},
};
use m68k::{AddressBus, CpuCore};

fn span(r: Rect, ow: i16, oh: i16, y: i16) -> Option<(i16, i16)> {
    let width = i32::from(r.right) - i32::from(r.left);
    let height = i32::from(r.bottom) - i32::from(r.top);
    if width <= 0 || height <= 0 || y < r.top || y >= r.bottom {
        return None;
    }
    let cw = i32::from(ow).min(width);
    let ch = i32::from(oh).min(height);
    let cut = if cw <= 1 || ch <= 0 {
        0
    } else if ch == 1 {
        // Native one-row corners retain the narrowed middle until the final
        // bottom edge. This is a parameter-level compatibility quirk.
        if y == r.bottom - 1 {
            0
        } else {
            cw / 2
        }
    } else {
        let yy = (i32::from(y) - i32::from(r.top)).min(i32::from(r.bottom) - 1 - i32::from(y));
        if yy >= ch / 2 {
            0
        } else {
            let (cw, ch) = (i64::from(cw), i64::from(ch));
            let dy = 2 * i64::from(yy) + 1 - ch;
            let (mut lo, mut hi) = (0, cw / 2);
            while lo < hi {
                let mid = (lo + hi) / 2;
                let dx = 2 * mid + 1 - cw;
                if dx * dx * ch * ch + dy * dy * cw * cw <= cw * cw * ch * ch {
                    hi = mid;
                } else {
                    lo = mid + 1;
                }
            }
            lo as i32
        }
    };
    Some((
        (i32::from(r.left) + cut) as i16,
        (i32::from(r.right) - cut) as i16,
    ))
}
pub fn dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut CpuCore,
    m: &mut B,
    q: &QuickDraw,
) -> Result<bool, String> {
    if !(0xa8b0..=0xa8b4).contains(&op) {
        return Ok(false);
    }
    let sp = cpu.dar[15];
    let fill = op == 0xa8b4;
    let offset = if fill { 4 } else { 0 };
    let oh = m.read_word(sp + offset) as i16;
    let ow = m.read_word(sp + offset + 2) as i16;
    let rp = m.read_long(sp + offset + 4);
    let rect = read_rect(m, rp);
    let port = q.get_port(m)?;
    if port == 0 {
        return Err("rounded rectangle without current port".into());
    }
    let ph = m.read_word(port + 52) as i16;
    let pw = m.read_word(port + 54) as i16;
    if m.read_word(port + 66) as i16 >= 0 {
        if op == 0xa8b0 {
            if ph == 0 || pw == 0 {
                return Err("unsupported rounded frame with zero pen dimension".into());
            }
            if rect.top > rect.bottom || rect.left > rect.right {
                return Err("unsupported rounded frame with inverted bounds".into());
            }
        }
        let (pattern, mode) = match op {
            0xa8b0 | 0xa8b1 => (q.pattern(m, port, 58)?, m.read_word(port + 56)),
            0xa8b2 => (q.pattern(m, port, 32)?, 8),
            0xa8b3 => ([255; 8], 10),
            _ => {
                let ptr = m.read_long(sp);
                let mut p = [0; 8];
                for i in 0..8 {
                    p[i] = m.read_byte(ptr + i as u32);
                }
                (p, 8)
            }
        };
        let inner = Rect {
            top: rect.top.wrapping_add(ph),
            left: rect.left.wrapping_add(pw),
            bottom: rect.bottom.wrapping_sub(ph),
            right: rect.right.wrapping_sub(pw),
        };
        let iw = ow.wrapping_sub(pw.wrapping_mul(2));
        let ih = oh.wrapping_sub(ph.wrapping_mul(2));
        let bitmap = q.port_bitmap(m, port)?;
        let bounds = read_rect(m, bitmap + 6);
        for y in rect.top.max(bounds.top)..rect.bottom.min(bounds.bottom) {
            let Some((left, right)) = span(rect, ow, oh, y) else {
                continue;
            };
            let paint = |m: &mut B, l: i16, r: i16| -> Result<(), String> {
                if l < r {
                    q.paint_rect(
                        m,
                        Rect {
                            top: y,
                            left: l,
                            bottom: y + 1,
                            right: r,
                        },
                        pattern,
                        mode,
                    )?;
                }
                Ok(())
            };
            if let Some((il, ir)) = if op == 0xa8b0 {
                span(inner, iw, ih, y)
            } else {
                None
            } {
                if ir <= left || il >= right {
                    paint(m, left, right)?;
                } else {
                    paint(m, left, right.min(il))?;
                    paint(m, left.max(ir), right)?;
                }
            } else {
                paint(m, left, right)?;
            }
        }
    }
    cpu.dar[15] += if fill { 12 } else { 8 };
    cpu.pc += 2;
    cpu.set_ccr(0);
    Ok(true)
}
