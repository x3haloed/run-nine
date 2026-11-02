//! ModalDialog continues on the application CPU; filters and control tracking
//! execute through the same Pascal callback and Toolbox paths as other callers.
use crate::{
    callbacks::{Arg, Callbacks},
    controls::Controls,
    dialogs::Dialogs,
    events::{self, Event, Events},
    fonts::Fonts,
    managers::Heap,
    quickdraw::{read_rect, QuickDraw},
    windows::Windows,
};
use m68k::{AddressBus, CpuCore};
use std::collections::VecDeque;
const TRACK_PROCEDURE: u32 = 0x18500;
struct Active {
    sp: u32,
    pc: u32,
    dialog: u32,
    old_port: u32,
    output: u32,
    filter: u32,
    buffer: u32,
    filtering: bool,
    filter_result: Option<bool>,
    tracking: Option<usize>,
    track_result: Option<i16>,
    update: bool,
}
#[derive(Default)]
pub struct ModalDialog {
    active: Option<Active>,
}
impl ModalDialog {
    pub fn callback_result(&mut self, result: i16) -> Result<(), String> {
        let a = self
            .active
            .as_mut()
            .ok_or("modal filter returned without ModalDialog")?;
        if !a.filtering || a.filter_result.is_some() {
            return Err("unexpected modal filter return".into());
        }
        a.filter_result = Some(result != 0);
        Ok(())
    }
    pub fn track_result(&mut self, result: i16) -> Result<(), String> {
        let a = self
            .active
            .as_mut()
            .ok_or("modal tracking returned without ModalDialog")?;
        if a.tracking.is_none() || a.track_result.is_some() {
            return Err("unexpected modal tracking return".into());
        }
        a.track_result = Some(result);
        Ok(())
    }
    pub fn waiting_for_input(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|a| !a.filtering && a.tracking.is_none())
    }
    fn finish<B: AddressBus>(
        &mut self,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
    ) -> Result<(), String> {
        let a = self
            .active
            .take()
            .ok_or("ModalDialog completion without continuation")?;
        heap.dispose_ptr(a.buffer).map_err(|e| e.to_string())?;
        m.write_word(0x220, 0);
        q.set_port(m, a.old_port)?;
        cpu.dar[0] = 1;
        cpu.set_ccr(0);
        cpu.dar[15] = a.sp + 8;
        cpu.pc = a.pc + 2;
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &Controls,
        events: &mut Events,
        dialogs: &mut Dialogs,
        callbacks: &mut Callbacks,
        script: &mut VecDeque<Event>,
    ) -> Result<bool, String> {
        if op != 0xa991 {
            return Ok(false);
        }
        if self.active.is_none() {
            let dialog = windows.front();
            if !dialogs.records.contains_key(&dialog) {
                return Err("ModalDialog without a front dialog".into());
            }
            let sp = cpu.dar[15];
            let output = m.read_long(sp);
            if output == 0 {
                return Err("ModalDialog with nil itemHit".into());
            }
            let filter = m.read_long(sp + 4);
            let filter = if filter == 0 {
                m.write_word(Dialogs::STANDARD_FILTER, 0xaa68);
                Dialogs::STANDARD_FILTER
            } else {
                filter
            };
            let buffer = heap
                .new_ptr(m, 16, true)
                .map_err(|e| format!("modal event buffer: {e}"))?;
            m.write_word(0x220, 0);
            self.active = Some(Active {
                sp,
                pc: cpu.pc,
                dialog,
                old_port: q.get_port(m)?,
                output,
                filter,
                buffer,
                filtering: false,
                filter_result: None,
                tracking: None,
                track_result: None,
                update: false,
            });
        }
        let a = self.active.as_mut().unwrap();
        if a.sp != cpu.dar[15] || a.pc != cpu.pc {
            return Err("nested ModalDialog is not implemented".into());
        }
        if dialogs.has_pending_draws() {
            return Ok(true);
        }
        if a.update {
            windows.end_update(m, heap, a.dialog)?;
            a.update = false;
        }
        if let Some(index) = a.tracking {
            let Some(part) = a.track_result.take() else {
                return Ok(true);
            };
            a.tracking = None;
            if part != 0 {
                m.write_word(a.output, (index + 1) as u16);
                self.finish(cpu, m, heap, q)?;
                return Ok(true);
            }
            m.write_word(a.output, 0xffff);
        }
        let event = if a.filtering {
            let Some(handled) = a.filter_result.take() else {
                return Ok(true);
            };
            a.filtering = false;
            if handled {
                self.finish(cpu, m, heap, q)?;
                return Ok(true);
            }
            Event {
                what: m.read_word(a.buffer),
                message: m.read_long(a.buffer + 2),
                when: m.read_long(a.buffer + 6),
                v: m.read_word(a.buffer + 10) as i16,
                h: m.read_word(a.buffer + 12) as i16,
                modifiers: m.read_word(a.buffer + 14),
            }
        } else {
            let event = if let Some(e) = script.pop_front() {
                e
            } else {
                windows.next_event(m, events, 0xffff, true)?.1
            };
            events::write_event(m, a.buffer, event);
            a.filtering = true;
            // The native filter receives the caller's itemHit pointer and the
            // caller's current port, including its initial itemHit contents.
            callbacks.call_byte(
                cpu,
                m,
                a.filter,
                &[
                    Arg::Long(a.dialog),
                    Arg::Long(a.buffer),
                    Arg::Long(a.output),
                ],
                9,
            )?;
            return Ok(true);
        };
        // DialogSelect's unhandled-event result is -1, even if a filter wrote
        // an item number before returning false. A true filter may return any
        // short, including zero and negative application-defined results.
        m.write_word(a.output, 0xffff);
        match event.what {
            6 if event.message == a.dialog => {
                windows.begin_update(m, heap, q, a.dialog)?;
                dialogs.draw(m, heap, q, fonts, controls, a.dialog)?;
                if dialogs.has_pending_draws() {
                    a.update = true;
                } else {
                    windows.end_update(m, heap, a.dialog)?;
                }
            }
            1 => {
                let bm = q.port_bitmap(m, a.dialog)?;
                let origin = read_rect(m, bm + 6);
                let (v, h) = (
                    event.v.wrapping_add(origin.top),
                    event.h.wrapping_add(origin.left),
                );
                let under =
                    dialogs.records[&a.dialog]
                        .items
                        .iter()
                        .enumerate()
                        .find(|(_, item)| {
                            if item.kind & 128 != 0 || !item.rect.contains(v, h) {
                                return false;
                            }
                            let basic = item.kind & 127;
                            if matches!(basic, 4 | 5 | 6) {
                                let p = m.read_long(item.handle);
                                m.read_byte(p + 16) != 0 && m.read_byte(p + 17) != 255
                            } else {
                                basic == 0
                            }
                        });
                if let Some((index, item)) = under {
                    if item.kind & 127 == 0 {
                        m.write_word(a.output, (index + 1) as u16);
                        self.finish(cpu, m, heap, q)?;
                        return Ok(true);
                    }
                    // Owned Pascal wrapper for TrackControl. Its implementation
                    // already has native hit/drag/release comparisons.
                    m.write_word(TRACK_PROCEDURE, 0x225f);
                    m.write_word(TRACK_PROCEDURE + 2, 0xa968);
                    m.write_word(TRACK_PROCEDURE + 4, 0x4ed1);
                    a.tracking = Some(index);
                    let point = (u32::from(v as u16) << 16) | u32::from(h as u16);
                    callbacks.call_word(
                        cpu,
                        m,
                        TRACK_PROCEDURE,
                        &[Arg::Long(item.handle), Arg::Long(point), Arg::Long(0)],
                        10,
                    )?;
                }
            }
            3 | 5 => {
                // Text fields require DialogSelect/TextEdit selection handling,
                // not an invented successful key result.
                if m.read_word(a.dialog + 164) as i16 >= 0 {
                    return Err(
                        "ModalDialog editing an active text field is not implemented".into(),
                    );
                }
            }
            _ => {}
        }
        Ok(true)
    }
}
