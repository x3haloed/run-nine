//! Real shared memory-trap replay, with full owned payload/guard observations.
use crate::{machine::Memory, managers::Heap};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn payload(m: &mut Memory, heap: &mut Heap, h: u32, n: u32) -> Result<String, String> {
    if h == 0 {
        return Ok(String::new());
    }
    let p = heap.data(h).map_err(|e| e.to_string())?;
    Ok((0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect())
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut records = Vec::new();
    for (kind, length) in [0u32, 0, 1, 17, 512].into_iter().enumerate() {
        for state in if kind == 0 {
            vec![0]
        } else {
            vec![0, 64, 128, 192]
        } {
            for flags in [16, 31, 0] {
                let guard = heap
                    .new_handle(&mut m, 37, false)
                    .map_err(|e| e.to_string())?;
                let g = heap.data(guard).map_err(|e| e.to_string())?;
                for i in 0..37 {
                    m.write_byte(g + i, (i * 19 + 7) as u8);
                }
                let source = if kind == 0 {
                    0
                } else {
                    let h = heap
                        .new_handle(&mut m, length, false)
                        .map_err(|e| e.to_string())?;
                    let p = heap.data(h).map_err(|e| e.to_string())?;
                    for i in 0..length {
                        m.write_byte(p + i, (i * 37 + 19) as u8);
                    }
                    heap.set_state(h, state).map_err(|e| e.to_string())?;
                    h
                };
                let mut cpu = CpuCore::new();
                cpu.pc = 0x6000;
                cpu.dar[15] = 0x700000;
                cpu.dar[0] = 0x12345678;
                cpu.dar[8] = source;
                cpu.set_ccr(flags);
                m.write_long(cpu.dar[15], 0x55667788);
                assert!(classic_core::traps::memory_dispatch(
                    0xa9e1, &mut cpu, &mut m, &mut heap
                ));
                assert_eq!(cpu.pc, 0x6002);
                assert_eq!(m.read_long(0x700000), 0x55667788);
                let copy = cpu.dar[8];
                records.push(json!({"kind":kind,"state":state,"length":length,"input_d0":305419896u32,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_ccr(),"stack_delta":0x700000i64-cpu.dar[15]as i64,"mem_error":m.read_word(0x220)as i16,"output_nonnull":i32::from(copy!=0),"distinct_source":i32::from(copy!=0&&copy!=source),"source_size":if source!=0{heap.size(source).map_err(|e|e.to_string())?as i64}else{-1},"copy_size":if copy!=0{heap.size(copy).map_err(|e|e.to_string())?as i64}else{-1},"source_state":if source!=0{heap.state(source).map_err(|e|e.to_string())?as i16}else{-1},"copy_state":if copy!=0{heap.state(copy).map_err(|e|e.to_string())?as i16}else{-1},"source_hex":payload(&mut m,&mut heap,source,length)?,"copy_hex":payload(&mut m,&mut heap,copy,length)?,"guard_hex":payload(&mut m,&mut heap,guard,37)?}));
                if copy != 0 && copy != source {
                    heap.dispose(&mut m, copy).map_err(|e| e.to_string())?;
                }
                if source != 0 {
                    heap.dispose(&mut m, source).map_err(|e| e.to_string())?;
                }
                heap.dispose(&mut m, guard).map_err(|e| e.to_string())?;
            }
        }
    }
    Ok(json!({"schema":1,"probe":"hand-clone-v1","memory":{"records":records}}).to_string())
}
