//! Shared Device Manager lookup exercised through the production CPU entry.
use crate::{machine::Memory, managers::Heap};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    classic_core::drivers::initialize(&mut m, &mut heap, classic_core::drivers::PROFILE_UNIT_COUNT)
        .map_err(|e| e.to_string())?;
    let count = m.read_word(0x1d2);
    let table = m.read_long(0x11c);
    let pb = 0x1000;
    let mut rows = Vec::new();
    for op in [0xa004u16, 0xa204, 0xa005, 0xa205] {
        for reference in [-4i16, -32768, 0, 1] {
            for code in [2i16, -1] {
                for seed in [0x12345678u32, 0] {
                    for flags in [16u8, 31, 0] {
                        m.write_long(pb - 4, 0x11223344);
                        m.write_long(pb + 50, 0x55667788);
                        for i in 0..50 {
                            m.write_byte(pb + i, 0xa5);
                        }
                        m.write_long(pb, 0);
                        m.write_word(pb + 4, 0);
                        m.write_word(pb + 6, 0x5a5a);
                        m.write_long(pb + 8, 0);
                        m.write_long(pb + 12, 0);
                        m.write_word(pb + 16, 0x1234);
                        m.write_long(pb + 18, 0);
                        m.write_word(pb + 22, 0);
                        m.write_word(pb + 24, reference as u16);
                        m.write_word(pb + 26, code as u16);
                        m.write_word(pb + 28, 3);
                        let index = !(reference as u16);
                        let in_range = index < count;
                        let empty = if in_range {
                            (m.read_long(table + u32::from(index) * 4) == 0) as i16
                        } else {
                            -1
                        };
                        let mut cpu = CpuCore::new();
                        cpu.pc = 0x6000;
                        cpu.dar[8] = pb;
                        cpu.dar[15] = 0x700000;
                        cpu.dar[0] = seed;
                        cpu.set_ccr(flags);
                        assert!(classic_core::drivers::dispatch(op, &mut cpu, &mut m)
                            .map_err(|e| format!("{e:?}"))?);
                        assert_eq!(cpu.pc, 0x6002);
                        let bytes: String = (0..50)
                            .map(|i| format!("{:02x}", m.read_byte(pb + i)))
                            .collect();
                        rows.push(json!({"trap":op,"reference":reference,"code":code,"input_d0":seed,"input_ccr":flags,"unit_index":index,"in_range":in_range as i32,"entry_empty":empty,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":cpu.dar[15]as i64-0x700000,"a0_preserved":(cpu.dar[8]==pb)as i32,"before_guard":m.read_long(pb-4),"after_guard":m.read_long(pb+50),"pb_hex":bytes}));
                    }
                }
            }
        }
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"empty-driver-raw-v1","unit_count":count,"records":rows}),
    )
    .unwrap())
}
