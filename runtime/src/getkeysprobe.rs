//! Owned physical keyboard snapshots through the production GetKeys trap.
use crate::{
    events::{Events, KEY_MAP},
    machine::Memory,
    quickdraw::QuickDraw,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, address: u32) -> String {
    (0..16)
        .map(|i| format!("{:02x}", m.read_byte(address + i)))
        .collect()
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x10000);
    let events = Events::new();
    let q = QuickDraw::new();
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    let states: [&[u8]; 6] = [&[], &[126], &[13], &[49], &[56, 126], &[]];
    for (phase, state) in states.into_iter().enumerate() {
        let mut keys = [0u8; 16];
        for code in state {
            keys[usize::from(code / 8)] |= 1 << (code % 8);
        }
        Events::publish_keys(&mut m, keys);
        for seed in [0x12345678u32, 0] {
            for flags in [16, 31, 0] {
                m.write_long(0x8000, 0xa1b2c3d4);
                for i in 0..16 {
                    m.write_byte(0x8004 + i, 0x7b);
                }
                m.write_long(0x8014, 0x10203040);
                cpu.pc = 0x6000;
                cpu.dar[15] = 0xf000;
                cpu.dar[0] = seed;
                cpu.set_ccr(flags);
                m.write_long(0xf000, 0x8004);
                m.write_long(0xf004, 0x55667788);
                let before = hex(&mut m, KEY_MAP);
                m.write_word(0x220, (-123i16) as u16);
                m.write_word(0xa60, (-192i16) as u16);
                assert!(events.dispatch_state(0xa976, &mut cpu, &mut m, &q)?);
                assert_eq!(cpu.pc, 0x6002);
                records.push(json!({"phase":phase,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0xf004,"guard":m.read_long(0xf004),"prefix":m.read_long(0x8000),"suffix":m.read_long(0x8014),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"before_hex":before,"after_hex":hex(&mut m,KEY_MAP),"keys_hex":hex(&mut m,0x8004)}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"get-keys-runtime-raw-v1","records":records}).to_string())
}
