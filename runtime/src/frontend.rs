//! Native presentation and keyboard input; Macintosh services remain in the runtime.
use crate::{
    events::{Event, Events},
    mac_roman::MAC_ROMAN,
    quickdraw::QuickDraw,
};
use m68k::AddressBus;
#[cfg(not(target_os = "macos"))]
use minifb::MouseButton;
use minifb::{InputCallback, Key, MouseMode, Window, WindowOptions};
use std::{
    cell::RefCell,
    collections::{BTreeSet, VecDeque},
    fs::File,
    io::Write,
    rc::Rc,
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

static SNAPSHOT_REQUESTED: AtomicBool = AtomicBool::new(false);
#[cfg(any(target_os = "macos", target_os = "linux"))]
extern "C" fn request_snapshot(_: i32) {
    // Signal handlers only touch a lock-free flag; all I/O runs on the GUI loop.
    SNAPSHOT_REQUESTED.store(true, Ordering::Relaxed);
}
#[cfg(any(target_os = "macos", target_os = "linux"))]
fn install_snapshot_signal() -> Result<(), String> {
    extern "C" {
        fn signal(number: i32, handler: usize) -> usize;
    }
    #[cfg(target_os = "macos")]
    let number = 30; // SIGUSR1
    #[cfg(target_os = "linux")]
    let number = 10;
    if unsafe { signal(number, request_snapshot as *const () as usize) } == usize::MAX {
        return Err("could not register framebuffer snapshot signal".into());
    }
    Ok(())
}
#[derive(Default)]
struct Keyboard {
    down: BTreeSet<Key>,
    pending: VecDeque<Event>,
    physical: u8,
    pointer: (i16, i16),
    mouse_down: bool,
}
impl Keyboard {
    fn publish_keys<B: AddressBus>(&mut self, m: &mut B, active: bool) {
        // A release delivered to another window must not leave a held guest key.
        if !active {
            self.down.clear();
        }
        let mut keys = [0u8; 16];
        for key in &self.down {
            if let Some(code) = mac_key_code(*key) {
                keys[usize::from(code / 8)] |= 1 << (code % 8);
            }
        }
        Events::publish_keys(m, keys);
    }
    fn modifiers(&self) -> u16 {
        let has = |a, b| self.down.contains(&a) || self.down.contains(&b);
        (if self.mouse_down { 0 } else { 0x80 })
            | if has(Key::LeftSuper, Key::RightSuper) {
                0x100
            } else {
                0
            }
            | if has(Key::LeftShift, Key::RightShift) {
                0x200
            } else {
                0
            }
            | if has(Key::LeftAlt, Key::RightAlt) {
                0x800
            } else {
                0
            }
            | if has(Key::LeftCtrl, Key::RightCtrl) {
                0x1000
            } else {
                0
            }
    }
    fn emit(&mut self, c: u8, k: u8) {
        self.pending.push_back(Event {
            what: 3,
            message: ((k as u32) << 8) | c as u32,
            modifiers: self.modifiers(),
            v: self.pointer.0,
            h: self.pointer.1,
            ..Event::default()
        });
    }
    fn pointer(&mut self, x: f32, y: f32, button: i32, state: bool) {
        self.pointer = (y as i16, x as i16);
        if button == 0 {
            self.mouse_down = state;
        }
        if button == -1 || button == 0 {
            self.pending.push_back(Event {
                what: if button == -1 {
                    0
                } else if state {
                    1
                } else {
                    2
                },
                message: 0,
                modifiers: self.modifiers(),
                v: self.pointer.0,
                h: self.pointer.1,
                ..Event::default()
            });
        }
    }
    fn key(&mut self, key: Key, state: bool) {
        if !state {
            self.down.remove(&key);
            return;
        }
        self.down.insert(key);
        self.physical = mac_key_code(key).unwrap_or(0);
        let special = match key {
            Key::Backspace => Some((8, 0x33)),
            Key::Delete => Some((0x7f, 0x75)),
            Key::Enter => Some((13, 0x24)),
            Key::NumPadEnter => Some((13, 0x4c)),
            Key::Escape => Some((27, 0x35)),
            Key::Tab => Some((9, 0x30)),
            Key::Left => Some((0x1c, 0x7b)),
            Key::Right => Some((0x1d, 0x7c)),
            Key::Up => Some((0x1e, 0x7e)),
            Key::Down => Some((0x1f, 0x7d)),
            _ => None,
        };
        if let Some((c, k)) = special {
            self.emit(c, k);
        } else if self.modifiers() & 0x100 != 0 {
            if let Some((c, k)) = shortcut(key) {
                self.emit(
                    if self.modifiers() & 0x200 != 0 {
                        c.to_ascii_uppercase()
                    } else {
                        c
                    },
                    k,
                );
            }
        }
    }
    fn character(&mut self, c: u32) {
        if self.modifiers() & 0x100 == 0 && c >= 32 && c != 127 && !(0xf700..=0xf8ff).contains(&c) {
            if let Some(i) = MAC_ROMAN.iter().position(|&v| v as u32 == c) {
                self.emit(i as u8, self.physical);
            } else {
                eprintln!("Character U+{c:04X} cannot be represented in Macintosh Roman");
            }
        }
    }
}
struct Input(Rc<RefCell<Keyboard>>);
impl InputCallback for Input {
    fn pointer_event(&mut self, x: f32, y: f32, button: i32, state: bool) {
        self.0.borrow_mut().pointer(x, y, button, state);
    }
    fn add_char(&mut self, c: u32) {
        self.0.borrow_mut().character(c);
    }
    fn set_key_state(&mut self, k: Key, state: bool) {
        self.0.borrow_mut().key(k, state);
    }
}
pub struct Frontend {
    window: Window,
    pixels: Vec<u32>,
    keyboard: Rc<RefCell<Keyboard>>,
    started: Instant,
    presented: Instant,
    mouse_down: bool,
    host_cursor_visible: bool,
    last_host_mouse: Option<(i16, i16)>,
    mouse_driver: classic_core::mouse::Driver,
    cursor_warps: u64,
    presentation_log: Option<File>,
    snapshot_path: Option<String>,
    frame: u64,
}
pub struct PresentationContext<'a> {
    pub pc: u32,
    /// Execution-loop counter, including modal waits and callback scheduling.
    pub instruction: usize,
    pub update_depth: usize,
    pub audio_frame: &'a dyn Fn() -> Option<u64>,
}
impl Frontend {
    pub fn new(q: &QuickDraw, presentation_log: Option<&str>) -> Result<Self, String> {
        let presentation_log = presentation_log
            .map(File::create)
            .transpose()
            .map_err(|e| format!("presentation log: {e}"))?;
        let mut window = Window::new(
            "Classic Runtime",
            q.width as usize,
            q.height as usize,
            WindowOptions::default(),
        )
        .map_err(|e| e.to_string())?;
        window.set_target_fps(0);
        let keyboard = Rc::new(RefCell::new(Keyboard::default()));
        window.set_input_callback(Box::new(Input(keyboard.clone())));
        Ok(Self {
            window,
            pixels: vec![0xffffff; q.width as usize * q.height as usize],
            keyboard,
            started: Instant::now(),
            presented: Instant::now() - Duration::from_secs(1),
            mouse_down: false,
            host_cursor_visible: true,
            last_host_mouse: None,
            mouse_driver: classic_core::mouse::Driver::default(),
            cursor_warps: 0,
            presentation_log,
            snapshot_path: None,
            frame: 0,
        })
    }
    /// SIGUSR1 requests a read-only snapshot to the configured final-capture
    /// path. It neither injects guest input nor changes the guest execution state.
    pub fn set_snapshot_path(&mut self, path: Option<&str>) -> Result<(), String> {
        self.snapshot_path = path.map(str::to_owned);
        if self.snapshot_path.is_some() {
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            install_snapshot_signal()?;
        }
        Ok(())
    }
    pub fn poll<B: AddressBus>(
        &mut self,
        m: &mut B,
        q: &QuickDraw,
        events: &mut Events,
        _input: &mut VecDeque<Event>,
        context: PresentationContext<'_>,
    ) -> Result<bool, String> {
        events.ticks = (self.started.elapsed().as_nanos() * 60 / 1_000_000_000) as u32;
        m.write_long(0x16a, events.ticks);
        if self.presented.elapsed() < Duration::from_millis(16) {
            return Ok(self.window.is_open());
        }
        let conversion_started = Instant::now();
        let audio_frame_before = (context.audio_frame)();
        let interval_us = self.presented.elapsed().as_micros();
        let mut hash = 0xcbf29ce484222325u64;
        let mut dark_pixels = 0usize;
        let tracing = self.presentation_log.is_some();
        let palette = q.screen_colors(m)?;
        for y in 0..q.height as u32 {
            for x in 0..q.width as u32 {
                let byte = m.read_byte(
                    q.screen_base + y * q.row_bytes() as u32 + if q.depth == 8 { x } else { x / 8 },
                );
                let index = if q.depth == 8 {
                    byte
                } else {
                    u8::from(byte & (0x80 >> (x & 7)) != 0)
                };
                let rgb = palette[index as usize];
                if tracing {
                    if q.depth == 8 || x & 7 == 0 {
                        hash = (hash ^ byte as u64).wrapping_mul(0x100000001b3);
                    }
                    dark_pixels += usize::from(rgb != 0xffffff);
                }
                self.pixels[(y * q.width as u32 + x) as usize] = rgb;
            }
        }
        if SNAPSHOT_REQUESTED.swap(false, Ordering::Relaxed) {
            if let Some(path) = &self.snapshot_path {
                let mut bytes = format!("P6\n{} {}\n255\n", q.width, q.height).into_bytes();
                bytes.reserve(self.pixels.len() * 3);
                for rgb in &self.pixels {
                    bytes.extend_from_slice(&[(rgb >> 16) as u8, (rgb >> 8) as u8, *rgb as u8]);
                }
                std::fs::write(path, &bytes).map_err(|e| format!("framebuffer snapshot: {e}"))?;
                let keys = (0..16)
                    .map(|i| format!("{:02x}", m.read_byte(crate::events::KEY_MAP + i)))
                    .collect::<String>();
                let low_mouse = |m: &mut B, address: u32| [m.read_word(address) as i16, m.read_word(address + 2) as i16];
                let metadata = serde_json::json!({"frame":self.frame,"audio_frame_before":audio_frame_before,"audio_frame_after":(context.audio_frame)(),"elapsed_us":self.started.elapsed().as_micros(),"pc":context.pc,"execution_iteration":context.instruction,"key_map_hex":keys,"mouse":[events.mouse.0,events.mouse.1],"host_mouse":self.last_host_mouse,"cursor_warps":self.cursor_warps,"low_memory_mouse":{"0x828":low_mouse(m,0x828),"0x82c":low_mouse(m,0x82c),"0x830":low_mouse(m,0x830),"0x8ce_word":m.read_word(0x8ce),"cursor_visible":m.read_byte(classic_core::cursor::VISIBLE),"cursor_state":m.read_word(classic_core::cursor::STATE) as i16},"modifiers":events.modifiers,"ticks":events.ticks,"active":self.window.is_active(),"read_only_observation":true});
                std::fs::write(format!("{path}.json"), metadata.to_string())
                    .map_err(|e| format!("snapshot metadata: {e}"))?;
                println!(
                    "framebuffer snapshot frame={} pc={:#x} path={path}",
                    self.frame, context.pc
                );
            }
        }
        let conversion_us = conversion_started.elapsed().as_micros();
        let present_started = Instant::now();
        self.window
            .update_with_buffer(&self.pixels, q.width as usize, q.height as usize)
            .map_err(|e| e.to_string())?;
        self.presented = Instant::now();
        let present_us = present_started.elapsed().as_micros();
        if let Some(log) = self.presentation_log.as_mut() {
            writeln!(
                log,
                "{}",
                serde_json::json!({
                    "frame": self.frame, "elapsed_us": self.started.elapsed().as_micros(),
                    "interval_us": interval_us, "conversion_us": conversion_us,
                    "present_us": present_us, "pc": context.pc,
                    "instruction": context.instruction, "update_depth": context.update_depth,
                    "dark_pixels": dark_pixels, "framebuffer_fnv64": format!("{hash:016x}")
                })
            )
            .map_err(|e| format!("presentation log: {e}"))?;
        }
        self.frame += 1;
        let cursor_request = m.read_byte(classic_core::mouse::NEW) != 0
            && m.read_byte(classic_core::mouse::COUPLE) != 0;
        self.mouse_driver.service(m);
        let mode = if cursor_request { MouseMode::Pass } else { MouseMode::Discard };
        let host_point = self.window.get_mouse_pos(mode)
            .filter(|_| self.window.is_active())
            .map(|(x,y)| (y as i16,x as i16));
        if let Some(point) = host_point {
            if let Some(previous) = self.last_host_mouse {
                self.mouse_driver.motion(m, (point.0.wrapping_sub(previous.0), point.1.wrapping_sub(previous.1)));
            } else if !cursor_request {
                self.mouse_driver.absolute(m, point);
            }
        }
        self.last_host_mouse = host_point;
        events.mouse = classic_core::mouse::read(m, classic_core::mouse::LOCATION);
        let mut warped = false;
        #[cfg(target_os = "macos")]
        if cursor_request && host_point.is_some() && host_point != Some(events.mouse) {
            warped = self.window.set_mouse_position(events.mouse.1 as f32, events.mouse.0 as f32);
            if warped {
                self.last_host_mouse = Some(events.mouse);
                self.cursor_warps += 1;
            }
        }
        let visible = m.read_byte(classic_core::cursor::VISIBLE) != 0
            || !self.window.is_active()
            || self.window.get_mouse_pos(MouseMode::Discard).is_none();
        if visible != self.host_cursor_visible {
            self.window.set_cursor_visibility(visible);
            self.host_cursor_visible = visible;
        }
        let mut keyboard = self.keyboard.borrow_mut();
        if warped { keyboard.pointer = events.mouse; }
        keyboard.publish_keys(m, self.window.is_active());
        #[cfg(not(target_os = "macos"))]
        let down = self.window.get_mouse_down(MouseButton::Left);
        #[cfg(target_os = "macos")]
        let down = keyboard.mouse_down;
        let button = if down { 0 } else { 0x80 };
        events.modifiers = (keyboard.modifiers() & !0x80) | button;
        if !cfg!(target_os = "macos") && down != self.mouse_down {
            events.post_live(Event {
                what: if down { 1 } else { 2 },
                message: 0,
                when: events.ticks,
                v: events.mouse.0,
                h: events.mouse.1,
                modifiers: events.modifiers,
            });
            self.mouse_down = down;
        }
        for mut event in keyboard.pending.drain(..) {
            if let Some(point) = host_point {
                event.v = event.v.wrapping_add(events.mouse.0.wrapping_sub(point.0));
                event.h = event.h.wrapping_add(events.mouse.1.wrapping_sub(point.1));
            }
            event.when = events.ticks;
            if event.what != 0 {
                println!(
                    "host event: what={} message={:#x} modifiers={:#x} point=({},{})",
                    event.what, event.message, event.modifiers, event.h, event.v
                );
            }
            events.post_live(event);
        }
        Ok(self.window.is_open())
    }
}
impl Drop for Frontend {
    fn drop(&mut self) {
        // Balance the host hide once when a hidden guest window closes.
        if !self.host_cursor_visible {
            self.window.set_cursor_visibility(true);
        }
    }
}
// Physical Macintosh virtual codes, following the existing minifb macOS
// adapter. Native OS9 fixtures independently verify Up, W, Space and Shift.
// The host enum cannot distinguish main/keypad Equal or every extended key.
fn mac_key_code(key: Key) -> Option<u8> {
    Some(match key {
        Key::A => 0x00,
        Key::S => 0x01,
        Key::D => 0x02,
        Key::F => 0x03,
        Key::H => 0x04,
        Key::G => 0x05,
        Key::Z => 0x06,
        Key::X => 0x07,
        Key::C => 0x08,
        Key::V => 0x09,
        Key::B => 0x0b,
        Key::Q => 0x0c,
        Key::W => 0x0d,
        Key::E => 0x0e,
        Key::R => 0x0f,
        Key::Y => 0x10,
        Key::T => 0x11,
        Key::Key1 => 0x12,
        Key::Key2 => 0x13,
        Key::Key3 => 0x14,
        Key::Key4 => 0x15,
        Key::Key6 => 0x16,
        Key::Key5 => 0x17,
        Key::Equal => 0x18,
        Key::Key9 => 0x19,
        Key::Key7 => 0x1a,
        Key::Minus => 0x1b,
        Key::Key8 => 0x1c,
        Key::Key0 => 0x1d,
        Key::RightBracket => 0x1e,
        Key::O => 0x1f,
        Key::U => 0x20,
        Key::LeftBracket => 0x21,
        Key::I => 0x22,
        Key::P => 0x23,
        Key::Enter => 0x24,
        Key::L => 0x25,
        Key::J => 0x26,
        Key::Apostrophe => 0x27,
        Key::K => 0x28,
        Key::Semicolon => 0x29,
        Key::Backslash => 0x2a,
        Key::Comma => 0x2b,
        Key::Slash => 0x2c,
        Key::N => 0x2d,
        Key::M => 0x2e,
        Key::Period => 0x2f,
        Key::Tab => 0x30,
        Key::Space => 0x31,
        Key::Backspace => 0x33,
        Key::Escape => 0x35,
        Key::RightSuper => 0x36,
        Key::LeftSuper => 0x37,
        Key::LeftShift => 0x38,
        Key::CapsLock => 0x39,
        Key::LeftAlt => 0x3a,
        Key::LeftCtrl => 0x3b,
        Key::RightShift => 0x3c,
        Key::RightAlt => 0x3d,
        Key::RightCtrl => 0x3e,
        Key::NumPadDot => 0x41,
        Key::NumPadAsterisk => 0x43,
        Key::NumPadPlus => 0x45,
        Key::NumLock => 0x47,
        Key::NumPadSlash => 0x4b,
        Key::NumPadEnter => 0x4c,
        Key::NumPadMinus => 0x4e,
        Key::NumPad0 => 0x52,
        Key::NumPad1 => 0x53,
        Key::NumPad2 => 0x54,
        Key::NumPad3 => 0x55,
        Key::NumPad4 => 0x56,
        Key::NumPad5 => 0x57,
        Key::NumPad6 => 0x58,
        Key::NumPad7 => 0x59,
        Key::NumPad8 => 0x5b,
        Key::NumPad9 => 0x5c,
        Key::F5 => 0x60,
        Key::F6 => 0x61,
        Key::F7 => 0x62,
        Key::F3 => 0x63,
        Key::F8 => 0x64,
        Key::F9 => 0x65,
        Key::F11 => 0x67,
        Key::F14 => 0x6b,
        Key::F10 => 0x6d,
        Key::F12 => 0x6f,
        Key::F15 => 0x71,
        Key::Insert => 0x72,
        Key::Home => 0x73,
        Key::PageUp => 0x74,
        Key::Delete => 0x75,
        Key::F4 => 0x76,
        Key::End => 0x77,
        Key::F2 => 0x78,
        Key::PageDown => 0x79,
        Key::F1 => 0x7a,
        Key::Left => 0x7b,
        Key::Right => 0x7c,
        Key::Down => 0x7d,
        Key::Up => 0x7e,
        _ => return None,
    })
}
fn shortcut(k: Key) -> Option<(u8, u8)> {
    Some(match k {
        Key::A => (b'a', 0),
        Key::S => (b's', 1),
        Key::D => (b'd', 2),
        Key::F => (b'f', 3),
        Key::H => (b'h', 4),
        Key::G => (b'g', 5),
        Key::Z => (b'z', 6),
        Key::X => (b'x', 7),
        Key::C => (b'c', 8),
        Key::V => (b'v', 9),
        Key::B => (b'b', 11),
        Key::Q => (b'q', 12),
        Key::W => (b'w', 13),
        Key::E => (b'e', 14),
        Key::R => (b'r', 15),
        Key::Y => (b'y', 16),
        Key::T => (b't', 17),
        Key::O => (b'o', 31),
        Key::U => (b'u', 32),
        Key::I => (b'i', 34),
        Key::P => (b'p', 35),
        Key::L => (b'l', 37),
        Key::J => (b'j', 38),
        Key::K => (b'k', 40),
        Key::N => (b'n', 45),
        Key::M => (b'm', 46),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn held_keys_survive_event_drain_release_independently_and_clear_on_focus_loss() {
        let mut k = Keyboard::default();
        let mut m = crate::machine::Memory::zeroed(0x10000);
        for key in [Key::Up, Key::W, Key::Space, Key::LeftShift] {
            k.key(key, true);
        }
        k.pending.clear(); // Consuming keyDown events does not release the device.
        k.publish_keys(&mut m, true);
        assert_eq!(m.read_byte(0x175), 0x20); // W13
        assert_eq!(m.read_byte(0x17a), 2); // Space49
        assert_eq!(m.read_byte(0x17b), 1); // Shift56
        assert_eq!(m.read_byte(0x183), 0x40); // Up126
        k.key(Key::Up, false);
        k.publish_keys(&mut m, true);
        assert_eq!(m.read_byte(0x183), 0);
        assert_eq!(m.read_byte(0x175), 0x20);
        k.publish_keys(&mut m, false);
        assert!((0x174..0x184).all(|p| m.read_byte(p) == 0));
        k.publish_keys(&mut m, true);
        assert!((0x174..0x184).all(|p| m.read_byte(p) == 0));
    }
    #[test]
    fn fast_pointer_edges_keep_their_coordinates_and_order() {
        let mut k = Keyboard::default();
        k.pointer(30.0, 50.0, -1, false);
        k.pointer(30.0, 50.0, 0, true);
        k.pointer(60.0, 50.0, -1, false);
        k.pointer(60.0, 50.0, 0, false);
        assert_eq!(
            k.pending
                .iter()
                .map(|e| (e.what, e.h, e.v, e.modifiers))
                .collect::<Vec<_>>(),
            vec![
                (0, 30, 50, 0x80),
                (1, 30, 50, 0),
                (0, 60, 50, 0),
                (2, 60, 50, 0x80)
            ]
        );
    }
    #[test]
    fn command_modifier_survives_release_before_frame_poll() {
        let mut k = Keyboard::default();
        k.key(Key::LeftSuper, true);
        k.key(Key::N, true);
        k.character('n' as u32);
        k.key(Key::N, false);
        k.key(Key::LeftSuper, false);
        assert_eq!(k.modifiers(), 0x80);
        assert_eq!(k.pending.len(), 1);
        let e = k.pending.pop_front().unwrap();
        assert_eq!(e.message, 0x2d6e);
        assert_eq!(e.modifiers, 0x180);
    }
    #[test]
    fn character_input_uses_layout_text_and_special_keys_do_not_duplicate() {
        let mut k = Keyboard::default();
        k.key(Key::LeftShift, true);
        k.key(Key::E, true);
        k.character('É' as u32);
        k.key(Key::E, false);
        k.key(Key::LeftShift, false);
        k.key(Key::Enter, true);
        k.character(13);
        assert_eq!(k.pending.len(), 2);
        assert_eq!(k.pending[0].message as u8, 0x83);
        assert_eq!(k.pending[0].modifiers, 0x280);
        assert_eq!(k.pending[1].message, 0x240d);
    }
}
