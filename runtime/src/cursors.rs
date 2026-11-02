//! Independently drawn monochrome cursors in the public 68-byte Cursor layout.
pub fn builtin(id: i16) -> Option<[u8; 68]> {
    let mut bytes = [0; 68];
    let mut pixel = |y: usize, x: usize| {
        bytes[y * 2 + x / 8] |= 0x80 >> (x % 8);
        bytes[32 + y * 2 + x / 8] |= 0x80 >> (x % 8);
    };
    match id {
        0 => {
            for y in 0..12 {
                for x in 0..=y / 2 {
                    pixel(y, x);
                }
            }
        }
        1 => {
            for y in 1..15 {
                pixel(y, 7);
            }
            for x in 4..11 {
                pixel(1, x);
                pixel(14, x);
            }
        }
        2 | 3 => {
            for i in 1..15 {
                pixel(7, i);
                pixel(i, 7);
            }
        }
        4 => {
            for y in 2..14 {
                for x in 2..14 {
                    if (x == 2 || x == 13 || y == 2 || y == 13)
                        || (x == 7 && y < 8)
                        || (y == 7 && x > 7)
                    {
                        pixel(y, x);
                    }
                }
            }
        }
        _ => return None,
    }
    if id != 0 {
        bytes[64..66].copy_from_slice(&7u16.to_be_bytes());
        bytes[66..68].copy_from_slice(&7u16.to_be_bytes());
    }
    Some(bytes)
}

/// Adapter supplies the owned arrow image; visibility rules live in shared core.
pub fn dispatch(op: u16, cpu: &mut m68k::CpuCore, m: &mut crate::machine::Memory) -> Result<bool, String> {
    if !classic_core::cursor::dispatch(op, cpu, m) { return Ok(false); }
    if op == 0xa850 { m.put(0x844, &builtin(0).unwrap())?; }
    Ok(true)
}
