//! Output adapters share the core renderer. Captured bytes are the exact frames
//! written into the device buffer, not a separately generated waveform.
use classic_core::audio::{Command, Engine, Event, Queued, Sample, StandardHeader};
use std::{
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

pub struct State {
    pub engine: Engine,
    captured: Option<Vec<[i16; 2]>>,
    pub rendered: u64,
    submitted: u64,
    consumed: u64,
    device_callbacks: u64,
    errors: Vec<i32>,
    overflow: bool,
    limit: Option<u64>,
    stopping: bool,
}
impl State {
    pub fn failure(&self) -> Option<String> {
        if self.overflow || !self.errors.is_empty() {
            Some(format!(
                "audio capture/device failure: overflow={} errors={:?}",
                self.overflow, self.errors
            ))
        } else {
            None
        }
    }
    fn render(&mut self, frames: &mut [[i16; 2]]) {
        self.engine.render_into(frames);
        self.rendered += frames.len() as u64;
        if let Some(capture) = &mut self.captured {
            if capture.len() + frames.len() <= 48000 * 60 * 20 {
                capture.extend_from_slice(frames);
            } else {
                self.overflow = true;
            }
        }
    }
}
pub struct Output {
    pub state: Arc<Mutex<State>>,
    capture: Option<PathBuf>,
    live: bool,
    #[cfg(target_os = "macos")]
    native: Option<mac::Queue>,
}
impl Output {
    pub fn new(
        engine: Engine,
        capture: Option<PathBuf>,
        live: bool,
        limit: Option<u64>,
    ) -> Result<Self, String> {
        if let Some(path) = &capture {
            if path.exists() || path.with_extension("json").exists() {
                return Err("audio capture and report must be new files".into());
            }
        }
        let capacity = limit.unwrap_or(48000 * 60).min(48000 * 60) as usize;
        let state = Arc::new(Mutex::new(State {
            engine,
            captured: capture.as_ref().map(|_| Vec::with_capacity(capacity)),
            rendered: 0,
            submitted: 0,
            consumed: 0,
            device_callbacks: 0,
            errors: Vec::new(),
            overflow: false,
            limit,
            stopping: false,
        }));
        #[cfg(not(target_os = "macos"))]
        if live {
            return Err("Live output is currently implemented only for macOS".into());
        }
        #[cfg(target_os = "macos")]
        let native = if live {
            Some(mac::Queue::new(state.clone())?)
        } else {
            None
        };
        Ok(Self {
            state,
            capture,
            live,
            #[cfg(target_os = "macos")]
            native,
        })
    }
    pub fn render_headless(&mut self, frames: usize, block: usize) -> Result<(), String> {
        if self.live || block == 0 {
            return Err("invalid headless render mode/block".into());
        }
        let mut buffer = vec![[0i16; 2]; block];
        let mut remaining = frames;
        while remaining > 0 {
            let n = remaining.min(block);
            self.state
                .lock()
                .map_err(|_| "audio state poisoned")?
                .render(&mut buffer[..n]);
            remaining -= n;
        }
        Ok(())
    }
    pub fn wait_consumed(&self, frames: u64) -> Result<(), String> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(15);
        loop {
            let state = self.state.lock().map_err(|_| "audio state poisoned")?;
            if !state.errors.is_empty() {
                return Err(format!("device callback errors: {:?}", state.errors));
            }
            if state.consumed >= frames {
                return Ok(());
            }
            drop(state);
            if std::time::Instant::now() >= deadline {
                return Err("device did not consume the finite test buffer within15s".into());
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
    pub fn finish(mut self) -> Result<serde_json::Value, String> {
        let drain_error = if self.live {
            let target = {
                let mut state = self.state.lock().map_err(|_| "audio state poisoned")?;
                let target = state.rendered;
                state.limit = Some(target);
                target
            };
            self.wait_consumed(target).err()
        } else {
            None
        };
        #[cfg(target_os = "macos")]
        {
            self.native.take();
        }
        let state = self.state.lock().map_err(|_| "audio state poisoned")?;
        let nonzero = state
            .captured
            .as_ref()
            .map(|v| v.iter().flat_map(|s| s.iter()).filter(|&&x| x != 0).count());
        let peak = state.captured.as_ref().map(|v| {
            v.iter()
                .flat_map(|s| s.iter())
                .map(|&x| i32::from(x).abs())
                .max()
                .unwrap_or(0)
        });
        let report = serde_json::json!({"schema":1,"sink":if self.live {"coreaudio-audioqueue"}else{"headless"},
            "sample_rate":state.engine.rate(),"channels":2,"bits":16,"rendered_frames":state.rendered,
            "submitted_frames":state.submitted,"consumed_frames":state.consumed,"device_callbacks":state.device_callbacks,
            "device_errors":state.errors,"drain_error":drain_error,"capture_overflow":state.overflow,"nonzero_samples":nonzero,"peak":peak,
            "capture_scope":"Exact interleaved signed16 frames rendered into the sink buffer before any host-device sample-rate conversion or hardware volume. Not a microphone/system-loopback recording.",
            "completion_clock":"Core output frame boundary. Hardware output latency is additional and is not measured by this capture."});
        if let Some(path) = &self.capture {
            write_wav(path, state.engine.rate(), state.captured.as_ref().unwrap())?;
            write_new(
                &path.with_extension("json"),
                &serde_json::to_vec_pretty(&report).unwrap(),
            )?;
        }
        if state.overflow || !state.errors.is_empty() || drain_error.is_some() {
            return Err(format!("audio output incomplete: {report}"));
        }
        Ok(report)
    }
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .and_then(|mut file| file.write_all(bytes))
        .map_err(|e| e.to_string())
}
fn write_wav(path: &Path, rate: u32, frames: &[[i16; 2]]) -> Result<(), String> {
    let length = u32::try_from(frames.len() * 4).map_err(|_| "WAV too large")?;
    let file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    let mut header = Vec::new();
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(length + 36).to_le_bytes());
    header.extend_from_slice(b"WAVEfmt ");
    for n in [16u32] {
        header.extend_from_slice(&n.to_le_bytes());
    }
    header.extend_from_slice(&1u16.to_le_bytes());
    header.extend_from_slice(&2u16.to_le_bytes());
    header.extend_from_slice(&rate.to_le_bytes());
    header.extend_from_slice(&(rate * 4).to_le_bytes());
    header.extend_from_slice(&4u16.to_le_bytes());
    header.extend_from_slice(&16u16.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&length.to_le_bytes());
    out.write_all(&header).map_err(|e| e.to_string())?;
    for frame in frames {
        for sample in frame {
            out.write_all(&sample.to_le_bytes())
                .map_err(|e| e.to_string())?;
        }
    }
    out.flush().map_err(|e| e.to_string())
}
pub fn event_json(event: Event) -> serde_json::Value {
    match event {
        Event::Started {
            channel,
            token,
            frame,
        } => serde_json::json!({"event":"started","channel":channel,"token":token,"frame":frame}),
        Event::Finished {
            channel,
            token,
            frame,
        } => serde_json::json!({"event":"finished","channel":channel,"token":token,"frame":frame}),
        Event::Cancelled {
            channel,
            token,
            frame,
        } => serde_json::json!({"event":"cancelled","channel":channel,"token":token,"frame":frame}),
        Event::Callback {
            channel,
            command,
            frame,
        } => {
            serde_json::json!({"event":"callback","channel":channel,"frame":frame,"command":command.code,"param1":command.param1,"param2":command.param2})
        }
    }
}
/// Independent input file, standard-header decoder, real shared renderer.
/// This entry point is a harness, not evidence that guest/game APIs work.
pub fn vector(args: &[String]) -> Result<(), String> {
    let data = std::fs::read(
        args.get(2)
            .ok_or("usage: --audio-vector INPUT_U8_RAW OUTPUT_WAV [--live]")?,
    )
    .map_err(|e| e.to_string())?;
    let path = PathBuf::from(args.get(3).ok_or("missing output WAV")?);
    if path.with_extension("events.json").exists() {
        return Err("audio event report already exists".into());
    }
    let mut header = [0u8; 22];
    header[4..8].copy_from_slice(
        &u32::try_from(data.len())
            .map_err(|_| "input exceeds standard header frame count")?
            .to_be_bytes(),
    );
    header[8..12].copy_from_slice(&0xbb800000u32.to_be_bytes());
    header[21] = 60;
    let sample = Sample::decode(
        StandardHeader::parse(&header).map_err(|e| format!("{e:?}"))?,
        &data,
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut engine = Engine::new(48000).map_err(|e| format!("{e:?}"))?;
    engine
        .create_channel(1, 128, true)
        .map_err(|e| format!("{e:?}"))?;
    engine
        .enqueue(1, Queued::Buffer { sample, token: 1 })
        .map_err(|e| format!("{e:?}"))?;
    engine
        .enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 123,
                param2: 0x12345678,
            }),
        )
        .map_err(|e| format!("{e:?}"))?;
    let frames = data.len() as u64 + 9600;
    let live = args.iter().any(|x| x == "--live");
    let mut output = Output::new(engine, Some(path.clone()), live, Some(frames))?;
    let result = if live {
        output.wait_consumed(frames)
    } else {
        output.render_headless(frames as usize, 143)
    };
    let events = {
        let mut state = output.state.lock().map_err(|_| "audio state poisoned")?;
        let mut events = Vec::new();
        while let Some(e) = state.engine.pop_event() {
            events.push(event_json(e))
        }
        events
    };
    // Preserve buffers and diagnostics even if device consumption failed.
    let report = output.finish()?;
    write_new(
        &path.with_extension("events.json"),
        &serde_json::to_vec_pretty(&events).unwrap(),
    )?;
    println!("{report}");
    result
}

