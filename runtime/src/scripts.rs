//! The current runtime profile supplies Roman text, fonts and keyboard input.
//! Queries outside that context remain explicit unsupported operations.
use m68k::{AddressBus, CpuCore};
pub struct Scripts {
    system: i16,
    application: i16,
    keyboard: i16,
}
impl Scripts {
    pub fn roman() -> Self {
        Self {
            system: 0,
            application: 0,
            keyboard: 0,
        }
    }
    pub fn dispatch<B: AddressBus>(
        &self,
        opcode: u16,
        cpu: &mut CpuCore,
        m: &mut B,
    ) -> Result<bool, String> {
        if opcode != 0xa8b5 {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let selector = m.read_long(sp);
        let (value, pop) = match selector {
            0x84020008 => {
                let variable = m.read_word(sp + 4);
                let value = match variable {
                    18 => self.system,
                    20 => self.application,
                    22 => self.keyboard,
                    _ => return Err(format!("unsupported Script Manager variable {variable}")),
                };
                (value as i32, 6)
            }
            0x8404000c => {
                let variable = m.read_word(sp + 4);
                let script = m.read_word(sp + 6) as i16;
                let script = if script == -1 { self.system } else { script };
                if script != 0 {
                    return Err(format!("unsupported script {script}"));
                }
                // Roman text flows left to right, is left aligned and permits
                // all seven classic character-style bits. Raster coverage is
                // separately bounded by the Font Manager's providers.
                let value = match variable {
                    6 | 8 => 0,
                    90 => 127,
                    _ => return Err(format!("unsupported Roman script variable {variable}")),
                };
                (value, 8)
            }
            _ => {
                return Err(format!(
                    "unsupported ScriptUtil selector {selector:#010x} at PC {:#x}",
                    cpu.pc
                ))
            }
        };
        m.write_long(sp + pop, value as u32);
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
