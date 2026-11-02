//! QuickDraw Random trap replay; no native observations are runtime inputs.
use crate::{machine::Memory, managers::Heap, quickdraw::QuickDraw};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const SEEDS: [u32; 13] = [
    0, 1, 2, 0x12345678, 0x7ffffffe, 0x7fffffff, 0x80000000, 0xffffffff, 0xffff0000, 0x0000ffff,
    0x00ff00ff, 0x55555555, 0xaaaaaaaa,
];
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let address = 0x100000 - 126;
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    for initial in SEEDS {
        for seed in [0x12345678u32, 0] {
            for flags in [16, 31, 0] {
                m.write_long(address, initial);
                for step in 0..16 {
                    let before = m.read_long(address);
                    cpu.pc = 0x6000;
                    cpu.dar[15] = 0x700000;
                    cpu.dar[0] = seed;
                    cpu.set_ccr(flags);
                    m.write_word(0x700000, 0x7b7b);
                    m.write_long(0x700002, 0x55667788);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    assert!(crate::colors::dispatch(
                        0xa861, &mut cpu, &mut m, &mut heap, &mut q
                    )?);
                    assert_eq!(cpu.pc, 0x6002);
                    records.push(json!({"initial_seed":initial,"input_d0":seed,"input_ccr":flags,"step":step,"seed_before":before,"seed_after":m.read_long(address),"result":m.read_word(0x700000)as i16,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0x700000,"guard":m.read_long(0x700002),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"qd-random-runtime-raw-v1","records":records}).to_string())
}
