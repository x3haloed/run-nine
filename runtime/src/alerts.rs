//! Resource-backed modal alerts. Results come from real events or guest filters.
use crate::{
    callbacks::{Arg, Callbacks},
    controls::Controls,
    dialogs::Dialogs,
    events::{Event, Events},
    fonts::Fonts,
    managers::Heap,
    quickdraw::{read_rect, QuickDraw},
    resource::Resource,
    windows::Windows,
};
use m68k::{AddressBus, CpuCore};
use std::collections::VecDeque;
struct Active {
    sp: u32,
    opcode: u16,
    dialog: u32,
    old_port: u32,
    old_front: u32,
    pixels: Vec<u8>,
    filter: u32,
    buffer: u32,
    filter_waiting: bool,
    filter_result: Option<bool>,
    pressed: Option<usize>,
    update: bool,
}
pub struct Alerts {
    active: Option<Active>,
    pub trace: bool,
}
impl Alerts {
    pub fn new() -> Self {
        Self {
            active: None,
            trace: true,
        }
    }
    pub fn is_active(&self) -> bool {
        self.active.is_some()
    }
    pub fn waiting_for_input(&self) -> bool {
        self.active.as_ref().is_some_and(|a| a.filter == 0)
    }
    pub fn callback_result(&mut self, result: i16) -> Result<(), String> {
        let active = self
            .active
            .as_mut()
            .ok_or("Alert filter returned without an alert")?;
        if !active.filter_waiting || active.filter_result.is_some() {
            return Err("Unexpected alert filter return".into());
        }
        active.filter_result = Some(result != 0);
        Ok(())
    }
    fn symbol<B: AddressBus>(m: &mut B, q: &QuickDraw, d: u32, opcode: u16) -> Result<(), String> {
        if opcode == 0xa984 {
            return Ok(());
        }
        let id = match opcode {
            0xa985 => 0,
            0xa986 => 2,
            _ => 1,
        };
        let bitmap = crate::icons::alert_bitmap(id).ok_or("Unknown alert symbol")?;
        for y in 0..32 {
            for x in 0..32 {
                let bit = bitmap[y * 4 + x / 8] & (0x80 >> (x & 7)) != 0;
                q.transfer(m, d, x as i16 + 20, y as i16 + 10, bit, 0)?;
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<B: AddressBus>(
        &mut self,
        opcode: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        dialogs: &mut Dialogs,
        callbacks: &mut Callbacks,
        resources: &[Resource<'_>],
        script: &mut VecDeque<Event>,
    ) -> Result<bool, String> {
        if !matches!(opcode, 0xa984..=0xa987) {
            return Ok(false);
        }
        if self.active.is_none() {
            let sp = cpu.dar[15];
            let old_port = q.get_port(m)?;
            let old_front = windows.front();
            let pixels = (0..q.row_bytes() as u32 * q.height as u32)
                .map(|i| m.read_byte(q.screen_base + i))
                .collect();
            let id = m.read_word(sp + 4) as i16;
            let filter = m.read_long(sp);
            let buffer = if filter != 0 {
                heap.new_ptr(m, 18, true)
                    .map_err(|e| format!("Alert filter buffer: {e}"))?
            } else {
                0
            };
            let dialog = dialogs
                .create_alert(m, heap, q, fonts, windows, controls, events, resources, id)?;
            q.set_port(m, dialog)?;
            if !dialogs.records[&dialog]
                .items
                .iter()
                .any(|i| i.kind & 0x7f == 32)
            {
                Self::symbol(m, q, dialog, opcode)?;
            }
            self.active = Some(Active {
                sp,
                opcode,
                dialog,
                old_port,
                old_front,
                pixels,
                filter,
                buffer,
                filter_waiting: false,
                filter_result: None,
                pressed: None,
                update: false,
            });
            if self.trace {
                println!("Alert opened: id={id} opcode={opcode:#x} dialog={dialog:#x}");
            }
            return Ok(true);
        }
        let a = self.active.as_mut().unwrap();
        if a.sp != cpu.dar[15] || a.opcode != opcode {
            return Err("Nested alerts are not implemented".into());
        }
        if a.update {
            windows.end_update(m, heap, a.dialog)?;
            a.update = false;
        }
        if dialogs.has_pending_draws() {
            return Ok(true);
        }
        let mut hit = 0i16;
        let event = if a.filter_waiting {
            let Some(handled) = a.filter_result.take() else {
                return Ok(true);
            };
            a.filter_waiting = false;
            if handled {
                hit = m.read_word(a.buffer + 16) as i16;
            }
            Event {
                what: if handled { 0 } else { m.read_word(a.buffer) },
                message: m.read_long(a.buffer + 2),
                when: m.read_long(a.buffer + 6),
                v: m.read_word(a.buffer + 10) as i16,
                h: m.read_word(a.buffer + 12) as i16,
                modifiers: m.read_word(a.buffer + 14),
            }
        } else {
            let event = script
                .pop_front()
                .unwrap_or_else(|| events.next(0xffff, true).1);
            if a.filter != 0 {
                let b = a.buffer;
                m.write_word(b, event.what);
                m.write_long(b + 2, event.message);
                m.write_long(b + 6, event.when);
                m.write_word(b + 10, event.v as u16);
                m.write_word(b + 12, event.h as u16);
                m.write_word(b + 14, event.modifiers);
                m.write_word(b + 16, 0);
                q.set_port(m, a.dialog)?;
                a.filter_waiting = true;
                callbacks.call_byte(
                    cpu,
                    m,
                    a.filter,
                    &[Arg::Long(a.dialog), Arg::Long(b), Arg::Long(b + 16)],
                    6,
                )?;
                return Ok(true);
            }
            event
        };
        let enabled = |index: usize, m: &mut B| -> bool {
            dialogs
                .records
                .get(&a.dialog)
                .and_then(|d| d.items.get(index))
                .map(|item| {
                    let data = m.read_long(item.handle);
                    item.kind == 4 && m.read_byte(data + 17) != 255
                })
                .unwrap_or(false)
        };
        if hit == 0 {
            match event.what {
                3 | 5 => {
                    let key = event.message as u8;
                    let index = if key == 13 || key == 3 {
                        m.read_word(a.dialog + 168) as usize
                    } else if key == 27 {
                        2
                    } else {
                        0
                    };
                    if index > 0 && enabled(index - 1, m) {
                        hit = index as i16;
                    }
                }
                1 | 2 => {
                    let bitmap = q.port_bitmap(m, a.dialog)?;
                    let origin = read_rect(m, bitmap + 6);
                    let (v, h) = (event.v + origin.top, event.h + origin.left);
                    let under = dialogs.records[&a.dialog]
                        .items
                        .iter()
                        .enumerate()
                        .find_map(|(i, item)| {
                            if item.rect.contains(v, h) && enabled(i, m) {
                                Some(i)
                            } else {
                                None
                            }
                        });
                    if event.what == 1 {
                        a.pressed = under;
                    } else if let Some(pressed) = a.pressed.take() {
                        if under == Some(pressed) {
                            hit = pressed as i16 + 1;
                        }
                    }
                }
                6 if event.message == a.dialog => {
                    windows.begin_update(m, heap, q, a.dialog)?;
                    dialogs.draw(m, heap, q, fonts, controls, a.dialog)?;
                    if !dialogs.records[&a.dialog]
                        .items
                        .iter()
                        .any(|i| i.kind & 0x7f == 32)
                    {
                        Self::symbol(m, q, a.dialog, opcode)?;
                    }
                    if dialogs.has_pending_draws() {
                        a.update = true;
                    } else {
                        windows.end_update(m, heap, a.dialog)?;
                    }
                }
                _ => {}
            }
        }
        if hit <= 0 {
            return Ok(true);
        }
        if hit as usize > dialogs.records[&a.dialog].items.len() {
            return Err("Alert filter returned an out-of-range item".into());
        }
        let a = self.active.take().unwrap();
        dialogs.dispose(m, heap, q, fonts, windows, controls, events, a.dialog)?;
        if a.old_front != 0 && windows.windows.iter().any(|w| w.address == a.old_front) {
            windows.select(m, q, fonts, events, a.old_front)?;
        }
        for (i, value) in a.pixels.iter().enumerate() {
            m.write_byte(q.screen_base + i as u32, *value);
        }
        q.set_port(m, a.old_port)?;
        if a.buffer != 0 {
            heap.dispose_ptr(a.buffer).map_err(|e| e.to_string())?;
        }
        m.write_word(a.sp + 6, hit as u16);
        cpu.dar[15] = a.sp + 6;
        cpu.pc += 2;
        if self.trace {
            println!("Alert closed: result={hit}");
        }
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn exercise(mouse: bool) {
        let bytes = include_bytes!("../../probes/alerts/resources.rsrc");
        let resources = crate::resource::parse(bytes).unwrap();
        let mut m = m68k::LinearMemoryBus::new(8 * 1024 * 1024);
        let mut heap = Heap::new(0x20000, 0xe0000);
        let mut q = QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let mut fonts = Fonts::new();
        fonts.init().unwrap();
        let mut windows = Windows::new();
        let mut events = Events::new();
        windows.init(&mut m, &mut heap, &q).unwrap();
        let owner = windows
            .create(
                &mut m,
                &mut heap,
                &q,
                &fonts,
                &mut events,
                0,
                crate::managers::Rect {
                    top: 60,
                    left: 90,
                    bottom: 220,
                    right: 460,
                },
                b"Owner",
                true,
                0,
                u32::MAX,
                true,
                0,
            )
            .unwrap();
        q.set_port(&mut m, owner).unwrap();
        let before = (0..q.row_bytes() as u32 * q.height as u32)
            .map(|i| m.read_byte(q.screen_base + i))
            .collect::<Vec<_>>();
        let mut controls = Controls::new();
        let mut dialogs = Dialogs::new();
        let mut alerts = Alerts::new();
        alerts.trace = false;
        let mut callbacks = Callbacks::silent();
        let mut script = VecDeque::new();
        let mut cpu = CpuCore::new();
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_long(0x700000, 0);
        m.write_word(0x700004, 128);
        m.write_word(0x700006, 0x5555);
        m.write_long(0x700008, 0xdeadbeef);
        macro_rules! step {
            () => {
                alerts
                    .dispatch(
                        0xa984,
                        &mut cpu,
                        &mut m,
                        &mut heap,
                        &q,
                        &fonts,
                        &mut windows,
                        &mut controls,
                        &mut events,
                        &mut dialogs,
                        &mut callbacks,
                        &resources,
                        &mut script,
                    )
                    .unwrap()
            };
        }
        assert!(step!());
        for _ in 0..100 {
            assert!(step!());
        }
        assert_eq!(
            (cpu.pc, cpu.dar[15], m.read_word(0x700006)),
            (0x6000, 0x700000, 0x5555)
        );
        if mouse {
            // Release outside the pressed button must keep the modal open.
            for (what, v, h) in [(1, 145, 375), (2, 220, 500)] {
                script.push_back(Event {
                    what,
                    v,
                    h,
                    ..Event::default()
                });
                assert!(step!());
            }
            assert_eq!(cpu.pc, 0x6000);
            // The second button reports only after a matching press/release.
            script.push_back(Event {
                what: 1,
                v: 145,
                h: 285,
                ..Event::default()
            });
            assert!(step!());
            assert_eq!(cpu.pc, 0x6000);
            script.push_back(Event {
                what: 2,
                v: 145,
                h: 285,
                ..Event::default()
            });
            assert!(step!());
        } else {
            script.push_back(Event {
                what: 3,
                message: b'x' as u32,
                ..Event::default()
            });
            assert!(step!());
            assert_eq!(cpu.pc, 0x6000);
            script.push_back(Event {
                what: 3,
                message: 13,
                ..Event::default()
            });
            assert!(step!());
        }
        assert_eq!(
            (cpu.pc, cpu.dar[15], m.read_word(0x700006)),
            (0x6002, 0x700006, if mouse { 2 } else { 1 })
        );
        assert_eq!(m.read_long(0x700008), 0xdeadbeef);
        assert_eq!(q.get_port(&mut m).unwrap(), owner);
        assert_eq!(windows.front(), owner);
        assert_eq!(windows.windows.len(), 1);
        assert_eq!(
            before,
            (0..q.row_bytes() as u32 * q.height as u32)
                .map(|i| m.read_byte(q.screen_base + i))
                .collect::<Vec<_>>()
        );
    }
    #[test]
    fn waits_for_matching_mouse_release_and_restores_owner() {
        exercise(true)
    }
    #[test]
    fn waits_for_real_default_key_and_restores_owner() {
        exercise(false)
    }
}
