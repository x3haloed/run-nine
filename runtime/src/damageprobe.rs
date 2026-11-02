//! Actual window damage/event/update paths with independently constructed inputs.
use crate::{
    events::Events,
    fonts::Fonts,
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{read_rect, write_rect, QuickDraw},
    regions::{Operation, Region},
    windows::Windows,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn rect(r: Rect) -> [i16; 4] {
    [r.top, r.left, r.bottom, r.right]
}
fn region_hex(m: &mut Memory, h: u32) -> String {
    let p = m.read_long(h);
    let n = m.read_word(p) as u32;
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
fn event(w: u32, pair: (bool, crate::events::Event)) -> [i32; 4] {
    let (hit, e) = pair;
    [
        hit as i32,
        e.what as i32,
        (e.message == w) as i32,
        (e.message == 0) as i32,
    ]
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    q.color_capable = true;
    q.device(&mut m, &mut heap)?;
    q.set_depth(&mut m, &mut heap, 8)
        .map_err(|e| e.to_string())?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(&mut m, &mut heap, &q)?;
    let w = windows.create(
        &mut m,
        &mut heap,
        &q,
        &fonts,
        &mut events,
        0,
        Rect {
            top: 80,
            left: 70,
            bottom: 200,
            right: 250,
        },
        b"Damage",
        true,
        0,
        u32::MAX,
        true,
        0,
    )?;
    windows.colorize(&mut m, &mut heap, &mut q, &fonts, w, None)?;
    q.set_port(&mut m, w)?;
    windows.next_event(&mut m, &mut events, 1 << 8, true)?;
    windows.next_event(&mut m, &mut events, 1 << 8, true)?;
    windows.begin_update(&mut m, &mut heap, &q, w)?;
    windows.end_update(&mut m, &mut heap, w)?;
    let source = heap
        .new_handle(&mut m, 10, true)
        .map_err(|e| e.to_string())?;
    let input = 0x110000;
    let mut rows = Vec::new();
    let shapes = [
        Rect {
            top: 10,
            left: 20,
            bottom: 30,
            right: 60,
        },
        Rect {
            top: -30,
            left: -50,
            bottom: 180,
            right: 250,
        },
        Rect {
            top: 5,
            left: 5,
            bottom: 5,
            right: 5,
        },
        Rect {
            top: 300,
            left: 400,
            bottom: 320,
            right: 420,
        },
    ];
    for op in [0xa92au16, 0xa929, 0xa928, 0xa927] {
        for origin in 0..2 {
            for shape in 0..4 {
                for flags in [16u8, 31, 0] {
                    q.set_port(&mut m, w)?;
                    q.set_origin(
                        &mut m,
                        &mut heap,
                        if origin == 1 { 31 } else { 0 },
                        if origin == 1 { -7 } else { 0 },
                    )?;
                    let clip = m.read_long(w + 28);
                    Region::rectangle(Rect {
                        top: 10,
                        left: 10,
                        bottom: 60,
                        right: 80,
                    })
                    .store(&mut m, &mut heap, clip)?;
                    let whole = read_rect(&mut m, w + 16);
                    windows.invalidate(&mut m, &mut heap, &q, &mut events, whole)?;
                    let input_rect = shapes[shape];
                    write_rect(&mut m, input, input_rect);
                    let mut mask = Region::rectangle(input_rect);
                    if shape == 0 && matches!(op, 0xa929 | 0xa927) {
                        mask = Region::rectangle(Rect {
                            top: 4,
                            left: 8,
                            bottom: 50,
                            right: 100,
                        })
                        .combine(
                            &Region::rectangle(Rect {
                                top: 12,
                                left: 20,
                                bottom: 36,
                                right: 80,
                            }),
                            Operation::Difference,
                        );
                    }
                    mask.store(&mut m, &mut heap, source)?;
                    let arg = if matches!(op, 0xa92a | 0xa928) {
                        input
                    } else {
                        source
                    };
                    let update = m.read_long(w + 122);
                    let vis = m.read_long(w + 24);
                    let bitmap = q.port_bitmap(&mut m, w)?;
                    let mut r = json!({"trap":op,"origin_case":origin,"shape":shape,"input_ccr":flags,"input_rect":rect(input_rect),"port_rect":rect(read_rect(&mut m,w+16)),"bitmap_bounds":rect(read_rect(&mut m,bitmap+6)),"input_region":region_hex(&mut m,source),"before_update":region_hex(&mut m,update),"before_vis":region_hex(&mut m,vis),"before_clip":region_hex(&mut m,clip)});
                    let mut cpu = CpuCore::new();
                    cpu.pc = 0x6000;
                    cpu.dar[15] = 0x700000 - 4;
                    cpu.dar[0] = 0x12345678;
                    cpu.set_ccr(flags);
                    m.write_long(cpu.dar[15], arg);
                    assert!(windows.damage_dispatch(
                        op,
                        &mut cpu,
                        &mut m,
                        &mut heap,
                        &q,
                        &mut events
                    )?);
                    assert_eq!(cpu.pc, 0x6002);
                    let up = m.read_long(update);
                    let d0 = cpu.dar[0];
                    let current = q.get_port(&mut m)?;
                    for (k, v) in [
                        ("d0", json!(d0)),
                        ("d0_is_arg", json!((d0 == arg) as i32)),
                        ("d0_is_window", json!((d0 == w) as i32)),
                        ("d0_is_update_handle", json!((d0 == update) as i32)),
                        ("d0_is_update_data", json!((d0 == up) as i32)),
                        ("ccr", json!(cpu.get_sr() & 31)),
                        ("stack_delta", json!(0x700000i64 - cpu.dar[15] as i64)),
                        ("current_same", json!((current == w) as i32)),
                        ("after_update", json!(region_hex(&mut m, update))),
                        ("after_vis", json!(region_hex(&mut m, vis))),
                        ("after_clip", json!(region_hex(&mut m, clip))),
                        ("after_input_region", json!(region_hex(&mut m, source))),
                        ("after_input_rect", json!(rect(read_rect(&mut m, input)))),
                        ("after_port_rect", json!(rect(read_rect(&mut m, w + 16)))),
                        (
                            "after_bitmap_bounds",
                            json!(rect(read_rect(&mut m, bitmap + 6))),
                        ),
                    ] {
                        r[k] = v;
                    }
                    r["event1"] = json!(event(
                        w,
                        windows.next_event(&mut m, &mut events, 1 << 6, true)?
                    ));
                    r["event2"] = json!(event(
                        w,
                        windows.next_event(&mut m, &mut events, 1 << 6, true)?
                    ));
                    r["after_events_update"] = json!(region_hex(&mut m, update));
                    windows.begin_update(&mut m, &mut heap, &q, w)?;
                    r["during_update"] = json!(region_hex(&mut m, update));
                    r["during_vis"] = json!(region_hex(&mut m, vis));
                    r["during_clip"] = json!(region_hex(&mut m, clip));
                    windows.end_update(&mut m, &mut heap, w)?;
                    r["final_update"] = json!(region_hex(&mut m, update));
                    r["final_vis"] = json!(region_hex(&mut m, vis));
                    r["final_clip"] = json!(region_hex(&mut m, clip));
                    rows.push(r);
                }
            }
        }
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"window-damage-raw-v2","records":rows}),
    )
    .unwrap())
}
