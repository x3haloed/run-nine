//! Monochrome icon-resource plotting for the independent one-bit device profile.
use crate::{
    quickdraw::{read_rect, QuickDraw},
    resource::Resource,
};
use m68k::{AddressBus, CpuCore};
pub fn dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut CpuCore,
    m: &mut B,
    q: &QuickDraw,
    resources: &[Resource<'_>],
) -> Result<bool, String> {
    if op != 0xabc9 {
        return Ok(false);
    }
    if cpu.dar[0] as u16 != 0x0500 {
        return Err(format!(
            "IconDispatch selector {:#x} is not implemented",
            cpu.dar[0] as u16
        ));
    }
    let sp = cpu.dar[15];
    let id = m.read_word(sp) as i16;
    let transform = m.read_word(sp + 2);
    let align = m.read_word(sp + 4);
    let rect_pointer = m.read_long(sp + 6);
    let rect = read_rect(m, rect_pointer);
    if transform != 0 || align != 0 {
        return Err("PlotIconID alignment and transforms are not implemented".into());
    }
    let width = rect.right as i32 - rect.left as i32;
    let height = rect.bottom as i32 - rect.top as i32;
    let preferred = match (width, height) {
        (16, 16) => *b"ics#",
        (32, 32) => *b"ICN#",
        _ => return Err("PlotIconID supports16x16 and32x32 rectangles in this profile".into()),
    };
    let found = resources
        .iter()
        .find(|r| r.kind == preferred && r.id == id)
        .or_else(|| resources.iter().find(|r| r.kind == *b"ICN#" && r.id == id));
    let error = if let Some(resource) = found {
        let data =
            crate::vendor::compressed::decompress_if_needed(resource.attributes, resource.data)
                .map_err(|e| format!("icon decompression: {e:?}"))?
                .unwrap_or_else(|| resource.data.to_vec());
        let size = if resource.kind == *b"ics#" { 16 } else { 32 };
        let plane = size * size / 8;
        if data.len() != plane * 2 {
            return Err("invalid monochrome icon and mask resource size".into());
        }
        let port = q.get_port(m)?;
        if m.read_long(port + 80) != 33 || m.read_long(port + 84) != 30 {
            return Err(
                "PlotIconID nondefault foreground/background colors are not implemented".into(),
            );
        }
        for y in 0..height {
            for x in 0..width {
                let sx = x as usize * size / width as usize;
                let sy = y as usize * size / height as usize;
                let offset = sy * (size / 8) + sx / 8;
                let bit = 0x80 >> (sx & 7);
                if data[plane + offset] & bit != 0 {
                    q.transfer(
                        m,
                        port,
                        rect.left + x as i16,
                        rect.top + y as i16,
                        data[offset] & bit != 0,
                        0,
                    )?;
                }
            }
        }
        0i16
    } else {
        -1000i16
    };
    m.write_word(sp + 10, error as u16);
    cpu.dar[15] += 10;
    cpu.pc += 2;
    Ok(true)
}

/// Owned geometric substitutes for the three standard32px alert symbols.
/// These are not copied Apple icon artwork or a pixel-equivalence claim.
pub fn alert_bitmap(id: i16) -> Option<Vec<u8>> {
    if !(0..=2).contains(&id) {
        return None;
    }
    let mut bitmap = vec![0u8; 128];
    for y in 0i16..32 {
        for x in 0i16..32 {
            let inside = |x: i16, y: i16| {
                x >= 0
                    && y >= 0
                    && x < 32
                    && y < 32
                    && x + y >= 8
                    && x + y <= 54
                    && (x - y).abs() <= 23
            };
            let exclamation = x >= 14 && x <= 17 && ((y >= 7 && y <= 21) || (y >= 25 && y <= 27));
            let bit = match id {
                0 => {
                    inside(x, y)
                        && (!inside(x - 2, y)
                            || !inside(x + 2, y)
                            || !inside(x, y - 2)
                            || !inside(x, y + 2)
                            || exclamation)
                }
                2 => {
                    let edge = (2 * x - 31).abs();
                    y >= 1 && edge <= y && (y >= 29 || edge >= y - 3 || exclamation)
                }
                _ => {
                    let radius = (2 * x - 31).pow(2) + (2 * y - 31).pow(2);
                    (radius <= 31 * 31 && radius >= 26 * 26)
                        || (x >= 14 && x <= 17 && ((y >= 12 && y <= 26) || (y >= 6 && y <= 9)))
                }
            };
            if bit {
                bitmap[y as usize * 4 + x as usize / 8] |= 0x80 >> (x & 7);
            }
        }
    }
    Some(bitmap)
}
