//! Per-application Macintosh event queue, independent of the host event system.
use std::collections::VecDeque;
/// Public physical keyboard bitmap, independent of queued events.
pub const KEY_MAP: u32 = 0x174;
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Event {
    pub what: u16,
    pub message: u32,
    pub when: u32,
    pub v: i16,
    pub h: i16,
    pub modifiers: u16,
}
impl Event {
    fn matches(self, mask: u16) -> bool {
        self.what < 16 && mask & (1u16 << self.what) != 0
    }
}
pub struct Events {
    queue: VecDeque<Event>,
    pub ticks: u32,
    pub mouse: (i16, i16),
    pub modifiers: u16,
    pub mouse_down_modifiers: u16,
}
impl Events {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            ticks: 0,
            mouse: (0, 0),
            modifiers: 0x80,
            mouse_down_modifiers: 0x80,
        }
    }
    /// Publish a device snapshot even when the guest never retrieves events.
    pub fn set_mouse<B: m68k::AddressBus>(&mut self, m: &mut B, point: (i16, i16)) {
        classic_core::mouse::publish(m, point);
        self.mouse = point;
    }
    /// Publish a device snapshot even when the guest never retrieves events.
    pub fn publish_keys<B: m68k::AddressBus>(m: &mut B, keys: [u8; 16]) {
        for (i, value) in keys.into_iter().enumerate() {
            m.write_byte(KEY_MAP + i as u32, value);
        }
    }
    /// Public event-state traps, shared by application execution and owned CPU probes.
    pub fn dispatch_state<B: m68k::AddressBus>(
        &self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        q: &crate::quickdraw::QuickDraw,
    ) -> Result<bool, String> {
        let sp = cpu.dar[15];
        match op {
            0xa973 => {
                let down = self.modifiers & 0x80 == 0
                    && !self.queue.iter().any(|e| matches!(e.what, 1 | 2));
                m.write_byte(sp, u8::from(down));
                cpu.set_ccr(0);
            }
            0xa974 => {
                let down = self.modifiers & 0x80 == 0;
                m.write_word(sp, if down { 0x0101 } else { 0 });
                cpu.set_ccr(if down { 16 } else { 20 });
            }
            0xa976 => {
                let out = m.read_long(sp);
                let keys: [u8; 16] = std::array::from_fn(|i| m.read_byte(KEY_MAP + i as u32));
                for (i, value) in keys.into_iter().enumerate() {
                    m.write_byte(out + i as u32, value);
                }
                cpu.dar[15] += 4;
                cpu.set_ccr(16);
                // Native D0 is ROM implementation scratch, not a public result.
            }
            0xa975 => m.write_long(sp, self.ticks),
            0xa972 => {
                let out = m.read_long(sp);
                let port = q.get_port(m)?;
                let bitmap = q.port_bitmap(m, port)?;
                let bounds = crate::quickdraw::read_rect(m, bitmap + 6);
                let point = classic_core::mouse::read(m, classic_core::mouse::LOCATION);
                m.write_word(out, point.0.wrapping_add(bounds.top) as u16);
                m.write_word(out + 2, point.1.wrapping_add(bounds.left) as u16);
                cpu.dar[15] += 4;
            }
            _ => return Ok(false),
        }
        cpu.pc += 2;
        Ok(true)
    }
    /// OS queue retrieval uses register arguments and does not synthesize
    /// window-manager events. The Toolbox event entry points remain separate.
    pub fn dispatch_os_event<B: m68k::AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
    ) -> bool {
        if !matches!(op, 0xa030 | 0xa031) {
            return false;
        }
        let x = if cpu.flag_x() { 16 } else { 0 };
        let (found, event) = self.next(cpu.dar[0] as u16, op == 0xa031);
        write_event(m, cpu.dar[8], event);
        cpu.dar[0] = if found { 0 } else { u32::MAX };
        cpu.set_ccr(if found { 4 } else { x | 8 });
        cpu.pc += 2;
        true
    }
    /// Read-only queue observation for owned behavioral probes.
    pub(crate) fn snapshot(&self) -> Vec<Event> {
        self.queue.iter().copied().collect()
    }
    /// OS PostEvent takes event number in A0 and message in D0.
    pub fn dispatch_post(&mut self, op: u16, cpu: &mut m68k::CpuCore) -> bool {
        if op != 0xa02f {
            return false;
        }
        let event = Event {
            what: cpu.dar[8] as u16,
            message: cpu.dar[0],
            when: self.ticks,
            v: self.mouse.0,
            h: self.mouse.1,
            modifiers: self.modifiers,
        };
        cpu.dar[0] = self.post(event).err().unwrap_or(0) as i32 as u32;
        cpu.pc += 2;
        true
    }
    /// Host input has already arrived at the virtual device. Queue it now,
    /// so mouse tracking can observe releases without another GetNextEvent.
    /// Pointer motion updates device state but is not an OS queue event.
    #[cfg(any(feature = "gui", test))]
    pub fn post_live(&mut self, event: Event) {
        if event.what == 0 {
            return;
        }
        if self.queue.len() >= 20 {
            self.queue.pop_front();
        }
        self.queue.push_back(event);
    }
    pub fn post(&mut self, event: Event) -> Result<(), i16> {
        if self.queue.len() >= 20 {
            return Err(-1);
        }
        self.queue.push_back(event);
        Ok(())
    }
    /// Window damage lives in updateRgn; queued hints coalesce and do not
    /// exhaust the input-event queue. A dirty window is also discoverable later.
    pub fn post_update(&mut self, event: Event) {
        if !self
            .queue
            .iter()
            .any(|e| e.what == 6 && e.message == event.message)
            && self.queue.len() < 20
        {
            self.queue.push_back(event);
        }
    }
    pub fn remove_update(&mut self, address: u32) {
        self.queue
            .retain(|e| !(e.what == 6 && e.message == address));
    }
    pub fn remove_window(&mut self, address: u32) {
        self.queue
            .retain(|e| !((e.what == 6 || e.what == 8) && e.message == address));
    }
    pub fn take_key(&mut self, character: u8) -> bool {
        let found = self
            .queue
            .iter()
            .any(|e| e.what == 3 && e.message as u8 == character);
        self.queue
            .retain(|e| !(e.what == 3 && e.message as u8 == character));
        found
    }
    pub fn flush(&mut self, which: u16, stop: u16) {
        let mut reached_stop = false;
        self.queue.retain(|e| {
            if e.matches(stop) {
                reached_stop = true;
            }
            reached_stop || !e.matches(which)
        });
    }
    pub fn next(&mut self, mask: u16, remove: bool) -> (bool, Event) {
        let event = self.queue.iter().position(|e| e.matches(mask)).map(|i| {
            if remove {
                self.queue.remove(i).unwrap()
            } else {
                self.queue[i]
            }
        });
        if remove {
            if let Some(e) = event {
                if e.what == 1 {
                    self.mouse_down_modifiers = e.modifiers;
                }
            }
        }
        event.map(|e| (true, e)).unwrap_or((
            false,
            Event {
                what: 0,
                message: 0,
                when: self.ticks,
                v: self.mouse.0,
                h: self.mouse.1,
                modifiers: self.modifiers,
            },
        ))
    }
}
pub fn write_event<B: m68k::AddressBus>(m: &mut B, p: u32, event: Event) {
    m.write_word(p, event.what);
    m.write_long(p + 2, event.message);
    m.write_long(p + 6, event.when);
    m.write_word(p + 10, event.v as u16);
    m.write_word(p + 12, event.h as u16);
    m.write_word(p + 14, event.modifiers);
}
pub fn read_script(path: &str) -> Result<VecDeque<Event>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("event script: {e}"))?;
    let data: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|e| format!("event script: {e}"))?;
    if data["schema"] != 1 {
        return Err("event script requires schema 1".into());
    }
    let mut result = VecDeque::new();
    for item in data["events"].as_array().ok_or("missing script events")? {
        let number = |key: &str, max: u64| -> Result<u64, String> {
            let n = item[key]
                .as_u64()
                .ok_or_else(|| format!("missing event {key}"))?;
            if n > max {
                return Err(format!("event {key} out of range"));
            }
            Ok(n)
        };
        result.push_back(Event {
            what: number("what", 15)? as u16,
            message: number("message", u32::MAX as u64)? as u32,
            modifiers: number("modifiers", 65535)? as u16,
            v: coordinate(item, "v")?,
            h: coordinate(item, "h")?,
            ..Event::default()
        });
    }
    Ok(result)
}
fn coordinate(item: &serde_json::Value, key: &str) -> Result<i16, String> {
    match item.get(key) {
        None => Ok(0),
        Some(value) => value
            .as_i64()
            .and_then(|v| i16::try_from(v).ok())
            .ok_or_else(|| format!("event {key} coordinate out of range")),
    }
}

