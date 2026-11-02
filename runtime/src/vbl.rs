//! CPU continuations for the host-independent vertical-retrace queues.
use crate::callbacks::Callbacks;
use classic_core::retrace::{Queue, Retrace};
use m68k::{AddressBus, CpuCore};
struct Manual {
    pc: u32,
    sp: u32,
    slot: u16,
    ready: bool,
}
pub struct VBL {
    pub state: Retrace,
    running: bool,
    manual: Option<Manual>,
    pub trace: bool,
}
impl VBL {
    pub fn new(limit: u32) -> Self {
        Self {
            state: Retrace::new(limit),
            running: false,
            manual: None,
            trace: false,
        }
    }
    pub fn is_busy(&self) -> bool {
        self.running || self.manual.is_some()
    }
    pub fn manual_pending(&self) -> bool {
        self.manual.is_some()
    }
    fn result(cpu: &mut CpuCore, error: i16, queue: Queue) {
        let x = if cpu.flag_x() { 16 } else { 0 };
        let modern = matches!(queue,Queue::Slot(slot) if slot>=32);
        if error == -337 {
            cpu.dar[0] = (cpu.dar[0] & 0xffff0000) | error as u16 as u32;
            cpu.set_ccr(8);
        } else {
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr((if modern { 0 } else { x }) | if error == 0 { 4 } else { 8 });
        }
        cpu.pc += 2;
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
    ) -> Result<bool, String> {
        if op == 0xa072 {
            if let Some(manual) = &self.manual {
                if cpu.pc != manual.pc || cpu.dar[15] != manual.sp {
                    return Err("DoVBLTask continuation frame changed".into());
                }
                if manual.ready {
                    let borrow = self.state.last_pulse_borrow();
                    cpu.dar[0] = 0;
                    cpu.set_ccr(4 | if borrow { 16 } else { 0 });
                    cpu.pc += 2;
                    self.manual = None;
                }
                return Ok(true);
            }
            if self.running {
                return Err(
                    "recursive DoVBLTask inside a VBL callback is outside current profile".into(),
                );
            }
            let slot = cpu.dar[0] as u16;
            let queue = Queue::Slot(slot);
            if !self.state.valid_queue(queue) {
                Self::result(cpu, -337, queue);
                return Ok(true);
            }
            if slot >= 32 {
                return Err(
                    "manual modern-driver VBL pulses are outside the measured profile".into(),
                );
            }
            self.state
                .begin_pulse(queue)
                .map_err(|e| format!("DoVBLTask pulse busy: {e}"))?;
            self.manual = Some(Manual {
                pc: cpu.pc,
                sp: cpu.dar[15],
                slot,
                ready: false,
            });
            return Ok(true);
        }
        let queue = match op {
            0xa033 | 0xa034 => Queue::System,
            0xa06f | 0xa070 => Queue::Slot(cpu.dar[0] as u16),
            _ => return Ok(false),
        };
        let task = cpu.dar[8];
        let result = if matches!(op, 0xa033 | 0xa06f) {
            self.state.install(m, task, queue)
        } else {
            self.state.remove(m, task, queue)
        };
        if self.trace {
            println!("VBL trap={op:#x} queue={queue:?} task={task:#x} status={result:?}");
            if result.is_ok() {
                println!(
                    "VBL record: type={} count={} phase={} procedure={:#x}",
                    m.read_word(task + 4),
                    m.read_word(task + 10) as i16,
                    m.read_word(task + 12) as i16,
                    m.read_long(task + 6)
                );
            }
        }
        Self::result(cpu, result.err().unwrap_or(0), queue);
        Ok(true)
    }
    pub fn service<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        tick: u64,
        callbacks: &mut Callbacks,
    ) -> Result<bool, String> {
        if self.running {
            return Ok(false);
        }
        let due = if let Some(manual) = self.manual.as_mut() {
            if manual.ready {
                return Ok(false);
            }
            let due = self.state.next_pulse_callback(m);
            if due.is_none() && !self.state.pulse_pending() {
                manual.ready = true;
                if self.trace {
                    println!("VBL explicit pulse complete: slot={}", manual.slot);
                }
            }
            due
        } else {
            self.state.next_time_callback(m, tick)
        };
        if let Some((task, procedure)) = due {
            if procedure == 0 {
                return Err("nil VBL procedure at delivery".into());
            }
            if self.trace {
                println!("VBL callback task={task:#x} procedure={procedure:#x} tick={tick}");
            }
            callbacks.call_register(cpu, m, procedure, &[(8, task)], 8)?;
            self.running = true;
            return Ok(true);
        }
        Ok(false)
    }
    pub fn callback_result<B: AddressBus>(&mut self, m: &mut B) -> Result<(), String> {
        if !self.running {
            return Err("VBL callback returned without active delivery".into());
        }
        self.state
            .finish_callback(m)
            .map_err(|e| format!("VBL callback completion: {e}"))?;
        self.running = false;
        Ok(())
    }
}
