//! Controlled public port state, independently replayed through production APIs.
use crate::{
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, read_rect, write_rect, QuickDraw},
    regions::{Operation, Region},
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn rect(r: Rect) -> [i16; 4] {
    [r.top, r.left, r.bottom, r.right]
}
fn words(m: &mut Memory, h: u32) -> Vec<u16> {
    let p = m.read_long(h);
    let n = m.read_word(p) / 2;
    (0..n).map(|i| m.read_word(p + 2 * i as u32)).collect()
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let mut fonts = crate::fonts::Fonts::new();
    fonts.init()?;
    let mut events = crate::events::Events::new();
    let mut windows = crate::windows::Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let mut cases = Vec::new();
    for kind in 0..4 {
        let port = if kind < 2 {
            let p = 0x110000;
            q.open_port(&mut m, &mut heap, p)?;
            if kind == 1 {
                q.make_color(&mut m, &mut heap, p)?;
            }
            p
        } else {
            let w = windows.create(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                &mut events,
                0,
                Rect {
                    top: 70,
                    left: 90,
                    bottom: 190,
                    right: 350,
                },
                b"Origin",
                true,
                0,
                u32::MAX,
                false,
                0,
            )?;
            if kind == 3 {
                windows.colorize(&mut m, &mut heap, &mut q, &fonts, w, None)?;
            }
            w
        };
        q.set_port(&mut m, port)?;
        write_rect(
            &mut m,
            port + 16,
            Rect {
                top: 0,
                left: 0,
                bottom: 120,
                right: 260,
            },
        );
        let bitmap = q.port_bitmap(&mut m, port)?;
        write_rect(
            &mut m,
            bitmap + 6,
            Rect {
                top: -70,
                left: -90,
                bottom: 410,
                right: 550,
            },
        );
        let a = Region::rectangle(Rect {
            top: 8,
            left: 11,
            bottom: 42,
            right: 53,
        });
        let b = Region::rectangle(Rect {
            top: 25,
            left: 30,
            bottom: 60,
            right: 75,
        });
        let vis = m.read_long(port + 24);
        a.combine(&b, Operation::Union)
            .store(&mut m, &mut heap, vis)?;
        let clip = m.read_long(port + 28);
        b.store(&mut m, &mut heap, clip)?;
        m.write_word(port + 48, (-19i16) as u16);
        m.write_word(port + 50, 37);
        m.write_word(port + 52, 3);
        m.write_word(port + 54, 2);
        for (step, (h, v)) in [(-17i16, 23i16), (31, -29), (31, -29), (0, 0)]
            .into_iter()
            .enumerate()
        {
            let flags = [16, 31, 0][step % 3];
            let mut cpu = CpuCore::new();
            cpu.pc = 0x6000;
            cpu.dar[0] = 0x12345678;
            cpu.dar[15] = 0x700000 - 4;
            cpu.set_ccr(flags);
            m.write_word(cpu.dar[15], v as u16);
            m.write_word(cpu.dar[15] + 2, h as u16);
            m.write_long(0x700000, 0x55667788);
            assert!(quickdraw::origin_dispatch(
                0xa878, &mut cpu, &mut m, &mut heap, &q
            )?);
            assert_eq!(cpu.pc, 0x6002);
            assert_eq!(m.read_long(0x700000), 0x55667788);
            let bounds = read_rect(&mut m, bitmap + 6);
            let mut converted = Vec::new();
            for (op, v, h) in [(0xa870, 13u16, 17u16), (0xa871, 130u16, 170u16)] {
                m.write_word(0x120000, v);
                m.write_word(0x120002, h);
                let mut conversion = CpuCore::new();
                conversion.pc = 0x6000;
                conversion.dar[15] = 0x700000 - 4;
                m.write_long(conversion.dar[15], 0x120000);
                assert!(quickdraw::origin_dispatch(
                    op,
                    &mut conversion,
                    &mut m,
                    &mut heap,
                    &q
                )?);
                assert_eq!(conversion.pc, 0x6002);
                assert_eq!(conversion.dar[15], 0x700000);
                assert_eq!(m.read_long(0x700000), 0x55667788);
                converted.push([m.read_word(0x120000) as i16, m.read_word(0x120002) as i16]);
            }
            cases.push(json!({"kind":kind,"step":step,"h":h,"v":v,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_ccr(),"stack_delta":0x700000i64-cpu.dar[15]as i64,"current_same":i32::from(q.get_port(&mut m)?==port),"pen":[m.read_word(port+48)as i16,m.read_word(port+50)as i16],"local_global":converted[0],"global_local":converted[1],"port_rect":rect(read_rect(&mut m,port+16)),"bitmap":rect(bounds),"visible":words(&mut m,vis),"clip":words(&mut m,clip)}));
        }
    }
    Ok(json!({"schema":1,"probe":"origin-v1","ports":{"cases":cases}}).to_string())
}
