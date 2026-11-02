//! Physical-button and OS event-queue replay through production event traps.
use crate::{
    events::Events,
    machine::Memory,
    quickdraw::QuickDraw,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const INPUTS: [&[u16]; 8] = [&[], &[3], &[2], &[1], &[3, 2], &[2, 3], &[1, 3], &[1, 2, 3]];
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x10000);
    let q = QuickDraw::new();
    let mut events = Events::new();
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    for phase in 0..2 {
        for op in 0..2 {
            for (queue, inputs) in INPUTS.iter().enumerate() {
                for seed in [0x12345678u32, 0] {
                    for flags in [16, 31, 0] {
                        events.flush(u16::MAX, 0);
                        events.modifiers = if phase == 0 { 0x80 } else { 0 };
                        let mut errors = Vec::new();
                        for (i, what) in inputs.iter().enumerate() {
                            cpu.pc = 0x5000;
                            cpu.dar[8] = u32::from(*what);
                            cpu.dar[0] = 0xa000 + i as u32;
                            assert!(events.dispatch_post(0xa02f, &mut cpu));
                            assert_eq!(cpu.pc, 0x5002);
                            errors.push(cpu.dar[0] as i16);
                        }
                        let before = i32::from(events.modifiers & 0x80 == 0);
                        m.write_word(0x220, (-123i16) as u16);
                        m.write_word(0xa60, (-192i16) as u16);
                        cpu.pc = 0x6000;
                        cpu.dar[15] = 0xf000;
                        cpu.dar[0] = seed;
                        cpu.set_ccr(flags);
                        m.write_word(0xf000, 0x7b7b);
                        m.write_long(0xf002, 0x55667788);
                        assert!(events.dispatch_state(
                            if op == 0 { 0xa973 } else { 0xa974 },
                            &mut cpu,
                            &mut m,
                            &q
                        )?);
                        assert_eq!(cpu.pc, 0x6002);
                        let mut remaining = Vec::new();
                        loop {
                            let (found, event) = events.next(u16::MAX, true);
                            if !found {
                                break;
                            }
                            remaining.push(event.what);
                        }
                        records.push(json!({"phase":phase,"op":op,"queue":queue,"input_d0":seed,"input_ccr":flags,"button_before":before,"button_after":i32::from(events.modifiers&0x80==0),"result_word":m.read_word(0xf000),"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0xf000,"guard":m.read_long(0xf002),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"post_errors":errors,"remaining_types":remaining}));
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"still-down-runtime-raw-v1","records":records}).to_string())
}
