//! Guest Sound Manager adapter backed by the shared renderer and actual sink.
use crate::{
    audio_output::Output,
    callbacks::{Arg, Callbacks},
};
use classic_core::{
    audio::{self, Command, Engine, Event, Queued, Sample, StandardHeader},
    Heap,
};
use m68k::{AddressBus, CpuCore};
use std::{
    collections::{BTreeMap, VecDeque},
    io::Write,
    path::PathBuf,
};

struct Channel {
    owned: bool,
    callback: u32,
    init: u32,
}
struct Pending {
    channel: u32,
    dispose: bool,
}
pub struct Sound {
    output: Option<Output>,
    channels: BTreeMap<u32, Channel>,
    pending: BTreeMap<(u32, u32), Pending>,
    callbacks: VecDeque<(u32, Command)>,
    callback_running: bool,
    trace: bool,
    log: Option<std::io::BufWriter<std::fs::File>>,
    inputs: Option<PathBuf>,
    next: u64,
    limit: u32,
    live: bool,
    origin: Option<u64>,
}
fn error(e: audio::Error) -> i16 {
    match e {
        audio::Error::QueueFull => -203,
        audio::Error::BadChannel => -205,
        audio::Error::UnsupportedEncoding | audio::Error::UnsupportedResource => -206,
        _ => -50,
    }
}
fn returned<B: AddressBus>(cpu: &mut CpuCore, m: &mut B, args: u32, error: i16, allocation: bool) {
    m.write_word(cpu.dar[15] + args, error as u16);
    cpu.dar[15] += args;
    cpu.pc += 2;
    cpu.set_ccr(if error == 0 {
        4
    } else {
        8 | if !allocation && cpu.flag_x() { 16 } else { 0 }
    });
}
impl Sound {
    pub fn new(args: &[String], limit: u32) -> Result<Self, String> {
        let value = |flag: &str| -> Result<Option<String>, String> {
            args.iter()
                .position(|s| s == flag)
                .map(|i| {
                    args.get(i + 1)
                        .cloned()
                        .ok_or(format!("missing {flag} value"))
                })
                .transpose()
        };
        let capture = value("--capture-audio")?.map(PathBuf::from);
        let mode = value("--audio-output")?.unwrap_or(
            if cfg!(target_os = "macos") && args.iter().any(|s| s == "--gui") {
                "coreaudio"
            } else if capture.is_some() {
                "headless"
            } else {
                "none"
            }
            .into(),
        );
        let live = mode == "coreaudio";
        let output = match mode.as_str() {
            "none" => {
                if capture.is_some() {
                    return Err("capture requires an audio sink".into());
                }
                None
            }
            "headless" | "coreaudio" => Some(Output::new(
                Engine::new(48000).map_err(|e| format!("{e:?}"))?,
                capture,
                live,
                None,
            )?),
            _ => return Err("--audio-output must be none,headless or coreaudio".into()),
        };
        let log = value("--sound-log")?
            .map(|p| {
                std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(p)
                    .map(std::io::BufWriter::new)
                    .map_err(|e| e.to_string())
            })
            .transpose()?;
        let inputs = value("--capture-audio-inputs")?.map(PathBuf::from);
        if let Some(p) = &inputs {
            std::fs::create_dir(p).map_err(|e| e.to_string())?;
        }
        Ok(Self {
            output,
            channels: BTreeMap::new(),
            pending: BTreeMap::new(),
            callbacks: VecDeque::new(),
            callback_running: false,
            trace: args.iter().any(|s| s == "--trace-sound"),
            log,
            inputs,
            next: 0,
            limit,
            live,
            origin: None,
        })
    }
    pub fn active(&self) -> bool {
        self.output.is_some()
    }
    /// Read-only renderer clock for correlating UI observations with PCM.
    #[cfg(feature = "gui")]
    pub fn output_frame(&self) -> Option<u64> {
        Some(self.output.as_ref()?.state.lock().ok()?.engine.frame())
    }
    pub fn initialize_guest<B: AddressBus>(&self, m: &mut B) {
        if self.active() {
            // Legacy system-beep state, distinct from default output gain.
            // SoundControlProbe-v1 measured these low-memory values on OS 9
            // with beep gain 0x00600060. They must not remain implicit mute
            // merely because guest RAM starts zeroed.
            m.write_byte(0x0260, 3); // SdVolume
            m.write_byte(0x027f, 255); // SoundLevel
        }
    }
    fn engine<T>(&self, f: impl FnOnce(&mut Engine) -> Result<T, audio::Error>) -> Result<T, i16> {
        let mut state = self
            .output
            .as_ref()
            .ok_or(-200i16)?
            .state
            .lock()
            .map_err(|_| -200i16)?;
        if state.failure().is_some() {
            return Err(-200);
        }
        f(&mut state.engine).map_err(error)
    }
    fn record(&mut self, value: serde_json::Value) -> Result<(), String> {
        if self.trace {
            println!("sound: {value}");
        }
        if let Some(log) = &mut self.log {
            writeln!(log, "{value}")
                .and_then(|_| log.flush())
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    fn valid(&self, p: u32, n: u32) -> bool {
        p >= 0x100 && p.checked_add(n).is_some_and(|end| end <= self.limit)
    }
    fn read<B: AddressBus>(&self, m: &mut B, p: u32, n: u32) -> Result<Vec<u8>, i16> {
        if !self.valid(p, n) {
            return Err(-50);
        }
        Ok((0..n).map(|i| m.read_byte(p + i)).collect())
    }
    fn sample<B: AddressBus>(&mut self, m: &mut B, p: u32) -> Result<(Sample, u64), i16> {
        if p & 1 != 0 {
            return Err(-50);
        }
        let bytes = self.read(m, p, 22)?;
        let header = StandardHeader::parse(&bytes).map_err(error)?;
        let source = if header.sample_pointer == 0 {
            p.checked_add(22).ok_or(-50i16)?
        } else {
            header.sample_pointer
        };
        let data = self.read(m, source, header.frames)?;
        let sample = Sample::decode(header, &data).map_err(error)?;
        self.next += 1;
        let token = self.next;
        let file = if let Some(dir) = &self.inputs {
            let path = dir.join(format!("buffer-{token:06}.raw"));
            std::fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&path)
                .and_then(|mut f| f.write_all(&data))
                .map_err(|_| -36i16)?;
            Some(path)
        } else {
            None
        };
        self.record(serde_json::json!({"event":"input","token":token,"header":p,"data":source,"frames":header.frames,
            "rate_fixed":header.rate_fixed,"base_frequency":header.base_frequency,"loop_start":header.loop_start,"loop_end":header.loop_end,"input_file":file})).map_err(|_|-36i16)?;
        Ok((sample, token))
    }
    fn allocate<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        p: u32,
        synth: i16,
        init: u32,
        callback: u32,
    ) -> Result<u32, i16> {
        if !self.valid(p, 4) || p & 1 != 0 {
            return Err(-50);
        }
        if synth == -1 {
            return Err(-2004);
        }
        if synth != 5 {
            return Err(-206);
        }
        if callback != 0 && (!self.valid(callback, 2) || callback & 1 != 0) {
            return Err(-50);
        }
        // Mono standard PCM and the no-interpolation/rate hints only.
        if init & !(0x80 | 0x30 | 0x04) != 0 {
            return Err(-206);
        }
        let supplied = m.read_long(p);
        let owned = supplied == 0;
        let capacity = if owned {
            128
        } else {
            if !self.valid(supplied, 36) || supplied & 1 != 0 {
                return Err(-50);
            }
            let n = m.read_word(supplied + 30);
            if n == 0 || n > 32767 || !self.valid(supplied, 36 + u32::from(n) * 8) {
                return Err(-50);
            }
            usize::from(n)
        };
        let channel = if owned {
            heap.new_ptr(m, 36 + capacity as u32 * 8, true)?
        } else {
            supplied
        };
        if let Err(e) = self.engine(|e| e.create_channel(channel, capacity, init & 4 == 0)) {
            if owned {
                let _ = heap.dispose_ptr(channel);
            }
            return Err(e);
        }
        m.write_long(channel, 0);
        m.write_long(channel + 4, 0);
        m.write_long(channel + 8, callback);
        m.write_long(channel + 16, 0);
        m.write_word(channel + 28, 0);
        m.write_word(channel + 30, capacity as u16);
        m.write_word(channel + 32, 0xffff);
        m.write_word(channel + 34, 0);
        m.write_long(p, channel);
        self.channels.insert(
            channel,
            Channel {
                owned,
                callback,
                init,
            },
        );
        Ok(channel)
    }
    fn dispose(&mut self, heap: &mut Heap, channel: u32) -> Result<(), i16> {
        let owned = self.channels.get(&channel).ok_or(-205i16)?.owned;
        self.engine(|e| e.remove_channel(channel))?;
        self.channels.remove(&channel);
        self.callbacks.retain(|(c, _)| *c != channel);
        if owned {
            heap.dispose_ptr(channel)?;
        }
        Ok(())
    }
    fn command<B: AddressBus>(
        &mut self,
        m: &mut B,
        channel: u32,
        command: Command,
        queued: bool,
    ) -> Result<(), i16> {
        if !self.channels.contains_key(&channel) {
            return Err(-205);
        }
        let Command {
            code,
            param1,
            param2,
        } = command;
        match code {
            0 => Ok(()),
            3 => self.engine(|e| e.quiet(channel)),
            4 => self.engine(|e| e.flush(channel)),
            11 => self.engine(|e| e.pause(channel, true)),
            12 => self.engine(|e| e.pause(channel, false)),
            13 => {
                if queued {
                    self.engine(|e| e.enqueue(channel, Queued::Callback(command)))
                } else {
                    self.callbacks.push_front((channel, command));
                    Ok(())
                }
            }
            43 => {
                let amplitude = u8::try_from(param1).map_err(|_| -50i16)?;
                if queued {
                    self.engine(|e| e.enqueue(channel, Queued::Amplitude(amplitude)))
                } else {
                    self.engine(|e| e.set_amplitude(channel, amplitude))
                }
            }
            45 => {
                if !self.valid(param2, 2) || param2 & 1 != 0 {
                    return Err(-50);
                }
                let amplitude = self.engine(|e| e.amplitude(channel))?;
                m.write_word(param2, u16::from(amplitude));
                Ok(())
            }
            46 => {
                if queued {
                    self.engine(|e| e.enqueue(channel, Queued::Volume(param2)))
                } else {
                    self.engine(|e| e.set_volume(channel, param2))
                }
            }
            47 => {
                if !self.valid(param2, 4) {
                    return Err(-50);
                }
                let v = self.engine(|e| e.volume(channel))?;
                m.write_long(param2, v);
                Ok(())
            }
            81 => {
                let (sample, token) = self.sample(m, param2)?;
                if !queued {
                    self.engine(|e| e.quiet(channel))?;
                }
                self.engine(|e| e.enqueue(channel, Queued::Buffer { sample, token }))
            }
            // Rate/note/instrument commands await native observation and real
            // application traces. A successful no-op here would be misleading.
            _ => Err(-50),
        }
    }
    pub fn callback_result(&mut self) {
        self.callback_running = false;
    }
    pub fn service<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        now: u64,
        callbacks: &mut Callbacks,
    ) -> Result<bool, String> {
        if let Some(output) = &mut self.output {
            if let Some(error) = output
                .state
                .lock()
                .map_err(|_| "audio state poisoned")?
                .failure()
            {
                return Err(error);
            }
            if !self.live {
                let origin = *self.origin.get_or_insert(now);
                let target = (now - origin).saturating_mul(48000) / 1_000_000;
                let frame = output
                    .state
                    .lock()
                    .map_err(|_| "audio state poisoned")?
                    .engine
                    .frame();
                if target > frame {
                    let n = target - frame;
                    if n > 48000 * 60 {
                        return Err("audio clock advanced more than60s between services".into());
                    }
                    output.render_headless(n as usize, 480)?;
                }
            }
            let mut events = Vec::new();
            {
                let mut state = output.state.lock().map_err(|_| "audio state poisoned")?;
                while let Some(event) = state.engine.pop_event() {
                    events.push(event);
                }
            }
            for event in events {
                self.record(crate::audio_output::event_json(event))?;
                if let Event::Callback {
                    channel, command, ..
                } = event
                {
                    self.callbacks.push_back((channel, command));
                }
            }
        }
        if !self.callback_running {
            while let Some((channel, command)) = self.callbacks.pop_front() {
                let Some(meta) = self.channels.get(&channel) else {
                    continue;
                };
                let target = meta.callback;
                if target == 0 {
                    continue;
                }
                let pointer = channel + 20;
                m.write_word(pointer, command.code);
                m.write_word(pointer + 2, command.param1 as u16);
                m.write_long(pointer + 4, command.param2);
                callbacks.call_void(
                    cpu,
                    m,
                    target,
                    &[Arg::Long(channel), Arg::Long(pointer)],
                    11,
                )?;
                self.callback_running = true;
                return Ok(true);
            }
        }
        Ok(false)
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        opcode: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<bool, String> {
        if !self.active() {
            return Ok(crate::sound::dispatch(opcode, cpu, m));
        }
        let sp = cpu.dar[15];
        let key = (cpu.pc, sp);
        if let Some(wait) = self.pending.get(&key) {
            let busy = self.engine(|e| e.busy(wait.channel));
            if busy == Ok(true) {
                std::thread::sleep(std::time::Duration::from_millis(1));
                return Ok(true);
            }
            let wait = self.pending.remove(&key).unwrap();
            let result = if let Err(error) = busy {
                Err(error)
            } else if wait.dispose {
                self.dispose(heap, wait.channel)
            } else {
                Ok(())
            };
            returned(
                cpu,
                m,
                if wait.dispose { 6 } else { 10 },
                result.err().unwrap_or(0),
                false,
            );
            return Ok(true);
        }
        let (args, result, allocation) = match opcode {
            0xa807 => {
                let callback = m.read_long(sp);
                let init = m.read_long(sp + 4);
                let synth = m.read_word(sp + 8) as i16;
                let pointer = m.read_long(sp + 10);
                let result = self.allocate(m, heap, pointer, synth, init, callback);
                self.record(serde_json::json!({"event":"new_channel","synth":synth,"init":init,"callback":callback,"channel":result.as_ref().ok(),"error":result.as_ref().err()}))?;
                (14, result.map(|_| ()), true)
            }
            0xa801 => {
                let quit = m.read_word(sp) != 0;
                let channel = m.read_long(sp + 2);
                if !quit && self.engine(|e| e.busy(channel)).unwrap_or(false) {
                    self.pending.insert(
                        key,
                        Pending {
                            channel,
                            dispose: true,
                        },
                    );
                    return Ok(true);
                }
                (6, self.dispose(heap, channel), false)
            }
            0xa803 | 0xa804 => {
                let (args, p, channel) = if opcode == 0xa803 {
                    (10, m.read_long(sp + 2), m.read_long(sp + 6))
                } else {
                    (8, m.read_long(sp), m.read_long(sp + 4))
                };
                let result = if self.valid(p, 8) && p & 1 == 0 {
                    let command = Command {
                        code: m.read_word(p),
                        param1: m.read_word(p + 2) as i16,
                        param2: m.read_long(p + 4),
                    };
                    self.record(serde_json::json!({"event":"command","channel":channel,"queued":opcode==0xa803,"code":command.code,"param1":command.param1,"param2":command.param2}))?;
                    self.command(m, channel, command, opcode == 0xa803)
                } else {
                    Err(-50)
                };
                (args, result, false)
            }
            0xa805 => {
                let asynchronous = m.read_word(sp) != 0;
                let handle = m.read_long(sp + 2);
                let channel = m.read_long(sp + 6);
                let result = (|| -> Result<(), i16> {
                    if !self.channels.contains_key(&channel) {
                        return Err(-205);
                    }
                    let size = heap.size(handle)?;
                    let pointer = m.read_long(handle);
                    let bytes = self.read(m, pointer, size)?;
                    let resource = audio::Resource::parse(&bytes).map_err(error)?;
                    for i in 0..resource.command_count {
                        let mut command = resource.command(i).ok_or(-206i16)?;
                        if command.code & 0x8000 != 0 {
                            command.code &= 0x7fff;
                            command.param2 = pointer.checked_add(command.param2).ok_or(-50i16)?;
                        }
                        self.command(m, channel, command, true)?;
                    }
                    Ok(())
                })();
                if result.is_ok()
                    && !asynchronous
                    && self.engine(|e| e.busy(channel)).unwrap_or(false)
                {
                    self.pending.insert(
                        key,
                        Pending {
                            channel,
                            dispose: false,
                        },
                    );
                    return Ok(true);
                }
                (10, result, false)
            }
            0xa800 => match cpu.dar[0] {
                0x022c0018 => {
                    let pointer = m.read_long(sp);
                    let result = if self.valid(pointer, 4) {
                        self.engine(|e| Ok(e.master_volume()))
                            .map(|volume| m.write_long(pointer, volume))
                    } else {
                        Err(-50)
                    };
                    (4, result, false)
                }
                0x02300018 => {
                    let level = m.read_long(sp);
                    (
                        4,
                        self.engine(|e| {
                            e.set_master_volume(level);
                            Ok(())
                        }),
                        false,
                    )
                }
                0x05100008 => {
                    let pointer = m.read_long(sp);
                    let length = m.read_word(sp + 4);
                    let channel = m.read_long(sp + 6);
                    let result = (|| -> Result<(), i16> {
                        if length < 24 || !self.valid(pointer, 24) {
                            return Err(-50);
                        }
                        let busy = self.engine(|e| e.busy(channel))?;
                        let paused = self.engine(|e| e.paused(channel))?;
                        let init = self.channels.get(&channel).ok_or(-205i16)?.init;
                        for i in 0..24 {
                            m.write_byte(pointer + i, 0);
                        }
                        m.write_byte(pointer + 12, u8::from(busy));
                        m.write_byte(pointer + 14, u8::from(paused));
                        m.write_long(pointer + 16, init);
                        Ok(())
                    })();
                    (10, result, false)
                }
                _ => return Ok(false),
            },
            _ => return Ok(false),
        };
        let error = result.err().unwrap_or(0);
        self.record(serde_json::json!({"event":"api_result","trap":opcode,"pc":cpu.pc,"error":error}))?;
        returned(cpu, m, args, error, allocation);
        Ok(true)
    }
}
impl Drop for Sound {
    fn drop(&mut self) {
        if let Some(output) = self.output.take() {
            match output.finish() {
                Ok(report) => {
                    if self.trace {
                        println!("audio sink: {report}");
                    }
                }
                Err(e) => eprintln!("audio sink finalization failed: {e}"),
            }
        }
        if let Some(log) = &mut self.log {
            let _ = log.flush();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Memory;

    #[test]
    fn backed_sound_initializes_legacy_volume_without_changing_output_gain() {
        let args = vec!["runtime".into(), "--audio-output".into(), "headless".into()];
        let sound = Sound::new(&args, 4096).unwrap();
        let mut memory = Memory::zeroed(4096);
        memory.write_byte(0x025f, 0xa5);
        memory.write_byte(0x0261, 0x5a);
        sound.initialize_guest(&mut memory);
        assert_eq!(memory.read_byte(0x0260), 3);
        assert_eq!(memory.read_byte(0x027f), 255);
        assert_eq!(memory.read_byte(0x025f), 0xa5);
        assert_eq!(memory.read_byte(0x0261), 0x5a);
        assert_eq!(sound.engine(|e| Ok(e.master_volume())), Ok(0x01000100));
    }

    #[test]
    fn absent_sink_does_not_advertise_a_legacy_volume() {
        let sound = Sound::new(&["runtime".into()], 4096).unwrap();
        let mut memory = Memory::zeroed(4096);
        sound.initialize_guest(&mut memory);
        assert_eq!(memory.read_byte(0x0260), 0);
        assert_eq!(memory.read_byte(0x027f), 0);
    }

    #[test]
    fn amplitude_traps_write_a_word_preserve_guards_and_reject_bad_ranges() {
        let args = vec!["runtime".into(), "--audio-output".into(), "headless".into()];
        let mut sound = Sound::new(&args, 16384).unwrap();
        let mut memory = Memory::zeroed(16384);
        let mut heap = Heap::new(4096, 12000);
        memory.write_long(0x300, 0);
        let channel = sound.allocate(&mut memory, &mut heap, 0x300, 5, 128, 0).unwrap();
        let mut cpu = CpuCore::new();
        for amplitude in [0, 1, 127, 255] {
            cpu.dar[15] = 0x800;
            cpu.pc = 0x200;
            memory.write_long(0x800, 0x400);
            memory.write_long(0x804, channel);
            memory.write_word(0x400, 43);
            memory.write_word(0x402, amplitude);
            memory.write_long(0x404, 0);
            assert!(sound.dispatch(0xa804, &mut cpu, &mut memory, &mut heap).unwrap());
            assert_eq!(memory.read_word(0x808), 0);
            assert_eq!(cpu.dar[15], 0x808);
            memory.write_long(0x500, 0x55555555);
            memory.write_word(0x400, 45);
            memory.write_long(0x404, 0x500);
            cpu.dar[15] = 0x800;
            assert!(sound.dispatch(0xa804, &mut cpu, &mut memory, &mut heap).unwrap());
            assert_eq!(memory.read_word(0x500), amplitude);
            assert_eq!(memory.read_word(0x502), 0x5555);
            assert_eq!(sound.engine(|e| e.volume(channel)), Ok(0x01000100));
        }
        for amplitude in [256, 0xffff] {
            memory.write_word(0x400, 43);
            memory.write_word(0x402, amplitude);
            cpu.dar[15] = 0x800;
            sound.dispatch(0xa804, &mut cpu, &mut memory, &mut heap).unwrap();
            assert_eq!(memory.read_word(0x808) as i16, -50);
            assert_eq!(sound.engine(|e| e.amplitude(channel)), Ok(255));
        }
    }
}
