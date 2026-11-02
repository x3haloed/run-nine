//! Fixed owned inputs replayed through production ScriptUtil truncation dispatch.
use crate::{fonts::Fonts, machine::Memory, managers::Heap, quickdraw::QuickDraw, truncation};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, ptr: u32, size: u32) -> String {
    (0..size)
        .map(|i| format!("{:02x}", m.read_byte(ptr + i)))
        .collect()
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let port = 0x12000;
    q.open_port(&mut m, &mut heap, port)?;
    q.set_port(&mut m, port)?;
    let texts: [&[u8]; 4] = [b"", b"Marathon", b"Alpha Beta Gamma", b"\x80\xc9\xd5Z"];
    let mut records = Vec::new();
    for op in 0..2 {
        for fontctx in 0..2 {
            for (which, text) in texts.iter().enumerate() {
                for mode in 0..2 {
                    let font = if fontctx == 0 { 1 } else { 0 };
                    let size = if fontctx == 0 { 12 } else { 0 };
                    m.write_word(port + 68, font as u16);
                    m.write_word(port + 74, size as u16);
                    m.write_byte(port + 70, 0);
                    let full = fonts.text_width(font, size, 0, text)?;
                    let ellipsis = fonts.text_width(font, size, 0, &[0xc9])?;
                    let widths = [
                        -1,
                        0,
                        ellipsis - 1,
                        ellipsis,
                        ellipsis + 1,
                        full - 1,
                        full,
                        full + 1,
                        20,
                    ];
                    for (w, width) in widths.iter().enumerate() {
                        for seed in [0x12345678u32, 0] {
                            for flags in [16u16, 31, 0] {
                                let ptr = 0x18004;
                                let lp = 0x19004;
                                let sp = 0x700000;
                                m.write_long(ptr - 4, 0xa1b2c3d4);
                                m.write_long(ptr + 256, 0x10203040);
                                for i in 0..256 {
                                    m.write_byte(ptr + i, 0xa5);
                                }
                                let start = ptr + op;
                                if op == 1 {
                                    m.write_byte(ptr, text.len() as u8);
                                }
                                m.put(start as usize, text)?;
                                m.write_long(lp - 4, 0x11223344);
                                m.write_word(lp, text.len() as u16);
                                m.write_long(lp + 2, 0x99aabbcc);
                                let before = hex(&mut m, ptr, 256);
                                let pop = if op == 1 { 12 } else { 16 };
                                m.write_long(sp, if op == 1 { 0x8208ffe0 } else { 0x820cffde });
                                m.write_word(sp + 4, if mode == 1 { 0x4000 } else { 0 });
                                if op == 0 {
                                    m.write_long(sp + 6, lp);
                                }
                                m.write_long(sp + if op == 1 { 6 } else { 10 }, ptr);
                                m.write_word(sp + pop - 2, *width as u16);
                                m.write_word(sp + pop, 0x7b7b);
                                m.write_long(sp + pop + 2, 0x55667788);
                                m.write_word(0x220, (-123i16) as u16);
                                m.write_word(0xa60, (-192i16) as u16);
                                let mut cpu = CpuCore::new();
                                cpu.dar[0] = seed;
                                cpu.dar[15] = sp;
                                cpu.pc = 0x6000;
                                cpu.set_ccr(flags as u8);
                                if !truncation::dispatch(0xa8b5, &mut cpu, &mut m, &q, &fonts)?
                                    || cpu.pc != 0x6002
                                {
                                    return Err("truncation dispatch did not return".into());
                                }
                                let after_sp = cpu.dar[15];
                                records.push(json!({"op":op,"fontctx":fontctx,"which":which,"mode":mode,"width_case":w,"width":width,"full_width":full,"ellipsis_width":ellipsis,"input_d0":seed,"input_ccr":flags,"before_length":text.len(),"before_hex":before,"result":m.read_word(after_sp)as i16,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":(after_sp+2)as i32-(sp+pop+2)as i32,"guard":m.read_long(after_sp+2),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"prefix":m.read_long(ptr-4),"suffix":m.read_long(ptr+256),"length_prefix":m.read_long(lp-4),"length_suffix":m.read_long(lp+2),"length_word":m.read_word(lp)as i16,"after_length":if op==1 {m.read_byte(ptr)as i16} else {m.read_word(lp)as i16},"after_hex":hex(&mut m,ptr,256)}));
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"text-truncation-runtime-raw-v1","records":records}).to_string())
}
