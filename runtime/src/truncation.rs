//! Roman QuickDraw text truncation, using the shared current-port font metrics.
use crate::{fonts::Fonts, quickdraw::QuickDraw};
use m68k::{AddressBus, CpuCore};

pub fn dispatch<B: AddressBus>(
    opcode: u16,
    cpu: &mut CpuCore,
    m: &mut B,
    q: &QuickDraw,
    fonts: &Fonts,
) -> Result<bool, String> {
    if opcode != 0xa8b5 {
        return Ok(false);
    }
    let sp = cpu.dar[15];
    let string = match m.read_long(sp) {
        0x820cffde => false,
        0x8208ffe0 => true,
        _ => return Ok(false),
    };
    let code = m.read_word(sp + 4);
    if !matches!(code, 0 | 0x4000) {
        return Err(format!("unsupported text truncation code {code:#x}"));
    }
    let pop = if string { 12 } else { 16 };
    let ptr = m.read_long(sp + if string { 6 } else { 10 });
    let lp = if string { 0 } else { m.read_long(sp + 6) };
    let length = if string {
        m.read_byte(ptr) as i16
    } else {
        m.read_word(lp) as i16
    };
    if length < 0 || ptr == 0 || (!string && lp == 0) {
        return Err("invalid text truncation buffer/length".into());
    }
    let width = m.read_word(sp + pop - 2) as i16;
    let start = ptr + u32::from(string);
    let text: Vec<u8> = (0..length as u32).map(|i| m.read_byte(start + i)).collect();
    let mut result = 0i16;
    let mut new_length = text.len();
    if !text.is_empty() {
        let port = q.get_port(m)?;
        if port == 0 {
            return Err("text truncation without current port".into());
        }
        if m.read_long(port + 76) != 0 {
            return Err("text truncation with SpaceExtra".into());
        }
        let font = m.read_word(port + 68) as i16;
        let size = m.read_word(port + 74) as i16;
        let face = m.read_byte(port + 70);
        if fonts.text_width(font, size, face, &text)? > width {
            // Native Roman truncation clears these error words when it enters
            // the ellipsis path, including a width too small for the ellipsis.
            m.write_word(0x220, 0);
            m.write_word(0xa60, 0);
            let widths = &fonts.select(font, size, face)?.widths;
            let budget = i32::from(width) - i32::from(widths[0xc9]);
            new_length = 0;
            result = -1;
            if budget >= 0 {
                let (mut left, mut right, mut used) = (0usize, 0usize, 0i32);
                while left + right < text.len() {
                    let suffix = code == 0x4000 && (left + right) % 2 == 1;
                    let at = if suffix { text.len() - right - 1 } else { left };
                    let advance = i32::from(widths[text[at] as usize]);
                    if used + advance > budget {
                        break;
                    }
                    used += advance;
                    if suffix {
                        right += 1;
                    } else {
                        left += 1;
                    }
                }
                let out = text[..left]
                    .iter()
                    .copied()
                    .chain([0xc9])
                    .chain(text[text.len() - right..].iter().copied());
                for (i, byte) in out.enumerate() {
                    m.write_byte(start + i as u32, byte);
                }
                new_length = left + 1 + right;
                result = 1;
            }
        }
    }
    if string {
        m.write_byte(ptr, new_length as u8);
    } else {
        m.write_word(lp, new_length as u16);
    }
    m.write_word(sp + pop, result as u16);
    cpu.dar[15] += pop;
    cpu.pc += 2;
    // D0 is unclaimed ROM scratch. Empty text leaves X set; other cases clear CCR.
    cpu.set_ccr(if length == 0 { 16 } else { 0 });
    Ok(true)
}
