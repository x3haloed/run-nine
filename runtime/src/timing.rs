//! Deadline-based classic Delay. The host supplies the clock; no wall time in this service.
use m68k::CpuCore;
#[derive(Default)]
pub struct Timing {
    pending: Option<(u32, u32)>,
}
impl Timing {
    /// None: another trap. Some(false): keep the CPU at Delay until its deadline.
    pub fn dispatch(
        &mut self,
        opcode: u16,
        cpu: &mut CpuCore,
        now: u32,
    ) -> Result<Option<bool>, String> {
        if opcode != 0xa03b {
            return Ok(None);
        }
        let deadline = match self.pending {
            Some((pc, deadline)) if pc == cpu.pc => deadline,
            Some(_) => return Err("Delay resumed at another PC".into()),
            None => {
                let count = cpu.dar[8];
                if count > i32::MAX as u32 {
                    return Err("negative Delay tick count is unsupported".into());
                }
                let deadline = now.wrapping_add(count);
                self.pending = Some((cpu.pc, deadline));
                deadline
            }
        };
        if (now.wrapping_sub(deadline) as i32) < 0 {
            return Ok(Some(false));
        }
        cpu.dar[0] = now;
        cpu.pc += 2;
        self.pending = None;
        Ok(Some(true))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn waits_and_wraps() {
        for start in [10, u32::MAX - 1] {
            let mut t = Timing::default();
            let mut c = CpuCore::new();
            c.pc = 0x1000;
            c.dar[8] = 2;
            c.dar[0] = 123;
            for delta in 0..2 {
                assert_eq!(
                    t.dispatch(0xa03b, &mut c, start.wrapping_add(delta))
                        .unwrap(),
                    Some(false)
                );
                assert_eq!(c.pc, 0x1000);
                assert_eq!(c.dar[0], 123);
            }
            assert_eq!(
                t.dispatch(0xa03b, &mut c, start.wrapping_add(2)).unwrap(),
                Some(true)
            );
            assert_eq!(c.pc, 0x1002);
            assert_eq!(c.dar[0], start.wrapping_add(2));
            assert_eq!(c.dar[8], 2);
        }
    }
}
