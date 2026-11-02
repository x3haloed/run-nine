//! Renderer-boundary observations of actual TETextBox dispatch on owned inputs.
use crate::{
    callbacks::Callbacks,
    events::Events,
    fonts::Fonts,
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, QuickDraw},
    regions::Region,
    textedit::TextEdit,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
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
    let bounds = Rect {
        top: 0,
        left: 0,
        bottom: 64,
        right: 96,
    };
    m.write_long(port + 2, 0x16000);
    m.write_word(port + 6, 12);
    quickdraw::write_rect(&mut m, port + 8, bounds);
    quickdraw::write_rect(&mut m, port + 16, bounds);
    let vis = m.read_long(port + 24);
    Region::rectangle(bounds).store(&mut m, &mut heap, vis)?;
    let clip = m.read_long(port + 28);
    let original = Rect {
        top: 1,
        left: 1,
        bottom: 45,
        right: 80,
    };
    let texts: [&[u8]; 8] = [
        b"",
        b"Marathon",
        b"Alpha Beta Gamma",
        b"line one\rline two\r",
        b"wideWWW WW  tail",
        b"\x80\xc9\xd5Z",
        b"tail   ",
        b"tail  \rnext",
    ];
    let mut te = TextEdit::new();
    let events = Events::new();
    let mut callbacks = Callbacks::silent();
    let mut records = Vec::new();
    for (which, text) in texts.into_iter().enumerate() {
        for fontctx in 0..2 {
            for (heightcase, height) in [0i16, 1, 24, 56].into_iter().enumerate() {
                for just in [0i16, 1, -1] {
                    for seed in [0x12345678u32, 0] {
                        for flags in [16u8, 31, 0] {
                            let rect = Rect {
                                top: 2,
                                left: 3,
                                bottom: 2 + height,
                                right: 63,
                            };
                            Region::rectangle(original).store(&mut m, &mut heap, clip)?;
                            for i in 0..768 {
                                m.write_byte(0x16000 + i, 0x5a);
                            }
                            m.write_word(port + 68, if fontctx == 0 { 3 } else { 0 });
                            m.write_word(port + 74, if fontctx == 0 { 12 } else { 0 });
                            m.write_byte(port + 70, 0);
                            m.write_word(port + 72, 1);
                            m.write_word(port + 52, 2);
                            m.write_word(port + 54, 3);
                            m.write_word(port + 56, 10);
                            for (i, b) in [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]
                                .into_iter()
                                .enumerate()
                            {
                                m.write_byte(port + 58 + i as u32, b);
                            }
                            q.move_to(&mut m, 19, 23)?;
                            let before_pen = hex(&mut m, port + 48, 18);
                            let before_text = hex(&mut m, port + 68, 20);
                            let sp = 0x700000;
                            let ptr = 0x18000;
                            let rp = 0x19000;
                            m.put(ptr, text)?;
                            quickdraw::write_rect(&mut m, rp, rect);
                            m.write_word(sp, just as u16);
                            m.write_long(sp + 2, rp);
                            m.write_long(sp + 6, text.len() as u32);
                            m.write_long(sp + 10, ptr as u32);
                            m.write_long(sp + 14, 0x55667788);
                            m.write_word(0x220, (-123i16) as u16);
                            m.write_word(0xa60, (-192i16) as u16);
                            let mut cpu = CpuCore::new();
                            cpu.dar[0] = seed;
                            cpu.dar[15] = sp;
                            cpu.pc = 0x6000;
                            cpu.set_ccr(flags);
                            let free = heap.free_bytes();
                            q.begin_text_observation()?;
                            if !te.dispatch(
                                0xa9ce,
                                &mut cpu,
                                &mut m,
                                &mut heap,
                                &q,
                                &fonts,
                                &events,
                                &mut callbacks,
                            )? || cpu.pc != 0x6002
                            {
                                return Err("TETextBox did not return".into());
                            }
                            let calls:Vec<_>=q.take_text_observations().ok_or("lost text observations")?.into_iter().map(|c|json!({"count":c.bytes.len(),"point":c.point,"font":c.font,"size":c.size,"face":c.face,"mode":c.mode,"clip":[c.clip.top,c.clip.left,c.clip.bottom,c.clip.right],"numer":[1,1],"denom":[1,1],"text_hex":c.bytes.iter().map(|b|format!("{b:02x}")).collect::<String>()})).collect();
                            if heap.free_bytes() != free {
                                return Err("text box leaked classic heap memory".into());
                            }
                            let cp = m.read_long(clip);
                            let cr = quickdraw::read_rect(&mut m, cp + 2);
                            records.push(json!({"which":which,"fontctx":fontctx,"height_case":heightcase,"box":[rect.top,rect.left,rect.bottom,rect.right],"justification":just,"input_d0":seed,"input_ccr":flags,"calls":calls,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+14),"guard":m.read_long(cpu.dar[15]),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"after_clip":[cr.top,cr.left,cr.bottom,cr.right],"before_pen_hex":before_pen,"after_pen_hex":hex(&mut m,port+48,18),"before_text_hex":before_text,"after_text_hex":hex(&mut m,port+68,20),"pixels_hex":hex(&mut m,0x16000,768)}));
                        }
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"text-box-runtime-raw-v3","records":records}).to_string())
}
