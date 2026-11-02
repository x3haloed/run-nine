//! Owned low-memory requests replayed through the shared cursor-device service.
use crate::{events::Events, machine::Memory, managers::Heap, quickdraw::QuickDraw};
use classic_core::mouse::{self, Driver};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};

fn state(m: &mut Memory, q: &QuickDraw) -> Result<Value, String> {
    let temp = mouse::read(m, mouse::TEMP);
    let raw = mouse::read(m, mouse::RAW);
    let location = mouse::read(m, mouse::LOCATION);
    let before = m.read_word(mouse::NEW);
    let p = 0x18000;
    m.write_long(p, 0x11223344);
    m.write_long(p + 4, 0x55555555);
    m.write_long(p + 8, 0x55667788);
    let mut cpu = CpuCore::new();
    cpu.pc = 0x6000;
    cpu.dar[15] = 0xf000;
    m.write_long(0xf000, p + 4);
    m.write_long(0xf004, 0x12345678);
    assert!(Events::new().dispatch_state(0xa972, &mut cpu, m, q)?);
    assert_eq!(
        (cpu.pc, cpu.dar[15], m.read_long(0xf004)),
        (0x6002, 0xf004, 0x12345678)
    );
    Ok(json!({"mtemp":temp,"raw_mouse":raw,"mouse":location,
        "flags_before_get":before,"flags_after_get":m.read_word(mouse::NEW),
        "get_mouse":mouse::read(m,p+4),"prefix":m.read_long(p),"suffix":m.read_long(p+8)}))
}

pub fn probe() -> Result<String, String> {
    let mut records = Vec::new();
    for hide in 0..2 {
        for origin in 0..2 {
            for mask in 0..18 {
                for (which, target) in [(40, 60), (200, 210), (1, 638), (478, 1)]
                    .into_iter()
                    .enumerate()
                {
                    let mut m = Memory::zeroed(0x200000);
                    let mut heap = Heap::new(0x20000, 0xe0000);
                    let mut q = QuickDraw::new();
                    q.init(&mut m, 0x100000)?;
                    q.open_port(&mut m, &mut heap, 0x12000)?;
                    q.set_port(&mut m, 0x12000)?;
                    q.set_origin(
                        &mut m,
                        &mut heap,
                        if origin == 1 { 7 } else { 0 },
                        if origin == 1 { 11 } else { 0 },
                    )?;
                    classic_core::cursor::initialize(&mut m);
                    if hide == 1 {
                        classic_core::cursor::dispatch(0xa852, &mut CpuCore::new(), &mut m);
                    }
                    mouse::initialize(&mut m, 640, 480);
                    let mut driver = Driver::default();
                    mouse::publish(&mut m, (200, 210));
                    m.write_word(mouse::NEW, 65535);
                    driver.service(&mut m);
                    let baseline = state(&mut m, &q)?;
                    if mask == 1
                        || mask == 4
                        || (5..=10).contains(&mask)
                        || [13, 15, 16].contains(&mask)
                    {
                        mouse::write(&mut m, mouse::TEMP, target);
                    }
                    if mask == 2
                        || mask == 4
                        || (5..=9).contains(&mask)
                        || [11, 14, 17].contains(&mask)
                    {
                        mouse::write(&mut m, mouse::RAW, target);
                    }
                    if [3, 5, 12].contains(&mask) {
                        mouse::write(&mut m, mouse::LOCATION, target);
                    }
                    if [13, 15].contains(&mask) {
                        mouse::write(&mut m, mouse::RAW, (100, 110));
                    }
                    if mask == 14 {
                        mouse::write(&mut m, mouse::TEMP, (100, 110));
                    }
                    match mask {
                        6 | 10..=14 => m.write_word(mouse::NEW, 65535),
                        7 => m.write_word(mouse::NEW, 0),
                        8 | 15..=17 => m.write_word(mouse::NEW, 65280),
                        9 => m.write_word(mouse::NEW, 255),
                        _ => (),
                    }
                    let immediate = state(&mut m, &q)?;
                    driver.service(&mut m);
                    let settled = state(&mut m, &q)?;
                    driver.service(&mut m);
                    let after_system_task = state(&mut m, &q)?;
                    m.write_word(mouse::NEW, 65280);
                    driver.service(&mut m);
                    let second_uncoupled = state(&mut m, &q)?;
                    m.write_word(mouse::NEW, 65535);
                    driver.service(&mut m);
                    let recoupled = state(&mut m, &q)?;
                    records.push(json!({"hide":hide,"origin":origin,"mask":mask,"which":which,
                        "requested":target,"baseline":baseline,"immediate":immediate,"settled":settled,
                        "after_system_task":after_system_task,"second_uncoupled":second_uncoupled,"recoupled":recoupled}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"mouse-coupling-runtime-raw-v1","records":records}).to_string())
}