#[cfg(target_os = "macos")]
mod mac {
    use super::*;
    use std::ffi::c_void;
    type QueueRef = *mut c_void;
    #[repr(C)]
    struct Format {
        rate: f64,
        format: u32,
        flags: u32,
        bytes_packet: u32,
        frames_packet: u32,
        bytes_frame: u32,
        channels: u32,
        bits: u32,
        reserved: u32,
    }
    #[repr(C)]
    struct Buffer {
        capacity: u32,
        data: *mut c_void,
        size: u32,
        user: *mut c_void,
        packet_capacity: u32,
        packets: *mut c_void,
        packet_count: u32,
    }
    #[link(name = "AudioToolbox", kind = "framework")]
    unsafe extern "C" {
        fn AudioQueueNewOutput(
            format: *const Format,
            callback: unsafe extern "C" fn(*mut c_void, QueueRef, *mut Buffer),
            user: *mut c_void,
            runloop: *mut c_void,
            mode: *mut c_void,
            flags: u32,
            out: *mut QueueRef,
        ) -> i32;
        fn AudioQueueAllocateBuffer(queue: QueueRef, bytes: u32, out: *mut *mut Buffer) -> i32;
        fn AudioQueueEnqueueBuffer(
            queue: QueueRef,
            buffer: *mut Buffer,
            packets: u32,
            descriptions: *const c_void,
        ) -> i32;
        fn AudioQueueStart(queue: QueueRef, time: *const c_void) -> i32;
        fn AudioQueueStop(queue: QueueRef, immediate: u8) -> i32;
        fn AudioQueueDispose(queue: QueueRef, immediate: u8) -> i32;
    }
    pub struct Queue {
        queue: QueueRef,
        user: *mut Arc<Mutex<State>>,
    }
    impl Queue {
        pub fn new(state: Arc<Mutex<State>>) -> Result<Self, String> {
            let rate = state
                .lock()
                .map_err(|_| "audio state poisoned")?
                .engine
                .rate();
            let format = Format {
                rate: f64::from(rate),
                format: u32::from_be_bytes(*b"lpcm"),
                flags: 12,
                bytes_packet: 4,
                frames_packet: 1,
                bytes_frame: 4,
                channels: 2,
                bits: 16,
                reserved: 0,
            };
            let user = Box::into_raw(Box::new(state));
            let mut queue = std::ptr::null_mut();
            let error = unsafe {
                AudioQueueNewOutput(
                    &format,
                    callback,
                    user.cast(),
                    std::ptr::null_mut(),
                    std::ptr::null_mut(),
                    0,
                    &mut queue,
                )
            };
            if error != 0 {
                unsafe {
                    drop(Box::from_raw(user));
                }
                return Err(format!("AudioQueueNewOutput: {error}"));
            }
            let result = Self { queue, user };
            for _ in 0..3 {
                let mut buffer = std::ptr::null_mut();
                let error = unsafe { AudioQueueAllocateBuffer(queue, 480 * 4, &mut buffer) };
                if error != 0 {
                    return Err(format!("AudioQueueAllocateBuffer: {error}"));
                }
                unsafe {
                    fill(user.cast(), queue, buffer, false);
                }
            }
            let error = unsafe { AudioQueueStart(queue, std::ptr::null()) };
            if error != 0 {
                return Err(format!("AudioQueueStart: {error}"));
            }
            Ok(result)
        }
    }
    unsafe extern "C" fn callback(user: *mut c_void, queue: QueueRef, buffer: *mut Buffer) {
        unsafe {
            fill(user, queue, buffer, true);
        }
    }
    unsafe fn fill(user: *mut c_void, queue: QueueRef, buffer: *mut Buffer, consumed: bool) {
        let shared = unsafe { &*user.cast::<Arc<Mutex<State>>>() };
        let Ok(mut state) = shared.lock() else {
            return;
        };
        if state.stopping {
            return;
        }
        if consumed {
            state.consumed += u64::from(unsafe { (*buffer).size }) / 4;
            state.device_callbacks += 1;
        }
        let remaining = state
            .limit
            .map(|n| n.saturating_sub(state.rendered))
            .unwrap_or(u64::MAX);
        let count = remaining.min(u64::from(unsafe { (*buffer).capacity }) / 4) as usize;
        if count == 0 {
            return;
        }
        let samples =
            unsafe { std::slice::from_raw_parts_mut((*buffer).data.cast::<[i16; 2]>(), count) };
        state.render(samples);
        unsafe {
            (*buffer).size = (count * 4) as u32;
        }
        let error = unsafe { AudioQueueEnqueueBuffer(queue, buffer, 0, std::ptr::null()) };
        if error != 0 {
            state.errors.push(error);
        } else {
            state.submitted += count as u64;
        }
    }
    impl Drop for Queue {
        fn drop(&mut self) {
            unsafe {
                if let Ok(mut state) = (*self.user).lock() {
                    state.stopping = true;
                }
                let stopped = AudioQueueStop(self.queue, 1);
                let disposed = AudioQueueDispose(self.queue, 1);
                if let Ok(mut state) = (*self.user).lock() {
                    if stopped != 0 {
                        state.errors.push(stopped);
                    }
                    if disposed != 0 {
                        state.errors.push(disposed);
                    }
                }
                // On disposal failure, preserve callback storage instead of making
                // a possible late system callback dereference freed user data.
                if disposed == 0 {
                    drop(Box::from_raw(self.user));
                }
            }
        }
    }
}
