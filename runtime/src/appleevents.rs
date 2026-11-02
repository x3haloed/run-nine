//! Application and system Apple Event handler registries for 68K Pascal UPPs.
//! Descriptor transport and callback delivery are separate, not implied here.
use m68k::{AddressBus, CpuCore};
use std::collections::BTreeMap;
#[derive(Clone, Copy)]
struct Handler {
    address: u32,
    refcon: u32,
}
pub struct AppleEvents {
    handlers: BTreeMap<(bool, u32, u32), Handler>,
}
impl AppleEvents {
    pub fn new() -> Self {
        Self {
            handlers: BTreeMap::new(),
        }
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
    ) -> Result<bool, String> {
        if op != 0xa816 {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let selector = cpu.dar[0] as u16;
        let system = m.read_byte(sp) != 0;
        let (error, pop) = match selector {
            0x091f => {
                let refcon = m.read_long(sp + 2);
                let address = m.read_long(sp + 6);
                let id = m.read_long(sp + 10);
                let class = m.read_long(sp + 14);
                let error = if address == 0 {
                    -50
                } else {
                    self.handlers
                        .insert((system, class, id), Handler { address, refcon });
                    0
                };
                (error, 18)
            }
            0x0921 => {
                let refcon_output = m.read_long(sp + 2);
                let handler_output = m.read_long(sp + 6);
                let id = m.read_long(sp + 10);
                let class = m.read_long(sp + 14);
                let error = if let Some(handler) = self.handlers.get(&(system, class, id)) {
                    m.write_long(handler_output, handler.address);
                    m.write_long(refcon_output, handler.refcon);
                    0
                } else {
                    -1717
                };
                (error, 18)
            }
            0x0720 => {
                let address = m.read_long(sp + 2);
                let id = m.read_long(sp + 6);
                let class = m.read_long(sp + 10);
                let key = (system, class, id);
                let error = if let Some(handler) = self.handlers.get(&key) {
                    // Native returns success for a mismatched address, but
                    // preserves the installed handler. Missing keys are errors.
                    if handler.address == address {
                        self.handlers.remove(&key);
                    }
                    0
                } else {
                    -1717
                };
                (error, 14)
            }
            _ => {
                return Err(format!(
                    "unimplemented Apple Event selector {selector:#06x}"
                ))
            }
        };
        m.write_word(sp + pop, error as i16 as u16);
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
