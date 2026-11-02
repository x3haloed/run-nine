//! Independent replay: production queue/CPU adapters and owned 68K instructions.
use crate::{callbacks::Callbacks, machine::Memory, vbl::VBL};
use m68k::{AddressBus, CpuCore, CpuType, StepResult};
use serde_json::{json, Value};
const TASK: u32 = 0x2000;
const PROC: u32 = 0x19000;
struct Scene {
    m: Memory,
    cpu: CpuCore,
    vbl: VBL,
    callbacks: Callbacks,
}
impl Scene {
    fn new(slot: u16) -> Result<Self, String> {
        let mut m = Memory::zeroed(0x800000);
        m.put(
            PROC as usize,
            include_bytes!("../../probes/vbl/callback.bin"),
        )?;
        let mut vbl = VBL::new(0x800000);
        if slot >= 32 {
            vbl.state.register_slot(slot).map_err(|e| e.to_string())?;
        }
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(CpuType::M68020);
        cpu.dar[13] = 0x13572468;
        Ok(Self {
            m,
            cpu,
            vbl,
            callbacks: Callbacks::silent(),
        })
    }
    fn task(&mut self, count: i16, phase: i16, limit: u32) {
        for p in TASK - 4..TASK + 70 {
            self.m.write_byte(p, 0);
        }
        self.m.write_long(TASK - 4, 0x11223344);
        self.m.write_word(TASK + 4, 1);
        self.m.write_long(TASK + 6, PROC);
        self.m.write_word(TASK + 10, count as u16);
        self.m.write_word(TASK + 12, phase as u16);
        self.m.write_long(TASK + 28, limit);
        self.m.write_word(TASK + 32, 3);
        self.m.write_long(TASK + 34, 0x55667788);
        self.m.write_long(TASK + 66, 0x99aabbcc);
    }
    fn step(&mut self) -> Result<(), String> {
        if let Some((token, _)) = self.callbacks.finish(&mut self.cpu, &mut self.m)? {
            if token != 8 {
                return Err("wrong VBL continuation token".into());
            }
            self.vbl.callback_result(&mut self.m)?;
            return Ok(());
        }
        let tick = self.m.read_long(0x16a) as u64;
        if self
            .vbl
            .service(&mut self.cpu, &mut self.m, tick, &mut self.callbacks)?
        {
            return Ok(());
        }
        let op = self.m.read_word(self.cpu.pc);
        if self.vbl.dispatch(op, &mut self.cpu, &mut self.m)? {
            return Ok(());
        }
        match self.cpu.step(&mut self.m) {
            StepResult::Ok { .. } => Ok(()),
            other => Err(format!("owned VBL routine stopped: {other:?}")),
        }
    }
    fn call(&mut self, op: u16, input: u32, ccr: u8) -> Result<(u32, u16, i64, u8), String> {
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = 0x700000;
        self.cpu.dar[0] = input;
        self.cpu.dar[8] = TASK;
        self.cpu.set_ccr(ccr);
        self.m.write_word(0x6000, op);
        self.m.write_long(0x700000, 0xcafebabe);
        if !self.vbl.dispatch(op, &mut self.cpu, &mut self.m)? {
            return Err("VBL API not dispatched".into());
        }
        for _ in 0..2000 {
            if self.cpu.pc == 0x6002 {
                break;
            }
            self.step()?;
        }
        if self.cpu.pc != 0x6002
            || self.m.read_long(0x700000) != 0xcafebabe
            || self.m.read_long(TASK + 66) != 0x99aabbcc
        {
            return Err("VBL API did not complete or corrupted guards".into());
        }
        Ok((
            self.cpu.dar[0],
            self.cpu.get_sr() & 31,
            0x700000i64 - self.cpu.dar[15] as i64,
            (self.cpu.dar[8] == TASK) as u8,
        ))
    }
    fn automatic(&mut self, tick: u32) -> Result<(), String> {
        self.m.write_long(0x16a, tick);
        self.cpu.pc = 0x6002;
        self.cpu.dar[15] = 0x700000;
        self.cpu.set_ccr(31);
        let dar = self.cpu.dar;
        let sr = self.cpu.get_sr();
        for _ in 0..16 {
            if !self
                .vbl
                .service(&mut self.cpu, &mut self.m, tick as u64, &mut self.callbacks)?
            {
                return Ok(());
            }
            let mut done = false;
            for _ in 0..2000 {
                if let Some((token, _)) = self.callbacks.finish(&mut self.cpu, &mut self.m)? {
                    if token != 8 {
                        return Err("wrong automatic VBL token".into());
                    }
                    self.vbl.callback_result(&mut self.m)?;
                    done = true;
                    break;
                }
                match self.cpu.step(&mut self.m) {
                    StepResult::Ok { .. } => {}
                    other => return Err(format!("automatic VBL CPU stopped: {other:?}")),
                }
            }
            if !done
                || self.cpu.pc != 0x6002
                || self.cpu.dar != dar
                || self.cpu.get_sr() != sr
                || self.m.read_long(TASK + 66) != 0x99aabbcc
            {
                return Err(
                    "VBL callback failed foreground PC/SP/register/SR/guard restoration".into(),
                );
            }
        }
        Err("VBL automatic delivery did not quiesce".into())
    }
    fn snapshot(&mut self) -> Value {
        json!({"calls":self.m.read_long(TASK+14),"self_same":(self.m.read_long(TASK+18)==TASK) as u8,"callback_type":self.m.read_word(TASK+22),"callback_count":self.m.read_word(TASK+24) as i16,"callback_phase":self.m.read_word(TASK+26) as i16,"after_type":self.m.read_word(TASK+4),"after_count":self.m.read_word(TASK+10) as i16,"after_phase":self.m.read_word(TASK+12) as i16,"guards":[self.m.read_long(TASK-4),self.m.read_long(TASK+34)]})
    }
    fn ticks(&mut self, before: u32, after: u32) -> Value {
        json!([
            before,
            self.m.read_long(TASK + 38),
            self.m.read_long(TASK + 42),
            self.m.read_long(TASK + 46),
            after
        ])
    }
}
pub fn probe() -> Result<String, String> {
    let args = std::env::args().collect::<Vec<_>>();
    let slot = args
        .iter()
        .position(|s| s == "--reference-slot")
        .and_then(|i| args.get(i + 1))
        .map(|s| s.parse::<u16>())
        .transpose()
        .map_err(|e| e.to_string())?
        .unwrap_or(32);
    let mut records = Vec::new();
    let mut slots = (0..=16).collect::<Vec<i16>>();
    slots.extend([-1, -2]);
    for value in slots {
        for upper in [0, 0x12340000] {
            for ccr in [16, 31, 0] {
                let mut s = Scene::new(slot)?;
                s.task(32000, 0, 1);
                let system = value == -2;
                let input = upper | if system { 0x3456 } else { value as u16 as u32 };
                for step in 0..2 {
                    let op = if system { 0xa033 + step } else { 0xa06f + step };
                    let (d0, flags, delta, same) = s.call(op, input, ccr)?;
                    records.push(json!({"system":system as u8,"slot":value,"step":step,"input_d0":input,"input_ccr":ccr,"d0":d0,"ccr":flags,"a0_same":same,"stack_delta":delta,"raw_task":TASK,"raw_link":s.m.read_long(TASK),"type":s.m.read_word(TASK+4),"count":s.m.read_word(TASK+10) as i16,"phase":s.m.read_word(TASK+12) as i16,"proc_same":(s.m.read_long(TASK+6)==PROC) as u8,"guards":[s.m.read_long(TASK-4),s.m.read_long(TASK+34)]}));
                }
            }
        }
    }
    let mut callbacks = Vec::new();
    for mode in 0..3 {
        for kind in 0..6 {
            let mut s = Scene::new(slot)?;
            let system = mode == 0;
            let queue_slot = if mode == 2 { slot } else { 0 };
            let phase = if kind == 1 || kind == 4 { 2 } else { 0 };
            s.task(
                if kind == 3 {
                    0
                } else if kind == 5 {
                    -2
                } else {
                    3
                },
                phase,
                if kind == 2 || kind == 4 { 3 } else { 1 },
            );
            let install = s.call(if system { 0xa033 } else { 0xa06f }, queue_slot as u32, 16)?;
            for tick in 1..=30 {
                s.automatic(tick)?;
            }
            let mut row = s.snapshot();
            let map = row.as_object_mut().unwrap();
            map.insert("system".into(), json!(system as u8));
            map.insert("slot".into(), json!(queue_slot));
            map.insert("kind".into(), json!(kind));
            map.insert("install_d0".into(), json!(install.0));
            map.insert("install_result".into(), json!(install.0 as i16));
            map.insert("install_ccr".into(), json!(install.1));
            map.insert("ticks".into(), s.ticks(0, 30));
            let remove = s.call(if system { 0xa034 } else { 0xa070 }, queue_slot as u32, 16)?;
            let map = row.as_object_mut().unwrap();
            map.insert("remove_d0".into(), json!(remove.0));
            map.insert("remove_result".into(), json!(remove.0 as i16));
            map.insert("remove_ccr".into(), json!(remove.1));
            callbacks.push(row);
        }
    }
    let mut manual = Vec::new();
    for kind in 0..6 {
        let mut s = Scene::new(slot)?;
        s.task(
            if kind == 3 {
                0
            } else if kind == 5 {
                -2
            } else {
                3
            },
            if kind == 1 || kind == 4 { 2 } else { 0 },
            if kind == 2 || kind == 4 { 3 } else { 1 },
        );
        let install = s.call(0xa06f, 0, 16)?;
        s.m.write_long(0x16a, 100);
        let mut outputs = Vec::new();
        let mut flags = Vec::new();
        for pulse in 1..=12 {
            s.m.write_long(TASK + 50, pulse);
            let r = s.call(0xa072, 0, 16)?;
            outputs.push(r.0);
            flags.push(r.1);
        }
        let mut row = s.snapshot();
        let ticks = s.ticks(100, 100);
        let samples = json!([
            s.m.read_long(TASK + 54),
            s.m.read_long(TASK + 58),
            s.m.read_long(TASK + 62)
        ]);
        let remove = s.call(0xa070, 0, 16)?;
        let map = row.as_object_mut().unwrap();
        for (key, value) in [
            ("kind", json!(kind)),
            ("install_d0", json!(install.0)),
            ("install_ccr", json!(install.1)),
            ("ticks", ticks),
            ("pulse_samples", samples),
            ("pulse_d0", json!(outputs)),
            ("pulse_ccr", json!(flags)),
            ("remove_d0", json!(remove.0)),
            ("remove_ccr", json!(remove.1)),
        ] {
            map.insert(key.into(), value);
        }
        manual.push(row);
    }
    Ok(json!({"schema":1,"probe":"vbl-runtime-raw-v1","configured_slot":slot,"records":records,"callbacks":callbacks,"manual":manual}).to_string())
}
