//! Owned handle/buffer inputs exercise the production GetDialogItemText ABI.
use crate::{dialogs, machine::Memory, managers::Heap};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
pub fn probe() -> Result<String, String> {
    let mut records = Vec::new();
    for (which, length) in [0u32, 1, 8, 64, 127, 255, 256, 300].into_iter().enumerate() {
        for state in 0..2 {
            for seed in [0x12345678u32, 0] {
                for flags in [16u8, 31, 0] {
                    let mut m = Memory::zeroed(0x800000);
                    let mut heap = Heap::new(0x20000, 0xe0000);
                    let h = heap
                        .new_handle(&mut m, length, false)
                        .map_err(|e| e.to_string())?;
                    let data = heap.data(h).map_err(|e| e.to_string())?;
                    for i in 0..length {
                        m.write_byte(data + i, (i * 37 + 11) as u8);
                    }
                    heap.lock(h, state == 1).map_err(|e| e.to_string())?;
                    let before = heap.state(h).map_err(|e| e.to_string())?;
                    let ptr = 0x18004;
                    let sp = 0x700000;
                    m.write_long(ptr - 4, 0xa1b2c3d4);
                    m.write_long(ptr + 600, 0x10203040);
                    for i in 0..600 {
                        m.write_byte(ptr + i, 0xa5);
                    }
                    m.write_long(sp, ptr);
                    m.write_long(sp + 4, h);
                    m.write_long(sp + 8, 0x55667788);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    let mut cpu = CpuCore::new();
                    cpu.dar[0] = seed;
                    cpu.dar[15] = sp;
                    cpu.pc = 0x6000;
                    cpu.set_ccr(flags);
                    if !dialogs::item_text_dispatch(0xa990, &mut cpu, &mut m, &mut heap)?
                        || cpu.pc != 0x6002
                    {
                        return Err("GetDialogItemText did not return".into());
                    }
                    let after = heap.state(h).map_err(|e| e.to_string())?;
                    let bytes: String = (0..600)
                        .map(|i| format!("{:02x}", m.read_byte(ptr + i)))
                        .collect();
                    records.push(json!({"which":which,"state":state,"length":length,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(sp+8),"guard":m.read_long(cpu.dar[15]),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"prefix":m.read_long(ptr-4),"suffix":m.read_long(ptr+600),"before_state":before,"after_state":after,"after_hex":bytes}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"dialog-item-text-runtime-raw-v1","records":records}).to_string())
}
