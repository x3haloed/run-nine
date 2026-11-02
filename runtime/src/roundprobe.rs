//! Owned rounded-rectangle matrix; unsupported frames are explicit rejections.
use crate::{
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, QuickDraw},
    regions::Region,
    roundrect,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
pub fn probe() -> Result<String, String> {
    let rects = [
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [3, 4, 11, 27],
        [5, 5, 5, 25],
        [14, 14, 26, 26],
        [-3, -4, 25, 37],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [20, 5, 10, 25],
        [5, 5, 6, 6],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
        [2, 2, 30, 30],
    ];
    let ovals = [
        (16i16, 16i16),
        (8, 4),
        (15, 9),
        (0, 0),
        (40, 40),
        (16, 12),
        (16, 16),
        (4, 4),
        (15, 9),
        (16, 16),
        (16, 16),
        (-6, 8),
        (16, 16),
        (16, 16),
        (16, 16),
        (1, 1),
        (1, 16),
        (16, 1),
        (16, 16),
        (2, 2),
        (16, 16),
    ];
    let cross = [0x81u8, 0x42, 0x24, 0x18, 0x18, 0x24, 0x42, 0x81];
    let checker = [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55];
    let mut records = Vec::new();
    let mut rejections = 0;
    for which in 0..21 {
        for op in 0..5 {
            for seed in [0x12345678u32, 0] {
                for flags in [16u8, 31, 0] {
                    let mut m = Memory::zeroed(0x800000);
                    let mut heap = Heap::new(0x20000, 0xe0000);
                    let mut q = QuickDraw::new();
                    q.init(&mut m, 0x100000)?;
                    let port = 0x12000;
                    q.open_port(&mut m, &mut heap, port)?;
                    q.set_port(&mut m, port)?;
                    let bounds = Rect {
                        top: 0,
                        left: 0,
                        bottom: 32,
                        right: 32,
                    };
                    m.write_long(port + 2, 0x16000);
                    m.write_word(port + 6, 4);
                    quickdraw::write_rect(&mut m, port + 8, bounds);
                    quickdraw::write_rect(&mut m, port + 16, bounds);
                    let vis = m.read_long(port + 24);
                    Region::rectangle(bounds).store(&mut m, &mut heap, vis)?;
                    let clip = m.read_long(port + 28);
                    Region::rectangle(Rect {
                        top: 1,
                        left: 1,
                        bottom: 31,
                        right: 31,
                    })
                    .store(&mut m, &mut heap, clip)?;
                    for i in 0..128 {
                        m.write_byte(
                            0x16000 + i,
                            if op <= 1 {
                                0
                            } else if i % 2 == 1 {
                                0xcc
                            } else {
                                0x33
                            },
                        );
                    }
                    let pw: i16 = if which == 7 || which == 9 {
                        3
                    } else if which == 12 {
                        0
                    } else if which == 13 {
                        -2
                    } else if which == 18 {
                        20
                    } else {
                        1
                    };
                    let ph: i16 = if which == 7 || which == 9 {
                        2
                    } else if which == 12 {
                        3
                    } else if which == 18 {
                        20
                    } else {
                        1
                    };
                    m.write_word(port + 52, ph as u16);
                    m.write_word(port + 54, pw as u16);
                    m.write_word(
                        port + 56,
                        if which == 9 {
                            10
                        } else if which == 20 {
                            9
                        } else {
                            8
                        },
                    );
                    m.write_word(port + 66, if which == 10 { 0xffff } else { 0 });
                    m.put(
                        (port + 58) as usize,
                        if which == 9 || which == 20 {
                            &checker
                        } else {
                            &[255; 8]
                        },
                    )?;
                    m.put(
                        (port + 32) as usize,
                        if which == 20 { &cross } else { &[0; 8] },
                    )?;
                    q.move_to(&mut m, 5, 7)?;
                    let r = rects[which];
                    let rect = Rect {
                        top: r[0],
                        left: r[1],
                        bottom: r[2],
                        right: r[3],
                    };
                    let (ow, oh) = ovals[which];
                    let rp = 0x18000;
                    let pp = 0x19000;
                    quickdraw::write_rect(&mut m, rp, rect);
                    m.put(pp, &cross)?;
                    let sp = 0x700000;
                    let offset = if op == 4 { 4 } else { 0 };
                    if op == 4 {
                        m.write_long(sp, pp as u32);
                    }
                    m.write_word(sp + offset, oh as u16);
                    m.write_word(sp + offset + 2, ow as u16);
                    m.write_long(sp + offset + 4, rp);
                    let pop = offset + 8;
                    m.write_long(sp + pop, 0x55667788);
                    let before = hex(&mut m, port + 48, 18);
                    let initial = hex(&mut m, 0x16000, 128);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    let mut cpu = CpuCore::new();
                    cpu.pc = 0x6000;
                    cpu.dar[0] = seed;
                    cpu.dar[15] = sp;
                    cpu.set_ccr(flags);
                    let result = roundrect::dispatch(0xa8b0 + op, &mut cpu, &mut m, &q);
                    let error = match result {
                        Ok(true) => {
                            if cpu.pc != 0x6002 {
                                return Err("rounded primitive did not advance".into());
                            }
                            None
                        }
                        Ok(false) => return Err("rounded primitive not handled".into()),
                        Err(e) => {
                            if op != 0 || !matches!(which, 12 | 14) {
                                return Err(e);
                            }
                            if cpu.pc != 0x6000
                                || cpu.dar[15] != sp
                                || cpu.dar[0] != seed
                                || cpu.get_sr() & 31 != u16::from(flags)
                                || hex(&mut m, port + 48, 18) != before
                                || hex(&mut m, 0x16000, 128) != initial
                            {
                                return Err("unsupported frame changed state".into());
                            }
                            rejections += 1;
                            Some(e)
                        }
                    };
                    if op == 0 && matches!(which, 12 | 14) && error.is_none() {
                        return Err("unsupported frame returned success".into());
                    }
                    records.push(json!({"which":which,"op":op,"bounds":r,"oval_width":ow,"oval_height":oh,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+pop),"guard":m.read_long(sp+pop),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"pen_visibility":m.read_word(port+66)as i16,"before_pen_hex":before,"after_pen_hex":hex(&mut m,port+48,18),"before_pixels_hex":initial,"pixels_hex":hex(&mut m,0x16000,128),"error":error}));
                }
            }
        }
    }
    if rejections != 12 {
        return Err("incomplete unsupported-frame checks".into());
    }
    Ok(json!({"schema":1,"probe":"round-rect-runtime-raw-v1","records":records}).to_string())
}
