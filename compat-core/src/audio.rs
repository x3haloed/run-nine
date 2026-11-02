//! Frame-clocked sampled sound. The host supplies frames, never audio semantics.
//!
//! Standard unsigned8 mono samples are decoded once. Channel queues, linear
//! interpolation, gain, wide accumulation, saturation and completion events all
//! live here so a headless sink and a hardware sink render identical frames.
//! This is an owned software output device, not a model of Apple's codec/EQ.
use alloc::{
    collections::{BTreeMap, VecDeque},
    vec::Vec,
};

/// A source playback ratio in unsigned16.16; guest API selectors must be
/// translated by their adapter, not inferred from this software-device model.
pub const UNITY: u32 = 0x0001_0000;
const PHASE_ONE: u64 = 1 << 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    TruncatedHeader,
    UnsupportedEncoding,
    TruncatedSamples,
    InvalidRate,
    BadChannel,
    QueueFull,
    InvalidQueueLength,
    UnsupportedResource,
    ExternalAddress,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StandardHeader {
    pub sample_pointer: u32,
    pub frames: u32,
    pub rate_fixed: u32,
    pub loop_start: u32,
    pub loop_end: u32,
    pub base_frequency: u8,
}
impl StandardHeader {
    pub fn parse(bytes: &[u8]) -> Result<Self, Error> {
        if bytes.len() < 22 {
            return Err(Error::TruncatedHeader);
        }
        if bytes[20] != 0 {
            return Err(Error::UnsupportedEncoding);
        }
        let long = |p| u32::from_be_bytes(bytes[p..p + 4].try_into().unwrap());
        let result = Self {
            sample_pointer: long(0),
            frames: long(4),
            rate_fixed: long(8),
            loop_start: long(12),
            loop_end: long(16),
            base_frequency: bytes[21],
        };
        if result.rate_fixed == 0 {
            return Err(Error::InvalidRate);
        }
        Ok(result)
    }
}

#[derive(Debug, Clone)]
pub struct Sample {
    pub header: StandardHeader,
    samples: Vec<i16>,
}
impl Sample {
    /// `data` is explicitly resolved by the caller (inline or external pointer).
    /// A buffer plays once; header loop markers are retained for instrument
    /// commands but do not turn a buffer command into an infinite loop.
    pub fn decode(header: StandardHeader, data: &[u8]) -> Result<Self, Error> {
        if header.rate_fixed == 0 {
            return Err(Error::InvalidRate);
        }
        let data = data
            .get(..header.frames as usize)
            .ok_or(Error::TruncatedSamples)?;
        Ok(Self {
            header,
            samples: data.iter().map(|&b| (i16::from(b) - 128) * 256).collect(),
        })
    }
    pub fn frames(&self) -> usize {
        self.samples.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Command {
    pub code: u16,
    pub param1: i16,
    pub param2: u32,
}

/// Checked resource structure, separate from guest address resolution.
pub struct Resource<'a> {
    bytes: &'a [u8],
    commands: usize,
    pub command_count: usize,
    pub modifier_count: usize,
}
impl<'a> Resource<'a> {
    pub fn parse(bytes: &'a [u8]) -> Result<Self, Error> {
        let word = |p: usize| {
            bytes
                .get(p..p + 2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]))
        };
        let (commands, modifier_count) = match word(0) {
            Some(1) => {
                let n = word(2).ok_or(Error::UnsupportedResource)? as usize;
                (4 + 6 * n, n)
            }
            Some(2) => (4, 0),
            _ => return Err(Error::UnsupportedResource),
        };
        let command_count = word(commands).ok_or(Error::UnsupportedResource)? as usize;
        if commands + 2 + 8 * command_count > bytes.len() {
            return Err(Error::UnsupportedResource);
        }
        Ok(Self {
            bytes,
            commands: commands + 2,
            command_count,
            modifier_count,
        })
    }
    pub fn command(&self, index: usize) -> Option<Command> {
        if index >= self.command_count {
            return None;
        }
        let b = &self.bytes[self.commands + index * 8..self.commands + (index + 1) * 8];
        Some(Command {
            code: u16::from_be_bytes([b[0], b[1]]),
            param1: i16::from_be_bytes([b[2], b[3]]),
            param2: u32::from_be_bytes(b[4..8].try_into().unwrap()),
        })
    }
    pub fn buffer(&self, command: Command) -> Result<Sample, Error> {
        if command.code != 0x8051 {
            return Err(Error::UnsupportedResource);
        }
        let bytes = self
            .bytes
            .get(command.param2 as usize..)
            .ok_or(Error::TruncatedHeader)?;
        let header = StandardHeader::parse(bytes)?;
        if header.sample_pointer != 0 {
            return Err(Error::ExternalAddress);
        }
        Sample::decode(header, &bytes[22..])
    }
}

