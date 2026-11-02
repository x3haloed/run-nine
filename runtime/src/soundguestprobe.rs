//! Execute the same owned236-byte caller used by the OS9 reference probe.
use m68k::{AddressBus, CpuCore, CpuType, StepResult};
pub fn run() -> Result<String, String> {
    const PROGRAM: u32 = 0x19000;
    const BLOCK: u32 = 0x10000;
    const STOP: u32 = 0x18400;
    const STACK: u32 = 0xf0000;
    let mut m = crate::machine::Memory::zeroed(0x100000);
    m.put(
        PROGRAM as usize,
        include_bytes!("../../probes/soundguest/program.bin"),
    )?;
    m.write_long(BLOCK + 64, BLOCK + 128);
    m.write_long(BLOCK + 68, 4800);
    m.write_long(BLOCK + 72, 0xbb800000);
    m.write_byte(BLOCK + 85, 60);
    for i in 0..4800 {
        m.write_byte(
            BLOCK + 128 + i,
            if i < 1200 || i >= 3600 {
                128
            } else if (i - 1200) / 32 & 1 != 0 {
                160
            } else {
                96
            },
        );
    }
    for i in (24..36).step_by(2) {
        m.write_word(BLOCK + i, 0x5555);
    }
    let args = std::env::args().collect::<Vec<_>>();
    let mut sound = crate::soundmanager::Sound::new(&args, 0x100000)?;
    if !sound.active() {
        return Err("sound-guest-v1 requires --capture-audio or --audio-output".into());
    }
    let mut heap = classic_core::Heap::new(0x20000, 0xe0000);
    let mut callbacks = crate::callbacks::Callbacks::silent();
    let mut cpu = CpuCore::new();
    cpu.set_cpu_type(CpuType::M68020);
    cpu.pc = PROGRAM;
    for i in 0..15 {
        cpu.dar[i] = 0x22220000 + i as u32;
    }
    cpu.dar[11] = BLOCK;
    cpu.dar[15] = STACK;
    m.write_long(STACK, STOP);
    let registers = cpu.dar;
    let started = std::time::Instant::now();
    let mut done = false;
    for _ in 0..10000000 {
        if let Some((token, _)) = callbacks.finish(&mut cpu, &mut m)? {
            if token != 11 {
                return Err("unexpected callback owner".into());
            }
            sound.callback_result();
            continue;
        }
        let now = started.elapsed().as_micros() as u64;
        m.write_long(0x16a, (now * 60 / 1000000) as u32);
        if sound.service(&mut cpu, &mut m, now, &mut callbacks)? {
            continue;
        }
        if cpu.pc == STOP {
            done = true;
            break;
        }
        let opcode = m.read_word(cpu.pc);
        if opcode & 0xf000 == 0xa000 {
            if !sound.dispatch(opcode, &mut cpu, &mut m, &mut heap)? {
                return Err(format!("unsupported sound fixture trap {opcode:04x}"));
            }
        } else if !matches!(cpu.step(&mut m), StepResult::Ok { .. }) {
            return Err(format!("sound fixture CPU failed at {:#x}", cpu.pc));
        }
    }
    if !done || cpu.dar[15] != STACK + 4 {
        return Err("sound fixture did not return with correct stack".into());
    }
    for i in (2..8).chain(10..15) {
        if cpu.dar[i] != registers[i] {
            return Err(format!("sound fixture changed callee-saved register {i}"));
        }
    }
    let errors = (24..36)
        .step_by(2)
        .map(|i| m.read_word(BLOCK + i) as i16)
        .collect::<Vec<_>>();
    Ok(serde_json::json!({"probe":"sound-guest-v1","errors":errors,"callbacks":m.read_long(BLOCK+4),
        "callback_param1":m.read_long(BLOCK+8),"callback_channel_matches":m.read_long(BLOCK+12),"volume":m.read_long(BLOCK+36)}).to_string())
}