#[cfg(test)]
mod damage_hint_tests {
    use super::*;
    #[test]
    fn repeated_damage_and_validation_preserve_keyboard_and_activation_events() {
        let mut events = Events::new();
        let address = 0x123400;
        for _ in 0..100 {
            events.post_update(Event {
                what: 6,
                message: address,
                ..Event::default()
            });
        }
        let key = Event {
            what: 3,
            message: 0x0061,
            modifiers: 0x180,
            ..Event::default()
        };
        let active = Event {
            what: 8,
            message: address,
            modifiers: 0x81,
            ..Event::default()
        };
        assert_eq!(events.post(key), Ok(()));
        assert_eq!(events.post(active), Ok(()));
        events.remove_update(address);
        assert!(!events.next(1 << 6, false).0);
        assert_eq!(events.next(1 << 3, true), (true, key));
        assert_eq!(events.next(1 << 8, true), (true, active));
    }
}
#[cfg(test)]
mod live_input_tests {
    use super::*;
    use m68k::AddressBus;
    #[test]
    fn release_arriving_during_tracking_is_visible_before_get_next_event() {
        let mut events = Events::new();
        let mut m = crate::machine::Memory::zeroed(0x10000);
        let q = crate::quickdraw::QuickDraw::new();
        let mut cpu = m68k::CpuCore::new();
        cpu.dar[15] = 0xf000;
        events.modifiers = 0;
        events.dispatch_state(0xa973, &mut cpu, &mut m, &q).unwrap();
        assert_eq!(m.read_byte(0xf000), 1);
        // A native host release enters the OS queue while a tracking routine
        // is still executing; there has been no event retrieval call.
        events.post_live(Event {
            what: 2,
            modifiers: 0x80,
            ..Event::default()
        });
        events.modifiers = 0x80;
        events.dispatch_state(0xa973, &mut cpu, &mut m, &q).unwrap();
        assert_eq!(m.read_byte(0xf000), 0);
        // A later physical press must not hide the pending release. StillDown
        // observes queued mouse events; Button reports the physical state.
        events.modifiers = 0;
        events.dispatch_state(0xa973, &mut cpu, &mut m, &q).unwrap();
        assert_eq!(m.read_byte(0xf000), 0);
        events.dispatch_state(0xa974, &mut cpu, &mut m, &q).unwrap();
        assert_eq!(m.read_word(0xf000), 0x0101);
        assert_eq!(events.next(4, true).1.what, 2);
        events.dispatch_state(0xa973, &mut cpu, &mut m, &q).unwrap();
        assert_eq!(m.read_byte(0xf000), 1);
    }
}
