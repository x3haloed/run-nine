//! Real OS event retrieval with independently owned queue/device inputs.
use crate::{
    events::{write_event, Event, Events},
    machine::Memory,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const INPUTS: [&[u16]; 8] = [&[], &[3], &[2], &[1], &[3, 2], &[2, 3], &[1, 3], &[1, 2, 3]];
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
fn queue(m: &mut Memory, events: &Events) -> Vec<String> {
    events
        .snapshot()
        .into_iter()
        .map(|e| {
            write_event(m, 0x9000, e);
            hex(m, 0x9000, 16)
        })
        .collect()
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x10000);
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    for (which, inputs) in INPUTS.iter().enumerate() {
        for mask in [0u16, 65535, 8, 6, 256] {
            for op in 0..2 {
                for upper in [0, 0x12340000u32] {
                    for flags in [16, 31, 0] {
                        let mut events = Events::new();
                        events.ticks = 0x40000000 + records.len() as u32;
                        events.set_mouse(&mut m, (15, 15));
                        m.write_long(0x16a, events.ticks);
                        m.write_byte(0x172, 0x80);
                        Events::publish_keys(&mut m, [0; 16]);
                        for p in [0x828, 0x82c, 0x830] {
                            m.write_word(p, 15);
                            m.write_word(p + 2, 15);
                        }
                        let physical =
                            format!("{}8000{}", hex(&mut m, 0x174, 16), hex(&mut m, 0x828, 12));
                        let errors: Vec<i16> = inputs
                            .iter()
                            .enumerate()
                            .map(|(j, &what)| {
                                events
                                    .post(Event {
                                        what,
                                        message: 0x12340000 + j as u32,
                                        when: 0x10203000 + j as u32,
                                        v: -123 - j as i16,
                                        h: 234 + j as i16,
                                        modifiers: 0x1a80 + j as u16,
                                    })
                                    .err()
                                    .unwrap_or(0)
                            })
                            .collect();
                        let before = queue(&mut m, &events);
                        m.write_long(0x8000, 0xa1b2c3d4);
                        for i in 0..16 {
                            m.write_byte(0x8004 + i, 0xa5);
                        }
                        m.write_long(0x8014, 0x10203040);
                        cpu.pc = 0x6000;
                        cpu.dar[15] = 0xf000;
                        cpu.dar[8] = 0x8004;
                        cpu.dar[0] = upper | u32::from(mask);
                        cpu.set_ccr(flags);
                        m.write_long(0xf000, 0x55667788);
                        m.write_word(0x220, (-123i16) as u16);
                        m.write_word(0xa60, (-192i16) as u16);
                        assert!(events.dispatch_os_event(
                            if op == 0 { 0xa030 } else { 0xa031 },
                            &mut cpu,
                            &mut m
                        ));
                        assert_eq!(cpu.pc, 0x6002);
                        let after = queue(&mut m, &events);
                        records.push(json!({"queue":which,"mask":mask,"op":op,"input_d0":upper|u32::from(mask),"input_ccr":flags,"post_errors":errors,"queue_before":before,"queue_after":after,"ticks_before":events.ticks,"ticks_after":events.ticks,"physical_hex":physical,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0xf000,"a0_preserved":i32::from(cpu.dar[8]==0x8004),"guard":m.read_long(0xf000),"prefix":m.read_long(0x8000),"suffix":m.read_long(0x8014),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"event_hex":hex(&mut m,0x8004,16)}));
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"os-events-runtime-raw-v2","records":records}).to_string())
}