#[derive(Debug, Clone)]
pub enum Queued {
    Buffer { sample: Sample, token: u64 },
    Callback(Command),
    Amplitude(u8),
    Volume(u32),
    PlaybackRatio(u32),
    WaitFrames(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Started {
        channel: u32,
        token: u64,
        frame: u64,
    },
    Finished {
        channel: u32,
        token: u64,
        frame: u64,
    },
    Cancelled {
        channel: u32,
        token: u64,
        frame: u64,
    },
    Callback {
        channel: u32,
        command: Command,
        frame: u64,
    },
}
struct Voice {
    sample: Sample,
    phase: u64,
    token: u64,
}
struct Channel {
    queue: VecDeque<Queued>,
    voice: Option<Voice>,
    wait: u64,
    capacity: usize,
    volume: u32,
    amplitude: u8,
    playback_ratio: u32,
    paused: bool,
    interpolate: bool,
}
impl Channel {
    fn prepare(&mut self, id: u32, frame: u64, events: &mut VecDeque<Event>) {
        if self.paused || self.voice.is_some() || self.wait > 0 {
            return;
        }
        while let Some(command) = self.queue.pop_front() {
            match command {
                Queued::Buffer { sample, token } => {
                    events.push_back(Event::Started {
                        channel: id,
                        token,
                        frame,
                    });
                    // A buffer command resets the channel rate to unity.
                    self.playback_ratio = UNITY;
                    if sample.samples.is_empty() {
                        events.push_back(Event::Finished {
                            channel: id,
                            token,
                            frame,
                        });
                    } else {
                        self.voice = Some(Voice {
                            sample,
                            phase: 0,
                            token,
                        });
                        return;
                    }
                }
                Queued::Callback(command) => events.push_back(Event::Callback {
                    channel: id,
                    command,
                    frame,
                }),
                Queued::Amplitude(amplitude) => self.amplitude = amplitude,
                Queued::Volume(volume) => self.volume = volume,
                Queued::PlaybackRatio(playback_ratio) => self.playback_ratio = playback_ratio,
                Queued::WaitFrames(frames) => {
                    self.wait = frames;
                    if frames > 0 {
                        return;
                    }
                }
            }
        }
    }
}
pub struct Engine {
    rate: u32,
    frame: u64,
    master_volume: u32,
    channels: BTreeMap<u32, Channel>,
    events: VecDeque<Event>,
}
impl Engine {
    pub fn new(rate: u32) -> Result<Self, Error> {
        if rate == 0 {
            return Err(Error::InvalidRate);
        }
        Ok(Self {
            rate,
            frame: 0,
            master_volume: 0x01000100,
            channels: BTreeMap::new(),
            events: VecDeque::new(),
        })
    }
    pub fn rate(&self) -> u32 {
        self.rate
    }
    pub fn frame(&self) -> u64 {
        self.frame
    }
    pub fn master_volume(&self) -> u32 {
        self.master_volume
    }
    pub fn set_master_volume(&mut self, volume: u32) {
        self.master_volume = volume;
    }
    pub fn create_channel(
        &mut self,
        id: u32,
        capacity: usize,
        interpolate: bool,
    ) -> Result<(), Error> {
        if capacity == 0 || capacity > 32767 {
            return Err(Error::InvalidQueueLength);
        }
        if self.channels.contains_key(&id) {
            return Err(Error::BadChannel);
        }
        self.channels.insert(
            id,
            Channel {
                queue: VecDeque::new(),
                voice: None,
                wait: 0,
                capacity,
                volume: 0x01000100,
                amplitude: 255,
                playback_ratio: UNITY,
                paused: false,
                interpolate,
            },
        );
        Ok(())
    }
    fn channel(&mut self, id: u32) -> Result<&mut Channel, Error> {
        self.channels.get_mut(&id).ok_or(Error::BadChannel)
    }
    pub fn enqueue(&mut self, id: u32, command: Queued) -> Result<(), Error> {
        let channel = self.channels.get_mut(&id).ok_or(Error::BadChannel)?;
        if channel.queue.len() >= channel.capacity {
            return Err(Error::QueueFull);
        }
        channel.queue.push_back(command);
        channel.prepare(id, self.frame, &mut self.events);
        Ok(())
    }
    pub fn busy(&self, id: u32) -> Result<bool, Error> {
        let c = self.channels.get(&id).ok_or(Error::BadChannel)?;
        Ok(c.voice.is_some() || c.wait > 0 || !c.queue.is_empty())
    }
    pub fn paused(&self, id: u32) -> Result<bool, Error> {
        Ok(self.channels.get(&id).ok_or(Error::BadChannel)?.paused)
    }
    pub fn volume(&self, id: u32) -> Result<u32, Error> {
        Ok(self.channels.get(&id).ok_or(Error::BadChannel)?.volume)
    }
    pub fn amplitude(&self, id: u32) -> Result<u8, Error> {
        Ok(self.channels.get(&id).ok_or(Error::BadChannel)?.amplitude)
    }
    pub fn set_amplitude(&mut self, id: u32, amplitude: u8) -> Result<(), Error> {
        self.channel(id)?.amplitude = amplitude;
        Ok(())
    }
    pub fn playback_ratio(&self, id: u32) -> Result<u32, Error> {
        Ok(self
            .channels
            .get(&id)
            .ok_or(Error::BadChannel)?
            .playback_ratio)
    }
    pub fn queued_commands(&self, id: u32) -> Result<usize, Error> {
        Ok(self.channels.get(&id).ok_or(Error::BadChannel)?.queue.len())
    }
    pub fn set_volume(&mut self, id: u32, volume: u32) -> Result<(), Error> {
        self.channel(id)?.volume = volume;
        Ok(())
    }
    pub fn set_playback_ratio(&mut self, id: u32, playback_ratio: u32) -> Result<(), Error> {
        self.channel(id)?.playback_ratio = playback_ratio;
        Ok(())
    }
    pub fn pause(&mut self, id: u32, paused: bool) -> Result<(), Error> {
        let c = self.channels.get_mut(&id).ok_or(Error::BadChannel)?;
        c.paused = paused;
        c.prepare(id, self.frame, &mut self.events);
        Ok(())
    }
    pub fn quiet(&mut self, id: u32) -> Result<(), Error> {
        let c = self.channels.get_mut(&id).ok_or(Error::BadChannel)?;
        if let Some(v) = c.voice.take() {
            self.events.push_back(Event::Cancelled {
                channel: id,
                token: v.token,
                frame: self.frame,
            });
        }
        c.wait = 0;
        Ok(())
    }
    pub fn flush(&mut self, id: u32) -> Result<(), Error> {
        let c = self.channels.get_mut(&id).ok_or(Error::BadChannel)?;
        for command in c.queue.drain(..) {
            if let Queued::Buffer { token, .. } = command {
                self.events.push_back(Event::Cancelled {
                    channel: id,
                    token,
                    frame: self.frame,
                });
            }
        }
        Ok(())
    }
    pub fn remove_channel(&mut self, id: u32) -> Result<(), Error> {
        self.quiet(id)?;
        self.flush(id)?;
        self.channels.remove(&id);
        self.events
            .retain(|e| !matches!(e,Event::Callback {channel,..} if *channel==id));
        Ok(())
    }
    pub fn pop_event(&mut self) -> Option<Event> {
        self.events.pop_front()
    }
    /// Always writes every requested stereo frame, including real silence.
    /// Phase persists across calls. Completion is at the first frame AFTER the
    /// last sample, and following buffers meet at that same boundary.
    pub fn render_into(&mut self, output: &mut [[i16; 2]]) {
        for out in output {
            let mut mix = [0i64; 2];
            for (&id, c) in &mut self.channels {
                c.prepare(id, self.frame, &mut self.events);
                if c.paused {
                    continue;
                }
                if c.wait > 0 {
                    c.wait -= 1;
                    continue;
                }
                let Some(v) = &mut c.voice else {
                    continue;
                };
                if c.playback_ratio == 0 {
                    continue;
                }
                let index = (v.phase >> 32) as usize;
                let first = i64::from(v.sample.samples[index]);
                let second = i64::from(
                    *v.sample
                        .samples
                        .get(index + 1)
                        .unwrap_or(&v.sample.samples[index]),
                );
                let sample = if c.interpolate {
                    first
                        + ((second - first) * (v.phase as u32 as i64)).div_euclid(PHASE_ONE as i64)
                } else {
                    first
                };
                // ampCmd is independent of stereo volumeCmd. Keep signed16
                // precision here; native 8-bit-device quantization belongs to
                // its output transport and is not reproduced in this sink.
                let sample = (sample * i64::from(c.amplitude)).div_euclid(255);
                mix[0] += (sample * i64::from(c.volume as u16)).div_euclid(256);
                mix[1] += (sample * i64::from((c.volume >> 16) as u16)).div_euclid(256);
                let step = (u64::from(v.sample.header.rate_fixed) * u64::from(c.playback_ratio))
                    / u64::from(self.rate);
                v.phase = v.phase.saturating_add(step);
                if (v.phase >> 32) >= v.sample.samples.len() as u64 {
                    self.events.push_back(Event::Finished {
                        channel: id,
                        token: v.token,
                        frame: self.frame + 1,
                    });
                    c.voice = None;
                }
            }
            *out = [
                (mix[0] * i64::from(self.master_volume as u16))
                    .div_euclid(256)
                    .clamp(-32768, 32767) as i16,
                (mix[1] * i64::from((self.master_volume >> 16) as u16))
                    .div_euclid(256)
                    .clamp(-32768, 32767) as i16,
            ];
            self.frame += 1;
        }
        for (&id, c) in &mut self.channels {
            c.prepare(id, self.frame, &mut self.events);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;
    fn sample(data: &[u8], rate: u32) -> Sample {
        Sample::decode(
            StandardHeader {
                sample_pointer: 0,
                frames: data.len() as u32,
                rate_fixed: rate * 65536,
                loop_start: 0,
                loop_end: 0,
                base_frequency: 60,
            },
            data,
        )
        .unwrap()
    }
    fn engine() -> Engine {
        let mut e = Engine::new(48000).unwrap();
        e.create_channel(1, 128, true).unwrap();
        e
    }
    fn buffer(e: &mut Engine, id: u32, data: &[u8], rate: u32, token: u64) {
        e.enqueue(
            id,
            Queued::Buffer {
                sample: sample(data, rate),
                token,
            },
        )
        .unwrap();
    }
    #[test]
    fn unsigned_conversion_silence_polarity_and_channel_layout() {
        let mut e = engine();
        buffer(&mut e, 1, &[0, 127, 128, 129, 255], 48000, 7);
        let mut out = [[1; 2]; 7];
        e.render_into(&mut out);
        assert_eq!(
            out,
            [
                [-32768; 2],
                [-256; 2],
                [0; 2],
                [256; 2],
                [32512; 2],
                [0; 2],
                [0; 2]
            ]
        );
        assert!(!e.busy(1).unwrap());
    }
    #[test]
    fn right_half_gain_and_overdrive_are_fixed_point() {
        let mut e = engine();
        e.set_volume(1, 0x00800100).unwrap();
        buffer(&mut e, 1, &[0, 64, 128, 192, 255], 48000, 0);
        let mut out = [[0; 2]; 5];
        e.render_into(&mut out);
        assert_eq!(
            out,
            [
                [-32768, -16384],
                [-16384, -8192],
                [0, 0],
                [16384, 8192],
                [32512, 16256]
            ]
        );
        e.set_volume(1, 0x04000400).unwrap();
        buffer(&mut e, 1, &[192, 64], 48000, 0);
        let mut out = [[0; 2]; 2];
        e.render_into(&mut out);
        assert_eq!(out, [[32767; 2], [-32768; 2]]);
    }
    #[test]
    fn queued_amplitude_preserves_stereo_gain_and_buffer_completion_order() {
        let mut e = engine();
        e.set_volume(1, 0x00800100).unwrap();
        buffer(&mut e, 1, &[0, 255], 48000, 1);
        e.enqueue(1, Queued::Amplitude(127)).unwrap();
        buffer(&mut e, 1, &[0, 255], 48000, 2);
        let mut out = [[0; 2]; 4];
        e.render_into(&mut out);
        assert_eq!(out, [[-32768, -16384], [32512, 16256], [-16320, -8160], [16192, 8096]]);
        assert_eq!(e.volume(1).unwrap(), 0x00800100);
        assert_eq!(e.amplitude(1).unwrap(), 127);
        assert!(!e.busy(1).unwrap());
        e.set_amplitude(1, 0).unwrap();
        buffer(&mut e, 1, &[0, 255], 48000, 3);
        let mut silence = [[1; 2]; 2];
        e.render_into(&mut silence);
        assert_eq!(silence, [[0; 2]; 2]);
        assert!(!e.busy(1).unwrap());
    }
    #[test]
    fn master_gain_scales_wide_mix_before_clipping_and_changes_without_reset() {
        let mut e = engine();
        e.create_channel(2, 128, true).unwrap();
        assert_eq!(e.master_volume(), 0x01000100);
        e.set_master_volume(0x00400080);
        assert_eq!(e.master_volume(), 0x00400080);
        buffer(&mut e, 1, &[255, 0, 192], 48000, 0);
        buffer(&mut e, 2, &[255, 0, 192], 48000, 0);
        let mut first = [[0; 2]; 2];
        e.render_into(&mut first);
        // Two full-scale voices must be attenuated before saturation.
        assert_eq!(first, [[32512, 16256], [-32768, -16384]]);
        e.set_master_volume(0);
        let mut muted = [[1; 2]; 1];
        e.render_into(&mut muted);
        assert_eq!(muted, [[0; 2]]);
        assert!(!e.busy(1).unwrap());
        assert!(!e.busy(2).unwrap());
    }
    #[test]
    fn all_voices_accumulate_before_one_saturation() {
        let mut e = engine();
        e.create_channel(2, 128, true).unwrap();
        e.create_channel(3, 128, true).unwrap();
        buffer(&mut e, 1, &[255, 0, 192, 64], 48000, 0);
        buffer(&mut e, 2, &[255, 0, 0, 255], 48000, 0);
        buffer(&mut e, 3, &[0, 128, 128, 128], 48000, 0);
        let mut out = [[0; 2]; 4];
        e.render_into(&mut out);
        assert_eq!(out, [[32256; 2], [-32768; 2], [-16384; 2], [16128; 2]]);
    }
    #[test]
    fn linear_upsampling_and_disabled_interpolation() {
        let mut e = engine();
        buffer(&mut e, 1, &[128, 160, 128], 12000, 0);
        let mut out = [[0; 2]; 12];
        e.render_into(&mut out);
        assert_eq!(
            out.map(|s| s[0]),
            [0, 2048, 4096, 6144, 8192, 6144, 4096, 2048, 0, 0, 0, 0]
        );
        let mut e = Engine::new(48000).unwrap();
        e.create_channel(1, 128, false).unwrap();
        buffer(&mut e, 1, &[128, 160, 128], 12000, 0);
        e.render_into(&mut out);
        assert_eq!(
            out.map(|s| s[0]),
            [0, 0, 0, 0, 8192, 8192, 8192, 8192, 0, 0, 0, 0]
        );
    }
    #[test]
    fn pitch_double_rate_and_zero_rate_pause() {
        let mut e = engine();
        buffer(&mut e, 1, &[128, 130, 132, 134, 136, 138], 48000, 0);
        e.set_playback_ratio(1, 2 * UNITY).unwrap();
        let mut out = [[0; 2]; 3];
        e.render_into(&mut out);
        assert_eq!(out, [[0; 2], [1024; 2], [2048; 2]]);
        assert!(!e.busy(1).unwrap());
        buffer(&mut e, 1, &[160, 192], 48000, 1);
        e.set_playback_ratio(1, 0).unwrap();
        e.render_into(&mut out);
        assert_eq!(out, [[0; 2]; 3]);
        assert!(e.busy(1).unwrap());
        e.set_playback_ratio(1, UNITY).unwrap();
        e.render_into(&mut out);
        assert_eq!(out, [[8192; 2], [16384; 2], [0; 2]]);
    }
    fn sequence() -> Engine {
        let mut e = engine();
        buffer(&mut e, 1, &[129, 130, 131, 132], 48000, 11);
        e.enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 123,
                param2: 0x12345678,
            }),
        )
        .unwrap();
        buffer(&mut e, 1, &[160, 192, 255], 48000, 12);
        e.enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 456,
                param2: 0x12345678,
            }),
        )
        .unwrap();
        e
    }
    fn events(e: &mut Engine) -> Vec<Event> {
        let mut out = Vec::new();
        while let Some(x) = e.pop_event() {
            out.push(x)
        }
        out
    }
    #[test]
    fn gapless_buffers_and_callback_order_independent_of_chunk_size() {
        let mut e = sequence();
        let mut one = [[0; 2]; 7];
        e.render_into(&mut one);
        let first = events(&mut e);
        assert_eq!(
            one,
            [[256; 2], [512; 2], [768; 2], [1024; 2], [8192; 2], [16384; 2], [32512; 2]]
        );
        assert_eq!(
            first,
            vec![
                Event::Started {
                    channel: 1,
                    token: 11,
                    frame: 0
                },
                Event::Finished {
                    channel: 1,
                    token: 11,
                    frame: 4
                },
                Event::Callback {
                    channel: 1,
                    command: Command {
                        code: 13,
                        param1: 123,
                        param2: 0x12345678
                    },
                    frame: 4
                },
                Event::Started {
                    channel: 1,
                    token: 12,
                    frame: 4
                },
                Event::Finished {
                    channel: 1,
                    token: 12,
                    frame: 7
                },
                Event::Callback {
                    channel: 1,
                    command: Command {
                        code: 13,
                        param1: 456,
                        param2: 0x12345678
                    },
                    frame: 7
                }
            ]
        );
        let mut e = sequence();
        let mut split = [[0; 2]; 7];
        e.render_into(&mut split[..2]);
        e.render_into(&mut split[2..3]);
        e.render_into(&mut split[3..]);
        assert_eq!(one, split);
        assert_eq!(first, events(&mut e));
        assert_eq!(e.frame(), 7);
    }
    #[test]
    fn pause_does_not_consume_buffer_and_flush_does_not_stop_current_voice() {
        let mut e = engine();
        buffer(&mut e, 1, &[129, 130, 131, 132], 48000, 1);
        let mut first = [[0; 2]; 2];
        e.render_into(&mut first);
        e.pause(1, true).unwrap();
        let mut quiet = [[1; 2]; 10];
        e.render_into(&mut quiet);
        assert_eq!(quiet, [[0; 2]; 10]);
        buffer(&mut e, 1, &[255], 48000, 2);
        e.flush(1).unwrap();
        e.pause(1, false).unwrap();
        e.render_into(&mut first);
        assert_eq!(first, [[768; 2], [1024; 2]]);
        assert!(!e.busy(1).unwrap());
        assert!(events(&mut e).contains(&Event::Finished {
            channel: 1,
            token: 1,
            frame: 14
        }));
    }
    #[test]
    fn fractional_rate_buffer_continuity() {
        fn make() -> Engine {
            let mut e = engine();
            let data: Vec<u8> = (0..137).map(|i| i as u8).collect();
            let mut s = sample(&data, 11127);
            s.header.rate_fixed = 0x2b7745d1;
            e.enqueue(
                1,
                Queued::Buffer {
                    sample: s,
                    token: 0,
                },
            )
            .unwrap();
            e
        }
        let mut a = make();
        let mut b = make();
        let mut full = [[0; 2]; 700];
        let mut split = [[0; 2]; 700];
        a.render_into(&mut full);
        for chunk in split.chunks_mut(17) {
            b.render_into(chunk)
        }
        assert_eq!(full, split);
        let completed = events(&mut a);
        assert_eq!(completed, events(&mut b));
        let expected_frames = (137u64 * 48000 * 65536).div_ceil(0x2b7745d1);
        assert!(completed.contains(&Event::Finished {
            channel: 1,
            token: 0,
            frame: expected_frames
        }));
        assert_eq!(full[expected_frames as usize - 1], [2048; 2]);
        assert_eq!(events(&mut make()).len(), 1);
        assert!(full[expected_frames as usize..]
            .iter()
            .all(|s| *s == [0; 2]));
    }
    #[test]
    fn decoder_rejects_incomplete_and_unsupported_headers() {
        assert_eq!(StandardHeader::parse(&[0; 21]), Err(Error::TruncatedHeader));
        let mut h = [0; 22];
        h[20] = 255;
        assert_eq!(StandardHeader::parse(&h), Err(Error::UnsupportedEncoding));
        h[20] = 0;
        assert_eq!(StandardHeader::parse(&h), Err(Error::InvalidRate));
        let header = sample(&[128, 129], 48000).header;
        assert_eq!(
            Sample::decode(header, &[128]).unwrap_err(),
            Error::TruncatedSamples
        );
        let mut resource = vec![0u8; 29];
        resource[1] = 2;
        resource[5] = 1;
        resource[6..8].copy_from_slice(&0x8051u16.to_be_bytes());
        resource[10..14].copy_from_slice(&u32::MAX.to_be_bytes());
        let r = Resource::parse(&resource).unwrap();
        assert_eq!(
            r.buffer(r.command(0).unwrap()).unwrap_err(),
            Error::TruncatedHeader
        );
        resource[5] = 2;
        assert!(Resource::parse(&resource[..21]).is_err());
    }
    #[test]
    fn both_resource_formats_decode_the_same_owned_buffer_once() {
        for format in [1, 2] {
            let offset = if format == 1 { 20 } else { 14 };
            let mut bytes = vec![0u8; offset + 25];
            bytes[1] = format;
            let command = if format == 1 {
                bytes[3] = 1;
                bytes[5] = 5;
                bytes[8..10].copy_from_slice(&0x00a0u16.to_be_bytes());
                bytes[11] = 1;
                12
            } else {
                bytes[5] = 1;
                6
            };
            bytes[command..command + 2].copy_from_slice(&0x8051u16.to_be_bytes());
            bytes[command + 4..command + 8].copy_from_slice(&(offset as u32).to_be_bytes());
            bytes[offset + 4..offset + 8].copy_from_slice(&3u32.to_be_bytes());
            bytes[offset + 8..offset + 12].copy_from_slice(&0xbb800000u32.to_be_bytes());
            bytes[offset + 12..offset + 16].copy_from_slice(&1u32.to_be_bytes());
            bytes[offset + 16..offset + 20].copy_from_slice(&2u32.to_be_bytes());
            bytes[offset + 21] = 60;
            bytes[offset + 22..].copy_from_slice(&[0, 128, 255]);
            let r = Resource::parse(&bytes).unwrap();
            assert_eq!(r.command_count, 1);
            let s = r.buffer(r.command(0).unwrap()).unwrap();
            assert_eq!(s.frames(), 3);
            assert_eq!((s.header.loop_start, s.header.loop_end), (1, 2));
            let mut e = engine();
            e.enqueue(
                1,
                Queued::Buffer {
                    sample: s,
                    token: 1,
                },
            )
            .unwrap();
            let mut out = [[0; 2]; 5];
            e.render_into(&mut out);
            assert_eq!(out, [[-32768; 2], [0; 2], [32512; 2], [0; 2], [0; 2]]);
            assert!(!e.busy(1).unwrap());
        }
    }
    #[test]
    fn full_queue_and_cancellation_never_fabricate_a_callback() {
        let mut e = Engine::new(48000).unwrap();
        e.create_channel(1, 1, true).unwrap();
        buffer(&mut e, 1, &[255, 255], 48000, 1);
        e.enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 1,
                param2: 2,
            }),
        )
        .unwrap();
        assert_eq!(e.enqueue(1, Queued::WaitFrames(1)), Err(Error::QueueFull));
        e.quiet(1).unwrap();
        e.flush(1).unwrap();
        let mut out = [[1; 2]; 8];
        e.render_into(&mut out);
        assert_eq!(out, [[0; 2]; 8]);
        assert!(events(&mut e)
            .iter()
            .all(|x| !matches!(x, Event::Callback { .. } | Event::Finished { .. })));
        e.enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 1,
                param2: 2,
            }),
        )
        .unwrap();
        e.remove_channel(1).unwrap();
        assert!(events(&mut e)
            .iter()
            .all(|x| !matches!(x, Event::Callback { .. })));
        assert_eq!(e.busy(1), Err(Error::BadChannel));
    }
    #[test]
    fn wait_and_empty_buffers_have_exact_completion_boundaries() {
        let mut e = engine();
        buffer(&mut e, 1, &[], 48000, 1);
        e.enqueue(1, Queued::WaitFrames(3)).unwrap();
        buffer(&mut e, 1, &[255], 48000, 2);
        e.enqueue(
            1,
            Queued::Callback(Command {
                code: 13,
                param1: 1,
                param2: 2,
            }),
        )
        .unwrap();
        let mut out = [[1; 2]; 5];
        e.render_into(&mut out);
        assert_eq!(out, [[0; 2], [0; 2], [0; 2], [32512; 2], [0; 2]]);
        let ev = events(&mut e);
        assert!(ev.contains(&Event::Finished {
            channel: 1,
            token: 1,
            frame: 0
        }));
        assert!(ev.contains(&Event::Callback {
            channel: 1,
            command: Command {
                code: 13,
                param1: 1,
                param2: 2
            },
            frame: 4
        }));
    }
}
