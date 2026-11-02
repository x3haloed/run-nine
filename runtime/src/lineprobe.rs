//! Classic bitmap line raster and pen state, with no native output replay.
use crate::{
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
fn state(m: &mut Memory, q: &QuickDraw, p: u32) -> Result<String, String> {
    let mut s = hex(m, p + 48, 10);
    for b in q.pattern(m, p, 58)? {
        s.push_str(&format!("{b:02x}"));
    }
    Ok(s)
}
pub fn probe() -> Result<String, String> {
    let starts = [
        (3i16, 4i16),
        (15, 4),
        (5, 3),
        (3, 3),
        (20, 3),
        (3, 3),
        (20, 10),
        (-4, -3),
        (8, 8),
        (3, 3),
        (3, 3),
        (3, 3),
    ];
    let ends = [
        (15i16, 4i16),
        (3, 4),
        (5, 19),
        (20, 20),
        (3, 20),
        (20, 10),
        (3, 3),
        (35, 38),
        (8, 8),
        (18, 9),
        (18, 9),
        (18, 9),
    ];
    let sizes = [
        (1i16, 1i16),
        (1, 1),
        (1, 1),
        (1, 1),
        (1, 1),
        (3, 2),
        (3, 2),
        (2, 3),
        (1, 1),
        (0, 3),
        (1, 1),
        (-2, 1),
    ];
    let patterns = [
        [255u8; 8],
        [170, 85, 170, 85, 170, 85, 170, 85],
        [129, 66, 36, 24, 24, 36, 66, 129],
    ];
    let mut records = Vec::new();
    for which in 0..12 {
        for op in 0..2 {
            for seed in [0x12345678u32, 0] {
                for flags in [16, 31, 0] {
                    let mut m = Memory::zeroed(0x800000);
                    let mut heap = Heap::new(0x20000, 0xe0000);
                    let mut q = QuickDraw::new();
                    q.init(&mut m, 0x100000)?;
                    let p = 0x12000;
                    q.open_port(&mut m, &mut heap, p)?;
                    q.set_port(&mut m, p)?;
                    let bounds = Rect {
                        top: 0,
                        left: 0,
                        bottom: 32,
                        right: 32,
                    };
                    m.write_long(p + 2, 0x16000);
                    m.write_word(p + 6, 4);
                    quickdraw::write_rect(&mut m, p + 8, bounds);
                    quickdraw::write_rect(&mut m, p + 16, bounds);
                    for offset in [24, 28] {
                        let h = m.read_long(p + offset);
                        let d = m.read_long(h);
                        quickdraw::write_rect(&mut m, d + 2, bounds);
                    }
                    q.pen_size(&mut m, sizes[which].0, sizes[which].1)?;
                    q.move_to(&mut m, starts[which].0, starts[which].1)?;
                    let pattern = if which == 3 || which == 6 {
                        1
                    } else if which == 4 || which == 7 {
                        2
                    } else {
                        0
                    };
                    for (i, b) in patterns[pattern].into_iter().enumerate() {
                        m.write_byte(0x14000 + i as u32, b);
                    }
                    q.pen_pattern(&mut m, 0x14000)?;
                    q.set_word(&mut m, 56, if which == 6 { 10 } else { 8 })?;
                    if which == 10 {
                        q.set_word(&mut m, 66, 0xffff)?;
                    }
                    let before = state(&mut m, &q, p)?;
                    let (h, v) = if op == 0 {
                        ends[which]
                    } else {
                        (
                            ends[which].0.wrapping_sub(starts[which].0),
                            ends[which].1.wrapping_sub(starts[which].1),
                        )
                    };
                    let sp = 0x17000;
                    let mut cpu = CpuCore::new();
                    cpu.pc = 0x18000;
                    cpu.dar[15] = sp;
                    cpu.dar[0] = seed;
                    cpu.set_ccr(flags);
                    m.write_word(sp, v as u16);
                    m.write_word(sp + 2, h as u16);
                    m.write_long(sp + 4, 0x55667788);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    assert!(quickdraw::line_dispatch(
                        if op == 0 { 0xa891 } else { 0xa892 },
                        &mut cpu,
                        &mut m,
                        &q
                    )?);
                    assert_eq!(cpu.pc, 0x18002);
                    records.push(json!({"which":which,"op":op,"input_d0":seed,"input_ccr":flags,"h":h,"v":v,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+4),"guard":m.read_long(sp+4),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"pen_visibility":m.read_word(p+66)as i16,"before_hex":before,"after_hex":state(&mut m,&q,p)?,"pixels_hex":hex(&mut m,0x16000,128)}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"qd-line-runtime-raw-v1","records":records}).to_string())
}
