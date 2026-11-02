//! Sound allocation discovery for the explicitly absent output backend.
//! No channel, queue, callback, playback or completion is fabricated.
use m68k::{AddressBus, CpuCore};
pub fn dispatch<B: AddressBus>(opcode: u16, cpu: &mut CpuCore, memory: &mut B) -> bool {
    if opcode != 0xa807 {
        return false;
    }
    let sp = cpu.dar[15];
    let synth = memory.read_word(sp + 8) as i16;
    let pointer = memory.read_long(sp + 10);
    let error: i16 = if pointer == 0 {
        -50
    } else if synth == -1 {
        // No output components are registered. OS9 returns this same failure
        // for an explicitly requested nonexistent output component.
        -2004
    } else {
        // Declared backend capability, not native-default-output equivalence.
        // There is no hardware sink and therefore no new sound channel.
        -200
    };
    if std::env::args().any(|a| a == "--trace-sound") {
        println!("sound allocation: synth={synth} init={:#x} callback={:#x} channel_pointer={pointer:#x} error={error}",memory.read_long(sp+4),memory.read_long(sp));
    }
    // A failure leaves the caller's pointer/owned record intact. In particular
    // it must not erase caller-supplied channel storage on the no-device path.
    memory.write_word(sp + 14, error as u16);
    cpu.dar[15] += 14;
    cpu.set_ccr(8);
    cpu.pc += 2;
    true
}
pub fn no_output_probe() -> Result<String, String> {
    let mut rows = Vec::new();
    for init in [0x7a5a3921u32, 0x74737421] {
        for input_ccr in [16, 31, 0] {
            let mut memory = m68k::LinearMemoryBus::new(4096);
            let mut cpu = CpuCore::new();
            cpu.pc = 0x100;
            cpu.dar[15] = 0x300;
            cpu.set_ccr(input_ccr);
            memory.write_long(0x200, 0x11223344);
            memory.write_long(0x204, 0);
            memory.write_long(0x208, 0x55667788);
            memory.write_long(0x300, 0);
            memory.write_long(0x304, init);
            memory.write_word(0x308, 0xffff);
            memory.write_long(0x30a, 0x204);
            memory.write_word(0x30e, 0x5555);
            memory.write_long(0x310, 0x66667777);
            if !dispatch(0xa807, &mut cpu, &mut memory) || cpu.pc != 0x102 {
                return Err("Sound allocation dispatch/PC failure".into());
            }
            let ccr = (u32::from(cpu.flag_x()) << 4)
                | (u32::from(cpu.flag_n()) << 3)
                | (u32::from(cpu.flag_z()) << 2)
                | (u32::from(cpu.flag_v()) << 1)
                | u32::from(cpu.flag_c());
            rows.push(serde_json::json!({"synth":-1,"init":init,"input_ccr":input_ccr,"error":memory.read_word(0x30e) as i16,
                "ccr":ccr,"stack_delta":cpu.dar[15] as i32-0x314,"stack_guard":memory.read_long(0x310),"channel_null":(memory.read_long(0x204)==0) as u8,
                "pointer_guards":[memory.read_long(0x200),memory.read_long(0x208)]}));
        }
    }
    Ok(
        serde_json::json!({"schema":1,"probe":"sound-no-output-v1","sound":{"cases":rows}})
            .to_string(),
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn no_backend_keeps_borrowed_channel_and_never_calls_callback() {
        let mut memory = m68k::LinearMemoryBus::new(4096);
        let mut cpu = CpuCore::new();
        for p in 0x400..0x430 {
            memory.write_byte(p, 0x55);
        }
        memory.write_word(0x41e, 128);
        memory.write_long(0x200, 0x400);
        cpu.pc = 0x100;
        cpu.dar[15] = 0x300;
        memory.write_long(0x300, 0xdeadbeef);
        memory.write_long(0x304, 0x80);
        memory.write_word(0x308, 5);
        memory.write_long(0x30a, 0x200);
        memory.write_long(0x310, 0x87654321);
        let before = (0x400..0x430)
            .map(|p| memory.read_byte(p))
            .collect::<Vec<_>>();
        assert!(dispatch(0xa807, &mut cpu, &mut memory));
        assert_eq!(memory.read_word(0x30e) as i16, -200);
        assert_eq!(memory.read_long(0x200), 0x400);
        assert_eq!((cpu.pc, cpu.dar[15]), (0x102, 0x30e));
        assert_eq!(memory.read_long(0x310), 0x87654321);
        assert_eq!(
            before,
            (0x400..0x430)
                .map(|p| memory.read_byte(p))
                .collect::<Vec<_>>()
        );
    }
}
