//! Independent replay through the production color services. No OS 9 capture
//! or expected observation is read here.
use crate::{
    callbacks::Arg,
    managers::{Heap, Rect},
    quickdraw::{write_rect, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
struct Memory(Vec<u8>);
impl AddressBus for Memory {
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
struct Scene {
    m: Memory,
    heap: Heap,
    q: QuickDraw,
    cpu: CpuCore,
}
impl Scene {
    fn call(
        &mut self,
        op: u16,
        selector: u16,
        args: &[Arg],
        result: u32,
        ccr: u16,
    ) -> Result<(i16, u16, u32, u32), String> {
        let mut sp = 0x700000;
        self.m.write_long(sp, 0x55667788);
        sp -= result;
        let end = sp;
        if result == 2 {
            self.m.write_word(sp, 0x7b7b);
        } else if result == 4 {
            self.m.write_long(sp, 0x7b7b7b7b);
        }
        for arg in args {
            match arg {
                Arg::Word(v) => {
                    sp -= 2;
                    self.m.write_word(sp, *v);
                }
                Arg::Long(v) => {
                    sp -= 4;
                    self.m.write_long(sp, *v);
                }
            }
        }
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = sp;
        self.cpu.dar[0] = selector as u32;
        self.cpu.set_ccr(ccr as u8);
        if !crate::colors::dispatch(op, &mut self.cpu, &mut self.m, &mut self.heap, &mut self.q)?
            || self.cpu.pc != 0x6002
            || self.cpu.dar[15] != end
        {
            return Err("color trap stack/PC ABI failure".into());
        }
        Ok((
            self.m.read_word(end) as i16,
            self.cpu.get_sr() & 31,
            end - sp,
            self.m.read_long(0x700000),
        ))
    }
    fn color(&mut self, index: u32, foreground: bool) -> Result<(), String> {
        self.call(
            if foreground { 0xaa14 } else { 0xaa15 },
            0,
            &[Arg::Long(0x1a002 + index * 8)],
            0,
            16,
        )?;
        Ok(())
    }
    fn sample(&mut self, x: u32, y: u32) -> u8 {
        self.m
            .read_byte(self.q.screen_base + y * self.q.row_bytes() as u32 + x)
    }
    fn copy(&mut self, bitmap: u32, dr: u32) -> Result<(), String> {
        self.call(
            0xa8ec,
            0,
            &[
                Arg::Long(0x1a300),
                Arg::Long(bitmap),
                Arg::Long(0x1a100),
                Arg::Long(dr),
                Arg::Word(0),
                Arg::Long(0),
            ],
            0,
            16,
        )?;
        Ok(())
    }
}
pub fn probe() -> Result<String, String> {
    let mut s = Scene {
        m: Memory(vec![0; 0x800000]),
        heap: Heap::new(0x20000, 0x500000),
        q: QuickDraw::new(),
        cpu: CpuCore::new(),
    };
    s.q.color_capable = true;
    s.q.init(&mut s.m, 0x100000)?;
    let device = s.q.device(&mut s.m, &mut s.heap)?;
    let mut abi = Vec::new();
    for selector in [0x0a14, 0x0a13] {
        for depth in [8, 3] {
            for ccr in [16, 31, 0] {
                let (result, flags, delta, guard) = s.call(
                    0xaaa2,
                    selector,
                    &[
                        Arg::Long(device),
                        Arg::Word(depth),
                        Arg::Word(0),
                        Arg::Word(0),
                    ],
                    2,
                    ccr,
                )?;
                abi.push(json!({"selector":selector,"depth":depth,"input_ccr":ccr,"result":result,"ccr":flags,"stack_delta":delta,"guard":guard}));
            }
        }
    }
    s.q.set_depth(&mut s.m, &mut s.heap, 8)
        .map_err(|e| e.to_string())?;
    let dp = s.m.read_long(device);
    let bitmap = s.q.device_bitmap(&mut s.m)?;
    let r = crate::quickdraw::read_rect(&mut s.m, bitmap + 6);
    let d = crate::quickdraw::read_rect(&mut s.m, dp + 34);
    let th = s.m.read_long(bitmap + 42);
    let table = s.m.read_long(th);
    let state = json!({"flag_roles":s.m.read_word(dp+20)&0xa821,"type":s.m.read_word(dp+4),"depth":s.m.read_word(bitmap+32),"pixel_type":s.m.read_word(bitmap+30),"components":s.m.read_word(bitmap+34),"component_size":s.m.read_word(bitmap+36),"row_bytes":s.m.read_word(bitmap+4),"bounds":[r.top,r.left,r.bottom,r.right],"device_bounds":[d.top,d.left,d.bottom,d.right],"table_size":s.m.read_word(table+6),"table_flags":s.m.read_word(table+4)});
    let port = 0x1c000;
    s.q.open_port(&mut s.m, &mut s.heap, port)?;
    s.q.make_color(&mut s.m, &mut s.heap, port)?;
    let default_palette = s.q.palette(&mut s.m, bitmap)?;
    for i in 0..4u32 {
        let p = 0x1a000 + i * 8;
        s.m.write_word(p, (16 + i) as u16);
        s.m.write_word(p + 2, (0x1234 + i * 0x2345) as u16);
        s.m.write_word(p + 4, (0x9876 - i * 0x1234) as u16);
        s.m.write_word(p + 6, (0x5678 + i * 0x1111) as u16);
    }
    s.call(
        0xaa3f,
        0,
        &[Arg::Word(16), Arg::Word(3), Arg::Long(0x1a000)],
        0,
        16,
    )?;
    let custom_entries = s.q.palette(&mut s.m, bitmap)?[16..20].to_vec();
    let mut indices = Vec::new();
    for i in 0..4u32 {
        s.call(0xaa33, 0, &[Arg::Long(0x1a002 + i * 8)], 4, 16)?;
        indices.push(s.m.read_long(0x6ffffc));
    }
    let rect = Rect {
        top: 40,
        left: 20,
        bottom: 42,
        right: 28,
    };
    let mut paints = Vec::new();
    for kind in 0..3 {
        s.color(0, true)?;
        s.q.paint_rect(&mut s.m, rect, [255; 8], 8)?;
        if kind > 0 {
            s.color(1, true)?;
            s.q.paint_rect(&mut s.m, rect, [255; 8], if kind == 1 { 10 } else { 11 })?;
        }
        paints.push((20..28).map(|x| s.sample(x, 40)).collect::<Vec<_>>());
    }
    s.call(
        0xaa16,
        0,
        &[Arg::Word(22), Arg::Word(40), Arg::Long(0x1a012)],
        0,
        16,
    )?;
    s.call(
        0xaa17,
        0,
        &[Arg::Word(22), Arg::Word(40), Arg::Long(0x1a080)],
        0,
        16,
    )?;
    let get_rgb = |s: &mut Scene| {
        [
            s.m.read_word(0x1a080),
            s.m.read_word(0x1a082),
            s.m.read_word(0x1a084),
        ]
    };
    let set_pixel = json!({"index":s.sample(22,40),"rgb":get_rgb(&mut s)});
    s.m.write_long(0x1a1fc, 0x11223344);
    for (i, v) in [16, 17, 0xaa, 0xbb, 18, 19, 0xcc, 0xdd]
        .into_iter()
        .enumerate()
    {
        s.m.write_byte(0x1a200 + i as u32, v);
    }
    s.m.write_long(0x1a208, 0x55667788);
    for i in 0..50 {
        let b = s.m.read_byte(bitmap + i);
        s.m.write_byte(0x1a300 + i, b);
    }
    s.m.write_long(0x1a300, 0x1a200);
    s.m.write_word(0x1a304, 0x8004);
    write_rect(
        &mut s.m,
        0x1a306,
        Rect {
            top: 0,
            left: 0,
            bottom: 2,
            right: 2,
        },
    );
    write_rect(
        &mut s.m,
        0x1a100,
        Rect {
            top: 0,
            left: 0,
            bottom: 2,
            right: 2,
        },
    );
    write_rect(
        &mut s.m,
        0x1a110,
        Rect {
            top: 44,
            left: 20,
            bottom: 46,
            right: 22,
        },
    );
    s.copy(bitmap, 0x1a110)?;
    let copy_pixels = [
        s.sample(20, 44),
        s.sample(21, 44),
        s.sample(20, 45),
        s.sample(21, 45),
    ];
    let copy_source = [
        s.m.read_long(0x1a1fc),
        s.m.read_byte(0x1a202) as u32,
        s.m.read_byte(0x1a203) as u32,
        s.m.read_byte(0x1a206) as u32,
        s.m.read_byte(0x1a207) as u32,
        s.m.read_long(0x1a208),
    ];
    for i in 0..3 {
        s.m.write_word(0x1a060 + i * 2, 0);
        s.m.write_word(0x1a070 + i * 2, 65535);
    }
    s.call(0xaa14, 0, &[Arg::Long(0x1a060)], 0, 16)?;
    s.call(0xaa15, 0, &[Arg::Long(0x1a070)], 0, 16)?;
    write_rect(
        &mut s.m,
        0x1a110,
        Rect {
            top: 48,
            left: 20,
            bottom: 50,
            right: 22,
        },
    );
    s.copy(bitmap, 0x1a110)?;
    let neutral = [
        s.sample(20, 48),
        s.sample(21, 48),
        s.sample(20, 49),
        s.sample(21, 49),
    ];
    let mut rops = Vec::new();
    for mode in 0..8 {
        for bit in [false, true] {
            s.color(0, true)?;
            s.q.paint_rect(&mut s.m, rect, [255; 8], 8)?;
            s.color(1, true)?;
            s.color(2, false)?;
            s.q.paint_rect(&mut s.m, rect, [if bit { 255 } else { 0 }; 8], 8 + mode)?;
            rops.push(s.sample(20, 40));
        }
    }
    for (i, v) in [0xaaaa, 0xbbbb, 0xcccc].into_iter().enumerate() {
        s.m.write_word(0x1a002 + i as u32 * 2, v);
    }
    s.call(
        0xaa3f,
        0,
        &[Arg::Word(16), Arg::Word(0), Arg::Long(0x1a000)],
        0,
        16,
    )?;
    s.call(
        0xaa17,
        0,
        &[Arg::Word(20), Arg::Word(48), Arg::Long(0x1a080)],
        0,
        16,
    )?;
    let recolored = json!({"index":s.sample(20,48),"rgb":get_rgb(&mut s)});
    let indexed: Value = json!({"available":1,"default_palette":default_palette,"custom_entries":custom_entries,"exact_indices":indices,"paints":paints,"set_pixel":set_pixel,"copy_pixels":copy_pixels,"copy_source":copy_source,"neutral_copy_pixels":neutral,"rop_modes":rops,"recolored":recolored});
    #[cfg(feature = "gui")]
    if std::env::args().any(|a| a == "--gui") {
        view(&mut s)?;
    }
    Ok(json!({"schema":1,"probe":"indexed-color-v1","color":{"abi":abi,"device8":state,"indexed":indexed}}).to_string())
}

#[cfg(feature = "gui")]
fn view(s: &mut Scene) -> Result<(), String> {
    // A controlled visual fixture for the existing presentation/input path.
    // Its bars are diagnostic owned output, not an original application UI.
    for i in 0..4 {
        s.color(i, true)?;
        s.q.paint_rect(
            &mut s.m,
            Rect {
                top: 80,
                left: 40 + i as i16 * 140,
                bottom: 400,
                right: 180 + i as i16 * 140,
            },
            [255; 8],
            8,
        )?;
    }
    let mut frontend = crate::frontend::Frontend::new(
        &s.q,
        Some("work/validation/indexed-probe-presentation.jsonl"),
    )?;
    let mut events = crate::events::Events::new();
    let mut input = std::collections::VecDeque::new();
    let mut changed = false;
    loop {
        if !frontend.poll(
            &mut s.m,
            &s.q,
            &mut events,
            &mut input,
            crate::frontend::PresentationContext {
                pc: 0,
                instruction: 0,
                update_depth: 0,
                audio_frame: &|| None,
            },
        )? {
            break;
        }
        while let Some(event) = input.pop_front() {
            if event.what == 3 {
                if event.message as u8 == 13 {
                    return Ok(());
                }
                if event.message as u8 == 32 {
                    changed = !changed;
                    for (i, value) in (if changed {
                        [65535, 0, 0]
                    } else {
                        [0xaaaa, 0xbbbb, 0xcccc]
                    })
                    .into_iter()
                    .enumerate()
                    {
                        s.m.write_word(0x1a002 + i as u32 * 2, value);
                    }
                    let before = s.sample(50, 100);
                    s.call(
                        0xaa3f,
                        0,
                        &[Arg::Word(16), Arg::Word(0), Arg::Long(0x1a000)],
                        0,
                        16,
                    )?;
                    let after = s.sample(50, 100);
                    let rgb = s.q.screen_colors(&mut s.m)?[after as usize];
                    eprintln!("indexed fixture: actual Space key; palette_changed={changed}; index_before={before}; index_after={after}; rgb={rgb:#08x}");
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    Ok(())
}
