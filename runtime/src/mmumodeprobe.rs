//! Legal MMU mode requests against the declared flat32-bit environment.
use crate::{environment::Environment, machine::Memory};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x10000);
    let env = Environment::runtime();
    env.initialize(&mut m);
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    for requested in [1u32, 0] {
        for upper in [0, 0x12345600, 0xffff0000] {
            for flags in [16, 31, 0] {
                let before = m.read_byte(0xcb2);
                cpu.pc = 0x6000;
                cpu.dar[15] = 0xf000;
                cpu.dar[8] = 0x8000;
                cpu.dar[0] = upper | requested;
                cpu.set_ccr(flags);
                m.write_long(0xf000, 0x55667788);
                m.write_word(0x220, (-123i16) as u16);
                m.write_word(0xa60, (-192i16) as u16);
                assert!(env.dispatch(0xa05d, &mut cpu, &mut m));
                assert_eq!(cpu.pc, 0x6002);
                let result = cpu.dar[0];
                let ccr = cpu.get_sr() & 31;
                let mode = m.read_byte(0xcb2);
                let same = i32::from(cpu.dar[8] == 0x8000);
                let delta = i64::from(cpu.dar[15]) - 0xf000;
                cpu.dar[0] = 1;
                assert!(env.dispatch(0xa05d, &mut cpu, &mut m));
                records.push(json!({"requested":requested,"input_d0":upper|requested,"input_ccr":flags,"mode_before":before,"mode_after":mode,"mode_restored":m.read_byte(0xcb2),"d0":result,"ccr":ccr,"stack_delta":delta,"a0_preserved":same,"guard":m.read_long(0xf000),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"mmu-mode-runtime-raw-v1","records":records}).to_string())
}
