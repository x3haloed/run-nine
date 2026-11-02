//! A bounded execution of owned 68K instructions, never a precomputed replay.
use crate::{traps, Heap};
use alloc::{format, string::String, vec::Vec};
use m68k::{AddressBus, CpuCore, CpuType, StepResult};
pub const OWNED_PROGRAM: &[u8] = include_bytes!("../../probes/owned68k/program.bin");
#[derive(Debug)]
pub struct TrapRecord {
    pub pc: u32,
    pub opcode: u16,
    pub d0_before: u32,
    pub d0_after: u32,
}
#[derive(Debug)]
pub struct OwnedExecution {
    pub instructions: u32,
    pub traps: Vec<TrapRecord>,
    pub result_hex: String,
    pub return_pc: u32,
    pub return_sp: u32,
    pub registers_preserved: bool,
}
impl OwnedExecution {
    pub fn observation(&self) -> String {
        format!(
            "{{\"schema\":1,\"probe\":\"owned-68k-v1\",\"execution\":{{\"result_hex\":\"{}\"}}}}",
            self.result_hex
        )
    }
}
pub fn owned_probe<B: AddressBus>(memory: &mut B) -> Result<OwnedExecution, String> {
    let entry = 0x1000;
    let exit = 0x1800;
    let stack = 0xf000;
    let output = 0x2004;
    for (i, b) in OWNED_PROGRAM.iter().enumerate() {
        memory.write_byte(entry + i as u32, *b);
    }
    for at in 0x2000..0x202c {
        memory.write_byte(at, 0x55);
    }
    memory.write_long(0, stack);
    memory.write_long(4, entry);
    memory.write_long(stack, exit);
    let mut cpu = CpuCore::new();
    cpu.set_cpu_type(CpuType::M68020);
    cpu.reset(memory);
    for i in 2..15 {
        cpu.dar[i] = 0x13570000 + i as u32;
    }
    cpu.dar[11] = output;
    let saved = cpu.dar;
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut instructions = 0;
    let mut records = Vec::new();
    while cpu.pc != exit {
        if instructions >= 10000 {
            return Err("Owned 68K program exceeded instruction budget".into());
        }
        let pc = cpu.pc;
        let opcode = memory.read_word(pc);
        if opcode & 0xf000 == 0xa000 {
            let before = cpu.dar[0];
            if !traps::memory_dispatch(opcode, &mut cpu, memory, &mut heap) {
                return Err(format!(
                    "Unsupported owned 68K trap {opcode:#06x} at {pc:#x}"
                ));
            }
            records.push(TrapRecord {
                pc,
                opcode,
                d0_before: before,
                d0_after: cpu.dar[0],
            });
        } else {
            match cpu.step(memory) {
                StepResult::Ok { .. } => {}
                other => {
                    return Err(format!(
                        "Owned 68K instruction failure at {pc:#x}: {other:?}"
                    ))
                }
            }
        }
        instructions += 1;
    }
    let registers_preserved = cpu.dar[2..8] == saved[2..8] && cpu.dar[10..15] == saved[10..15];
    if cpu.dar[15] != stack + 4 || !registers_preserved {
        return Err("Owned 68K return ABI mismatch".into());
    }
    if memory.read_long(output + 32) != 0x36384b21 {
        return Err("Owned 68K program did not reach its completion store".into());
    }
    let result_hex = (0x2000..0x202c)
        .map(|p| format!("{:02x}", memory.read_byte(p)))
        .collect::<String>();
    Ok(OwnedExecution {
        instructions,
        traps: records,
        result_hex,
        return_pc: cpu.pc,
        return_sp: cpu.dar[15],
        registers_preserved,
    })
}
