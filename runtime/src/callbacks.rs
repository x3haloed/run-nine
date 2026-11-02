//! Pascal callbacks execute on the application's CPU, including their traps.
use m68k::{AddressBus, CpuCore};
pub const RETURN: u32 = 0x18200;
pub enum Arg {
    Word(u16),
    Long(u32),
}
enum ReturnKind {
    Word,
    Boolean,
    Register,
    Void,
}
struct Frame {
    registers: [u32; 16],
    pc: u32,
    sr: u16,
    result: u32,
    token: u32,
    kind: ReturnKind,
}
pub struct Callbacks {
    frames: Vec<Frame>,
    trace: bool,
}
impl Callbacks {
    pub fn new() -> Self {
        Self {
            frames: Vec::new(),
            trace: true,
        }
    }
    pub fn silent() -> Self {
        Self {
            frames: Vec::new(),
            trace: false,
        }
    }
    pub fn call_word<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        target: u32,
        args: &[Arg],
        token: u32,
    ) -> Result<(), String> {
        self.call_pascal(cpu, m, target, args, token, ReturnKind::Word)
    }
    fn call_pascal<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        target: u32,
        args: &[Arg],
        token: u32,
        kind: ReturnKind,
    ) -> Result<(), String> {
        if target == 0 {
            return Err("nil Pascal callback".into());
        }
        let original = cpu.dar[15];
        let result = original
            .checked_sub(if matches!(kind, ReturnKind::Void) {
                0
            } else {
                2
            })
            .ok_or("callback stack underflow")?;
        let frame = Frame {
            registers: cpu.dar,
            pc: cpu.pc,
            sr: cpu.get_sr(),
            result,
            token,
            kind,
        };
        let mut sp = result;
        if !matches!(frame.kind, ReturnKind::Void) {
            m.write_word(sp, 0);
        }
        for arg in args {
            match arg {
                Arg::Word(value) => {
                    sp = sp.checked_sub(2).ok_or("callback stack underflow")?;
                    m.write_word(sp, *value);
                }
                Arg::Long(value) => {
                    sp = sp.checked_sub(4).ok_or("callback stack underflow")?;
                    m.write_long(sp, *value);
                }
            }
        }
        sp = sp.checked_sub(4).ok_or("callback stack underflow")?;
        m.write_long(sp, RETURN);
        self.frames.push(frame);
        if self.trace {
            println!("callback: enter {target:#x}, token={token}");
        }
        cpu.dar[15] = sp;
        cpu.pc = target;
        Ok(())
    }
    /// Pascal Boolean return occupies the high byte of its two-byte stack slot.
    pub fn call_byte<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        target: u32,
        args: &[Arg],
        token: u32,
    ) -> Result<(), String> {
        self.call_pascal(cpu, m, target, args, token, ReturnKind::Boolean)
    }
    /// Pascal procedure: no result slot; callee removes just its arguments.
    pub fn call_void<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        target: u32,
        args: &[Arg],
        token: u32,
    ) -> Result<(), String> {
        self.call_pascal(cpu, m, target, args, token, ReturnKind::Void)
    }
    /// Register-based Toolbox hook. Result is D0.w, with an ordinary return
    /// address on the stack; the outer trap resumes after real CPU execution.
    pub fn call_register<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        target: u32,
        registers: &[(usize, u32)],
        token: u32,
    ) -> Result<(), String> {
        if target == 0 {
            return Err("nil register callback".into());
        }
        if registers.iter().any(|(index, _)| *index >= 15) {
            return Err("invalid callback register".into());
        }
        let original = cpu.dar[15];
        let sp = original.checked_sub(4).ok_or("callback stack underflow")?;
        self.frames.push(Frame {
            registers: cpu.dar,
            pc: cpu.pc,
            sr: cpu.get_sr(),
            result: original,
            token,
            kind: ReturnKind::Register,
        });
        m.write_long(sp, RETURN);
        for &(index, value) in registers {
            cpu.dar[index] = value;
        }
        cpu.dar[15] = sp;
        cpu.pc = target;
        if self.trace {
            println!("callback: enter {target:#x}, token={token} (register hook)");
        }
        Ok(())
    }
    pub fn finish<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
    ) -> Result<Option<(u32, i16)>, String> {
        if cpu.pc != RETURN {
            return Ok(None);
        }
        let frame = self
            .frames
            .pop()
            .ok_or("callback return without a continuation")?;
        if cpu.dar[15] != frame.result {
            return Err(format!(
                "callback returned with wrong stack: {:#x}, expected {:#x}",
                cpu.dar[15], frame.result
            ));
        }
        let result = match frame.kind {
            ReturnKind::Register => cpu.dar[0] as i16,
            ReturnKind::Boolean => m.read_byte(frame.result) as i16,
            ReturnKind::Word => m.read_word(frame.result) as i16,
            ReturnKind::Void => 0,
        };
        if self.trace {
            println!("callback: return token={} result={result}", frame.token);
        }
        cpu.set_sr_noint_nosp(frame.sr);
        cpu.dar = frame.registers;
        cpu.pc = frame.pc;
        Ok(Some((frame.token, result)))
    }
}
