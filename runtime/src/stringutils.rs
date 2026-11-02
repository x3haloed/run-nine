//! Register-ABI string equality. Macintosh Roman folding needs its own native
//! measurements; Unicode casing is not a substitute for the classic tables.
use m68k::{AddressBus, CpuCore};
pub fn dispatch<B: AddressBus>(opcode: u16, cpu: &mut CpuCore, m: &mut B) -> Result<bool, String> {
    if ![0xa03c, 0xa23c, 0xa43c, 0xa63c].contains(&opcode) {
        return Ok(false);
    }
    let first = (cpu.dar[0] >> 16) as usize;
    let second = cpu.dar[0] as u16 as usize;
    let case_sensitive = opcode & 0x400 != 0;
    let marks_sensitive = opcode & 0x200 == 0;
    let equal = if first != second {
        false
    } else {
        let mut equal = true;
        for i in 0..first {
            let a = m.read_byte(cpu.dar[8].wrapping_add(i as u32));
            let b = m.read_byte(cpu.dar[9].wrapping_add(i as u32));
            if a == b {
                continue;
            }
            if (!a.is_ascii() || !b.is_ascii()) && (!case_sensitive || !marks_sensitive) {
                return Err(
                    "Macintosh Roman string case/diacritic folding is not implemented".into(),
                );
            }
            if case_sensitive || !a.eq_ignore_ascii_case(&b) {
                equal = false;
            }
        }
        equal
    };
    cpu.dar[0] = (!equal) as u32;
    // The OS trap result is inequality, unlike the Pascal EqualString Boolean.
    cpu.set_ccr((if cpu.flag_x() { 0x10 } else { 0 }) | if equal { 4 } else { 0 });
    cpu.pc += 2;
    Ok(true)
}
