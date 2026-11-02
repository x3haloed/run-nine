//! CPU/clock adapter for the shared host-independent Time Manager state.
use crate::callbacks::Callbacks;
use m68k::{AddressBus, CpuCore};
pub struct TimeManager {
    pub state: classic_core::timers::Timers,
    running: bool,
    pub trace: bool,
}
impl TimeManager {
    pub fn new(limit: u32) -> Self {
        Self {
            state: classic_core::timers::Timers::new(limit),
            running: false,
            trace: false,
        }
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        opcode: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        now: u64,
    ) -> bool {
        let task = cpu.dar[8];
        let request = cpu.dar[0] as i32;
        let result = match opcode {
            0xa058 => self.state.install(m, task, false),
            0xa458 => self.state.install(m, task, true),
            0xa05a => self.state.prime(m, task, request, now),
            0xa059 => self.state.remove(m, task, now),
            _ => return false,
        };
        if self.trace {
            println!("Time Manager trap={opcode:#x} task={task:#x} request={request} now_us={now} status={result:?}");
        }
        cpu.dar[0] = result.err().unwrap_or(0) as i32 as u32;
        cpu.set_ccr(if cpu.dar[0] == 0 { 4 } else { 8 });
        cpu.pc += 2;
        true
    }
    pub fn service<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        now: u64,
        callbacks: &mut Callbacks,
    ) -> Result<bool, String> {
        if self.running {
            return Ok(false);
        }
        while let Some((task, procedure)) = self.state.due(m, now) {
            if procedure == 0 {
                continue;
            }
            if self.trace {
                println!(
                    "Time Manager callback task={task:#x} procedure={procedure:#x} now_us={now}"
                );
            }
            callbacks.call_register(cpu, m, procedure, &[(8, procedure), (9, task)], 7)?;
            self.running = true;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn is_running(&self) -> bool {
        self.running
    }
    pub fn callback_result(&mut self) -> Result<(), String> {
        if !self.running {
            return Err("Timer callback returned without active task".into());
        }
        self.running = false;
        Ok(())
    }
}
/// One monotonic time domain for real presentation time and explicit headless
/// tick advancement. The core scheduler itself receives only integer clocks.
pub struct RunClock {
    started: std::time::Instant,
    last_ticks: u32,
    ticks: u64,
    now: u64,
}
impl RunClock {
    pub fn new() -> Self {
        Self {
            started: std::time::Instant::now(),
            last_ticks: 0,
            ticks: 0,
            now: 0,
        }
    }
    fn sample(&mut self, ticks: u32, elapsed: u64) -> u64 {
        let delta = ticks.wrapping_sub(self.last_ticks);
        if delta <= i32::MAX as u32 {
            self.ticks += delta as u64;
            self.last_ticks = ticks;
        }
        self.now = self
            .now
            .max(elapsed)
            .max(self.ticks.saturating_mul(1_000_000).saturating_add(59) / 60);
        self.now
    }
    pub fn now(&mut self, ticks: u32) -> u64 {
        self.sample(
            ticks,
            self.started.elapsed().as_micros().min(u64::MAX as u128) as u64,
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn presentation_and_synthetic_ticks_never_move_clock_backward() {
        let mut c = RunClock::new();
        assert_eq!(c.sample(1, 100), 16667);
        assert_eq!(c.sample(0, 200), 16667);
        assert_eq!(c.sample(1, 30000), 30000);
        c.last_ticks = u32::MAX;
        c.ticks = u32::MAX as u64;
        let before = c.sample(u32::MAX, 40000);
        assert!(c.sample(1, 50000) > before);
    }
}
