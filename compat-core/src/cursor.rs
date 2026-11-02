//! Classic cursor visibility bookkeeping in guest low memory, without host UI.
use crate::MemoryBus;
pub const VISIBLE: u32 = 0x8cc;
pub const STATE: u32 = 0x8d0;
pub const OBSCURE: u32 = 0x8d2;
pub fn initialize<B: MemoryBus>(m: &mut B) {
    m.write_word(STATE, 0);
    m.write_byte(OBSCURE, 0);
    m.write_byte(VISIBLE, 1);
}
pub fn moved<B: MemoryBus>(m: &mut B) {
    if m.read_byte(OBSCURE) != 0 {
        m.write_byte(OBSCURE, 0);
        let visible = m.read_word(STATE) == 0;
        m.write_byte(VISIBLE, u8::from(visible));
    }
}
#[cfg(feature = "m68k-adapter")]
pub fn dispatch<B: m68k::AddressBus>(op: u16, cpu: &mut m68k::CpuCore, m: &mut B) -> bool {
    match op {
        0xa850 => { initialize(m); cpu.dar[0] = 0; }
        0xa852 => {
            let state = MemoryBus::read_word(m, STATE) as i16;
            MemoryBus::write_word(m, STATE, state.wrapping_sub(1) as u16);
            MemoryBus::write_byte(m, VISIBLE, 0);
        }
        0xa853 => {
            let state = MemoryBus::read_word(m, STATE) as i16;
            if state < 0 {
                let after = state + 1;
                MemoryBus::write_word(m, STATE, after as u16);
                let visible = after == 0 && MemoryBus::read_byte(m, OBSCURE) == 0;
                MemoryBus::write_byte(m, VISIBLE, u8::from(visible));
            } else {
                initialize(m);
            }
        }
        0xa856 => {
            MemoryBus::write_byte(m, OBSCURE, 1);
            MemoryBus::write_byte(m, VISIBLE, 0);
        }
        _ => return false,
    }
    cpu.set_ccr(0);
    cpu.pc += 2;
    true
}
#[cfg(all(test, feature = "m68k-adapter"))]
mod tests {
    use super::*;
    #[test]
    fn deep_nesting_and_motion_keep_a_remaining_hide_effective() {
        let mut m = m68k::LinearMemoryBus::new(0x10000);
        let mut cpu = m68k::CpuCore::new();
        initialize(&mut m);
        for _ in 0..64 { assert!(dispatch(0xa852, &mut cpu, &mut m)); }
        for _ in 0..63 { assert!(dispatch(0xa853, &mut cpu, &mut m)); }
        assert_eq!(MemoryBus::read_word(&mut m, STATE) as i16, -1);
        dispatch(0xa856, &mut cpu, &mut m);
        moved(&mut m);
        assert_eq!(MemoryBus::read_byte(&mut m, OBSCURE), 0);
        assert_eq!(MemoryBus::read_byte(&mut m, VISIBLE), 0);
        dispatch(0xa853, &mut cpu, &mut m);
        assert_eq!(MemoryBus::read_byte(&mut m, VISIBLE), 1);
        for _ in 0..64 { dispatch(0xa853, &mut cpu, &mut m); }
        assert_eq!(MemoryBus::read_word(&mut m, STATE), 0);
    }
}
