//! Volume-query ABI with independently owned geometry/catalog/name facts.
use crate::{files::Files, machine::Memory};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const PB: u32 = 0x8004;
const NP: u32 = 0x9004;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
pub fn probe() -> Result<String, String> {
    let mut f = Files::new();
    let mut m = Memory::zeroed(0x10000);
    let mut cpu = CpuCore::new();
    m.write_long(0x4000 + 18, 0x5000);
    f.parameter_block(&mut m, 0xa014, 0x4000)
        .map_err(|e| e.to_string())?;
    let current_ref = m.read_word(0x4000 + 22) as i16;
    let n = u32::from(m.read_byte(0x5000)) + 1;
    let name: Vec<_> = (0..n).map(|i| m.read_byte(0x5000 + i)).collect();
    let current_name_hex = hex(&mut m, 0x5000, n);
    let mut records = Vec::new();
    for which in 0..8 {
        for seed in [0x12345678u32, 0] {
            for flags in [16, 31, 0] {
                m.write_long(PB - 4, 0xa1b2c3d4);
                m.write_long(PB + 128, 0x10203040);
                m.write_long(NP - 4, 0x11223344);
                m.write_long(NP + 256, 0x99aabbcc);
                for i in 0..128 {
                    m.write_byte(PB + i, 0xa5);
                }
                for i in 0..256 {
                    m.write_byte(NP + i, 0xa5);
                }
                let input: Option<&[u8]> = match which {
                    5 => Some(&name),
                    6 => Some(b"\x10NotAnOwnedVolume"),
                    _ => None,
                };
                if let Some(s) = input {
                    for (i, b) in s.iter().enumerate() {
                        m.write_byte(NP + i as u32, *b);
                    }
                }
                m.write_long(PB + 12, 0);
                m.write_long(PB + 18, if which == 7 { 0 } else { NP });
                m.write_word(
                    PB + 22,
                    match which {
                        1 => current_ref as u16,
                        4 => 32767,
                        _ => 0,
                    },
                );
                m.write_word(
                    PB + 28,
                    match which {
                        2 => 1,
                        3 => 2,
                        5 | 6 => 0xffff,
                        _ => 0,
                    },
                );
                m.write_word(PB + 16, 0x7b7b);
                let before = hex(&mut m, PB, 128);
                let name_before = hex(&mut m, NP, 256);
                m.write_word(0x220, (-123i16) as u16);
                m.write_word(0xa60, (-192i16) as u16);
                m.write_long(0xf000, 0x55667788);
                cpu.pc = 0x6000;
                cpu.dar[8] = PB;
                cpu.dar[15] = 0xf000;
                cpu.dar[0] = seed;
                cpu.set_ccr(flags);
                assert!(f.dispatch(0xa207, &mut cpu, &mut m)?);
                assert_eq!(cpu.pc, 0x6002);
                records.push(json!({"which":which,"input_d0":seed,"input_ccr":flags,"name_address":NP,"before_hex":before,"name_before_hex":name_before,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0xf000,"a0_preserved":i32::from(cpu.dar[8]==PB),"guard":m.read_long(0xf000),"pb_guards":[m.read_long(PB-4),m.read_long(PB+128)],"name_guards":[m.read_long(NP-4),m.read_long(NP+256)],"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"after_hex":hex(&mut m,PB,128),"name_after_hex":hex(&mut m,NP,256)}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"hgetvinfo-runtime-raw-v1","current_ref":current_ref,"current_name_hex":current_name_hex,"records":records}).to_string())
}
