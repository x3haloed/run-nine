//! CPU-facing classic memory services shared by hosted and freestanding runs.
use crate::Heap;
use m68k::{AddressBus, CpuCore};

pub fn word_result(cpu: &mut CpuCore) {
    let result = cpu.dar[0] as u16;
    cpu.set_ccr(
        (if cpu.flag_x() { 16 } else { 0 })
            | (if result == 0 { 4 } else { 0 })
            | (if result & 0x8000 != 0 { 8 } else { 0 }),
    );
}
pub fn memory_result<B: AddressBus>(cpu: &mut CpuCore, memory: &mut B, error: i16) {
    memory.write_word(0x220, error as u16);
    word_result(cpu);
}
pub fn memory_dispatch<B: AddressBus>(
    opcode: u16,
    cpu: &mut CpuCore,
    memory: &mut B,
    heap: &mut Heap,
) -> bool {
    if opcode == 0xa01c {
        cpu.dar[0] = heap.free_bytes();
        word_result(cpu);
        cpu.pc += 2;
        return true;
    }
    let h = cpu.dar[8];
    let size = cpu.dar[0];
    match opcode {
        0xa9e1 => {
            cpu.dar[8] = heap.duplicate(memory, h).unwrap_or(0);
            cpu.dar[0] = heap.error as i32 as u32;
            // Native HandToHand clears X on success and the nil-handle path.
            cpu.set_ccr(0);
        }
        0xa122 | 0xa322 => {
            cpu.dar[8] = heap.new_handle(memory, size, opcode == 0xa322).unwrap_or(0);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa11e | 0xa31e => {
            cpu.dar[8] = heap.new_ptr(memory, size, opcode == 0xa31e).unwrap_or(0);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa025 => {
            let size = heap.size(h);
            cpu.dar[0] = size.unwrap_or_else(|e| e as i32 as u32);
            if h == 0 {
                // The native nil-handle error path clears X as well as
                // setting word-sized NZVC. Other paths retain the existing
                // word-result behavior.
                // Other invalid master pointers are outside this observation.
                cpu.set_ccr(0);
            }
        }
        0xa023 => {
            let _ = heap.dispose(memory, h);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa064 => {
            let _ = heap.move_high(memory, h);
            cpu.dar[0] = heap.error as i32 as u32;
            // Measured MoveHHi returns clear X for both success and errors.
            cpu.set_ccr(0);
        }
        0xa024 => {
            let _ = heap.resize(memory, h, size);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa029 | 0xa02a => {
            let _ = heap.lock(h, opcode == 0xa029);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa01f => {
            let _ = heap.dispose_ptr(h);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        0xa036 => {
            let _ = heap.more_masters(memory);
            cpu.dar[0] = heap.error as i32 as u32;
        }
        _ => return false,
    }
    memory_result(cpu, memory, heap.error);
    cpu.pc += 2;
    true
}
