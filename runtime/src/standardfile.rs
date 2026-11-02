//! Standard File modal continuation. Application hooks run in the 68K loop.
use crate::{
    callbacks::{Arg, Callbacks},
    controls::Controls,
    dialogs::Dialogs,
    events::{Event, Events},
    files::Files,
    fonts::Fonts,
    managers::Heap,
    quickdraw::QuickDraw,
    resource::Resource,
    windows::Windows,
};
use m68k::{AddressBus, CpuCore};
use std::collections::VecDeque;
struct Put {
    entry_sp: u32,
    opening: bool,
    system_default: bool,
    choices: Vec<Vec<u8>>,
    index: Option<usize>,
    dialog: u32,
    reply: u32,
    hook: u32,
    filter: u32,
    filter_buffer: u32,
    filter_waiting: bool,
    filter_result: Option<bool>,
    filtered_event: Option<Event>,
    filter_input: Option<Event>,
    user: u32,
    old_port: u32,
    old_front: u32,
    name: Vec<u8>,
    selected: bool,
    phase: u8,
    action: i16,
    requested: Option<i16>,
    result: Option<i16>,
    waiting_logged: bool,
    pressed_item: Option<usize>,
    main_dialog: u32,
    confirming: bool,
    replace_confirmed: bool,
    confirmation_pixels: Option<Vec<u8>>,
    confirmation_old_port: u32,
    idle_tick: Option<u32>,
    update_window: Option<u32>,
}
pub struct StandardFile {
    put: Option<Put>,
    trace: bool,
}
impl StandardFile {
    fn accept(p: &mut Put, _files: &Files) -> Result<(), String> {
        if !p.name.is_empty() && p.name.len() <= 31 && !p.name.contains(&b':') {
            p.action = 1;
            p.phase = 2;
        }
        Ok(())
    }
    pub fn waiting_for_input(&self) -> bool {
        self.put.as_ref().is_some_and(|p| p.phase == 1)
    }
    fn draw_list<B: AddressBus>(
        m: &mut B,
        q: &QuickDraw,
        fonts: &Fonts,
        dialogs: &Dialogs,
        p: &Put,
    ) -> Result<(), String> {
        if p.confirming {
            return Ok(());
        }
        let d = dialogs
            .records
            .get(&p.dialog)
            .ok_or("missing file dialog")?;
        let r = d.items.get(6).ok_or("file dialog list item missing")?.rect;
        let old = q.get_port(m)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(p.dialog + i)).collect();
        q.set_port(m, p.dialog)?;
        q.set_word(m, 68, 1)?;
        q.set_word(m, 74, 12)?;
        q.set_word(m, 72, 1)?;
        let render = (|| {
            q.paint_rect(m, r, [0; 8], 8)?;
            q.frame_rect(m, r)?;
            if p.system_default {
                let separator = d
                    .items
                    .get(8)
                    .ok_or("standard Open separator missing")?
                    .rect;
                q.paint_rect(m, separator, [255; 8], 8)?;
            }

            let rows = ((r.bottom - r.top - 4) / 16).max(1) as usize;
            let first = p.index.map(|i| i.saturating_sub(rows - 1)).unwrap_or(0);
            for (i, name) in p.choices.iter().enumerate().skip(first).take(rows) {
                let top = r.top + 2 + (i - first) as i16 * 16;
                q.move_to(m, r.left + 4, top + 12)?;
                q.text(m, fonts, name)?;
                if p.index == Some(i) {
                    q.paint_rect(
                        m,
                        crate::managers::Rect {
                            top,
                            left: r.left + 1,
                            bottom: top + 16,
                            right: r.right - 1,
                        },
                        [255; 8],
                        10,
                    )?;
                }
            }
            if let Some(header) = d.items.get(7) {
                q.move_to(m, header.rect.left + 2, header.rect.top + 12)?;
                q.text(m, fonts, b"Runtime")?;
            }
            if !p.opening && p.selected && !p.name.is_empty() {
                let field = d.items.get(9).ok_or("missing file name field")?.rect;
                let width = fonts.text_width(1, 12, 0, &p.name)?;
                q.paint_rect(
                    m,
                    crate::managers::Rect {
                        top: field.top + 1,
                        left: field.left + 2,
                        bottom: field.top + 15,
                        right: (field.left + 2 + width).min(field.right - 1),
                    },
                    [255; 8],
                    10,
                )?;
            }
            Ok(())
        })();
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(p.dialog + i as u32, *b);
        }
        q.set_port(m, old)?;
        render
    }
    pub fn new() -> Self {
        Self {
            put: None,
            trace: true,
        }
    }
    pub fn callback_result(&mut self, token: u32, result: i16) -> Result<(), String> {
        if token != 1 && token != 3 {
            return Err("unknown callback owner".into());
        }
        let p = self
            .put
            .as_mut()
            .ok_or("Standard File callback without modal context")?;
        if token == 3 {
            if !p.filter_waiting || p.filter_result.is_some() {
                return Err("unexpected Standard File filter return".into());
            }
            p.filter_result = Some(result != 0);
            return Ok(());
        }
        if p.requested.is_none() {
            return Err("unexpected Standard File hook return".into());
        }
        p.result = Some(result);
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
        controls: &mut Controls,
        events: &mut Events,
        dialogs: &mut Dialogs,
        callbacks: &mut Callbacks,
        files: &Files,
        resources: &[Resource<'_>],
        script: &mut VecDeque<Event>,
    ) -> Result<bool, String> {
        if op != 0xa9ea {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let selector = m.read_word(sp);
        if let Some(active) = &self.put {
            if active.entry_sp != sp {
                return Err("reentrant Standard File dialogs are not implemented".into());
            }
        }
        if selector != 7 && selector != 8 {
            return Err(format!(
                "Standard File selector {selector} is not implemented"
            ));
        }
        if self.put.is_none() {
            if self.trace {
                println!("Standard File arguments: selector={selector} dialog={} hook={:#x} filter={:#x} active_list={:#x} activate={:#x} user={:#x}",m.read_word(sp+26) as i16,m.read_long(sp+18),m.read_long(sp+14),m.read_long(sp+10),m.read_long(sp+6),m.read_long(sp+2));
            }
            if m.read_long(sp + 6) != 0 || m.read_long(sp + 10) != 0 {
                return Err(
                    "CustomPutFile custom activation lists/callbacks are not implemented".into(),
                );
            }
            let old_port = q.get_port(m)?;
            let old_front = windows.front();
            let reply = m.read_long(sp + 28);
            let opening = selector == 8;
            let mut kinds = Vec::new();
            let name = if opening {
                if m.read_long(sp + 38) != 0 {
                    return Err("CustomGetFile file-filter callback not implemented".into());
                }
                let n = m.read_word(sp + 36) as i16;
                if n > 0 {
                    if n > 4 {
                        return Err("unsupported Standard File type-list length".into());
                    }
                    let types = m.read_long(sp + 32);
                    for i in 0..n as u32 {
                        kinds.push(m.read_long(types + i * 4).to_be_bytes());
                    }
                }
                Vec::new()
            } else {
                let default = m.read_long(sp + 32);
                let len = m.read_byte(default) as u32;
                (0..len).map(|i| m.read_byte(default + 1 + i)).collect()
            };
            let dialog_id = m.read_word(sp + 26) as i16;
            let dialog = if opening && dialog_id == 0 {
                dialogs.standard_open(m, heap, q, fonts, windows, controls, events)?
            } else {
                dialogs.create(
                    m,
                    heap,
                    q,
                    fonts,
                    windows,
                    controls,
                    events,
                    resources,
                    dialog_id,
                    u32::from_be_bytes(*b"stdf"),
                )?
            };
            if opening {
                let item = &dialogs.records[&dialog].items[0];
                let p = m.read_long(item.handle);
                m.write_byte(p + 17, 255);
            }
            if !opening {
                dialogs.set_text(m, heap, dialog, 10, &name)?;
            }
            dialogs.draw(m, heap, q, fonts, controls, dialog)?;
            self.put = Some(Put {
                entry_sp: sp,
                opening,
                system_default: opening && dialog_id == 0,
                choices: files.list(&kinds),
                index: None,
                dialog,
                reply,
                hook: m.read_long(sp + 18),
                filter: m.read_long(sp + 14),
                filter_buffer: if m.read_long(sp + 14) != 0 {
                    heap.new_ptr(m, 18, true)
                        .map_err(|e| format!("modal filter event buffer: {e}"))?
                } else {
                    0
                },
                filter_waiting: false,
                filter_result: None,
                filtered_event: None,
                filter_input: None,
                user: m.read_long(sp + 2),
                old_port,
                old_front,
                name,
                selected: true,
                phase: 0,
                action: 0,
                requested: None,
                result: None,
                waiting_logged: false,
                pressed_item: None,
                main_dialog: dialog,
                confirming: false,
                replace_confirmed: false,
                confirmation_pixels: None,
                confirmation_old_port: 0,
                idle_tick: None,
                update_window: None,
            });
            if self.trace {
                println!(
                    "Standard File: {} dialog opened",
                    if opening {
                        "CustomGetFile"
                    } else {
                        "CustomPutFile"
                    }
                );
            }
            Self::draw_list(m, q, fonts, dialogs, self.put.as_ref().unwrap())?;
        }
        let p = self.put.as_mut().unwrap();
        if let Some(window) = p.update_window.take() {
            windows.end_update(m, heap, window)?;
        }
        if let Some(requested) = p.requested {
            let Some(result) = p.result.take() else {
                return Ok(true);
            };
            if self.trace {
                println!(
                    "Standard File {}: item={requested} result={result} refcon={:#010x}",
                    if p.hook == 0 {
                        "default action"
                    } else {
                        "application hook"
                    },
                    m.read_long(p.dialog + 152)
                );
            }
            p.requested = None;
            p.phase = if requested == -1 {
                1
            } else if requested == -2 {
                4
            } else if [1, 2].contains(&result) {
                p.action = result;
                if result == 1
                    && !p.opening
                    && !p.confirming
                    && !p.replace_confirmed
                    && files.finder(&p.name).is_ok()
                {
                    5
                } else {
                    3
                }
            } else {
                1
            };
            if requested != 100 {
                dialogs.draw(m, heap, q, fonts, controls, p.dialog)?;
                Self::draw_list(m, q, fonts, dialogs, p)?;
                if dialogs.has_pending_draws() {
                    return Ok(true);
                }
            }
        }
        if p.phase == 5 {
            p.confirmation_old_port = q.get_port(m)?;
            p.confirmation_pixels = Some(
                (0..q.row_bytes() as u32 * q.height as u32)
                    .map(|i| m.read_byte(q.screen_base + i))
                    .collect(),
            );
            p.dialog =
                dialogs.replacement(m, heap, q, fonts, windows, controls, events, &p.name)?;
            p.confirming = true;
            p.action = 0;
            p.phase = 0;
            p.pressed_item = None;
            p.idle_tick = None;
            if self.trace {
                println!("Standard File: replacement confirmation opened");
            }
        }
        if p.phase == 4 && p.confirming {
            let accepted = p.action == 1;
            dialogs.dispose(m, heap, q, fonts, windows, controls, events, p.dialog)?;
            p.dialog = p.main_dialog;
            p.confirming = false;
            p.pressed_item = None;
            windows.select(m, q, fonts, events, p.dialog)?;
            if let Some(pixels) = p.confirmation_pixels.take() {
                for (i, b) in pixels.iter().enumerate() {
                    m.write_byte(q.screen_base + i as u32, *b);
                }
            }
            q.set_port(m, p.confirmation_old_port)?;
            p.action = if accepted { 1 } else { 0 };
            p.replace_confirmed = accepted;
            p.phase = if accepted { 3 } else { 1 };
            p.idle_tick = None;
            if self.trace {
                println!("Standard File: replacement confirmation closed, accepted={accepted}");
            }
            dialogs.draw(m, heap, q, fonts, controls, p.dialog)?;
            Self::draw_list(m, q, fonts, dialogs, p)?;
        }
        if p.filter_waiting {
            let Some(handled) = p.filter_result.take() else {
                return Ok(true);
            };
            p.filter_waiting = false;
            let b = p.filter_buffer;
            if handled {
                p.action = m.read_word(b + 16) as i16;
                p.phase = 2;
            } else {
                p.filtered_event = Some(Event {
                    what: m.read_word(b),
                    message: m.read_long(b + 2),
                    when: m.read_long(b + 6),
                    v: m.read_word(b + 10) as i16,
                    h: m.read_word(b + 12) as i16,
                    modifiers: m.read_word(b + 14),
                });
            }
        }
        if p.phase == 1
            && p.filter == 0
            && p.filtered_event.is_none()
            && (p.idle_tick.is_none() || (script.is_empty() && p.idle_tick != Some(events.ticks)))
        {
            p.idle_tick = Some(events.ticks);
            p.action = 100;
            p.phase = 2;
        }
        if [0, 2, 3].contains(&p.phase) {
            let item = if p.phase == 0 {
                -1
            } else if p.phase == 3 {
                -2
            } else {
                p.action
            };
            p.requested = Some(item);
            if p.hook != 0 {
                callbacks.call_word(
                    cpu,
                    m,
                    p.hook,
                    &[
                        Arg::Word(item as u16),
                        Arg::Long(p.dialog),
                        Arg::Long(p.user),
                    ],
                    1,
                )?;
            } else {
                p.result = Some(item);
            }
            return Ok(true);
        }
        if p.phase == 1 {
            let event = if let Some(event) = p.filtered_event.take() {
                event
            } else {
                let prepared = p.filter_input.take();
                let already_prepared = prepared.is_some();
                let next = prepared.or_else(|| script.pop_front()).or_else(|| {
                    let (found, event) = events.next(0xffff, true);
                    if found || p.filter != 0 {
                        Some(event)
                    } else {
                        None
                    }
                });
                let Some(event) = next else {
                    if !p.waiting_logged {
                        if self.trace {
                            println!("Standard File: waiting for filename, Return, or Escape");
                        }
                        p.waiting_logged = true;
                    }
                    return Ok(true);
                };
                if p.filter != 0 {
                    if event.what == 0 && !already_prepared {
                        p.filter_input = Some(event);
                        p.action = 100;
                        p.phase = 2;
                        return Ok(true);
                    }
                    if self.trace && event.what != 0 {
                        println!(
                            "Standard File filter event: what={} message={:#x} modifiers={:#x}",
                            event.what, event.message, event.modifiers
                        );
                    }
                    let b = p.filter_buffer;
                    m.write_word(b, event.what);
                    m.write_long(b + 2, event.message);
                    m.write_long(b + 6, event.when);
                    m.write_word(b + 10, event.v as u16);
                    m.write_word(b + 12, event.h as u16);
                    m.write_word(b + 14, event.modifiers);
                    m.write_word(b + 16, 0);
                    p.filter_waiting = true;
                    callbacks.call_byte(
                        cpu,
                        m,
                        p.filter,
                        &[
                            Arg::Long(p.dialog),
                            Arg::Long(b),
                            Arg::Long(b + 16),
                            Arg::Long(p.user),
                        ],
                        3,
                    )?;
                    return Ok(true);
                }
                event
            };
            if event.what == 6 || event.what == 8 {
                if event.what == 6 && event.message == p.dialog {
                    windows.begin_update(m, heap, q, p.dialog)?;
                    let draw = dialogs
                        .draw(m, heap, q, fonts, controls, p.dialog)
                        .and_then(|_| Self::draw_list(m, q, fonts, dialogs, p));
                    draw?;
                    if dialogs.has_pending_draws() {
                        p.update_window = Some(p.dialog);
                    } else {
                        windows.end_update(m, heap, p.dialog)?;
                    }
                }
                // System7 Standard File converts unhandled foreign updates to
                // null processing after giving the application's filter a turn.
                p.idle_tick = None;
                return Ok(true);
            }
            if event.what == 1 || event.what == 2 {
                let bitmap = q.port_bitmap(m, p.dialog)?;
                let origin = crate::quickdraw::read_rect(m, bitmap + 6);
                let (v, x) = (
                    event.v.wrapping_add(origin.top),
                    event.h.wrapping_add(origin.left),
                );
                let d = dialogs
                    .records
                    .get(&p.dialog)
                    .ok_or("missing Standard File dialog")?;
                let hit = d.items.iter().enumerate().find_map(|(i, item)| {
                    if !item.rect.contains(v, x) {
                        return None;
                    }
                    let kind = item.kind & 0x7f;
                    if matches!(kind, 4 | 5 | 6) {
                        let data = m.read_long(item.handle);
                        if m.read_byte(data + 17) == 255 {
                            return None;
                        }
                    }
                    Some(i)
                });
                if event.what == 1 {
                    p.pressed_item = hit;
                    if let Some(i) = hit {
                        let item = &d.items[i];
                        if matches!(item.kind & 0x7f, 4 | 5 | 6) {
                            let data = m.read_long(item.handle);
                            m.write_byte(data + 17, 1);
                        }
                    }
                } else if let Some(pressed) = p.pressed_item.take() {
                    let item = &d.items[pressed];
                    if matches!(item.kind & 0x7f, 4 | 5 | 6) {
                        let data = m.read_long(item.handle);
                        m.write_byte(data + 17, 0);
                    }
                    if hit == Some(pressed) {
                        let number = pressed + 1;
                        if self.trace {
                            println!("Standard File mouse item: {number}");
                        }
                        match number {
                            1 => Self::accept(p, files)?,
                            2 => {
                                p.action = 2;
                                p.phase = 2;
                            }
                            7 => {
                                let row = ((v - item.rect.top - 2) / 16).max(0) as usize;
                                if let Some(name) = p.choices.get(row) {
                                    p.index = Some(row);
                                    if p.opening {
                                        p.name = name.clone();
                                    }
                                    if p.opening {
                                        let control = m.read_long(d.items[0].handle);
                                        m.write_byte(control + 17, 0);
                                    }

                                    p.action = 7;
                                    p.phase = 2;
                                }
                            }
                            10 if !p.opening => {
                                p.selected = true;
                                p.action = 10;
                                p.phase = 2;
                            }
                            4 | 5 | 6 | 8 => return Err(
                                "Standard File volume and directory navigation is not implemented"
                                    .into(),
                            ),
                            _ if matches!(item.kind & 0x7f, 4 | 5 | 6) => {
                                p.action = number as i16;
                                p.phase = 2;
                            }
                            _ => {}
                        }
                    }
                }
                dialogs.draw(m, heap, q, fonts, controls, p.dialog)?;
                Self::draw_list(m, q, fonts, dialogs, p)?;
                return Ok(true);
            }
            if event.what == 0 {
                p.idle_tick = None;
                return Ok(true);
            }
            if event.what != 3 {
                return Err("Standard File event type is not implemented".into());
            }
            let key = event.message as u8;
            if self.trace {
                println!("Standard File input: key={key:#x}");
            }
            if p.confirming {
                match key {
                    13 | 3 => {
                        p.action = 1;
                        p.phase = 2;
                    }
                    27 => {
                        p.action = 2;
                        p.phase = 2;
                    }
                    _ => {}
                }
                return Ok(true);
            }
            if key == 27 {
                p.action = 2;
                p.phase = 2;
            } else if key == 13 || key == 3 {
                Self::accept(p, files)?;
            } else if p.opening && (key == 0x1e || key == 0x1f) {
                if !p.choices.is_empty() {
                    let index = match p.index {
                        None => 0,
                        Some(i) => {
                            if key == 0x1e {
                                i.saturating_sub(1)
                            } else {
                                (i + 1).min(p.choices.len() - 1)
                            }
                        }
                    };
                    p.index = Some(index);
                    p.name = p.choices[index].clone();
                    let control = m.read_long(dialogs.records[&p.dialog].items[0].handle);
                    m.write_byte(control + 17, 0);
                }
                p.action = 7;
                p.phase = 2;
            } else if p.opening {
                return Err(
                    "Standard File Open currently supports arrow selection, Return and Escape"
                        .into(),
                );
            } else if event.modifiers & 0x100 != 0 && key.eq_ignore_ascii_case(&b'a') {
                p.selected = true;
            } else if key == 8 {
                if p.selected {
                    p.name.clear();
                    p.selected = false;
                } else {
                    p.name.pop();
                }
                p.action = 10;
                p.phase = 2;
            } else if key >= 32 && event.modifiers & 0x100 == 0 {
                if p.selected {
                    p.name.clear();
                    p.selected = false;
                }
                if p.name.len() < 31 {
                    p.name.push(key);
                }
                p.action = 10;
                p.phase = 2;
            } else {
                return Err(format!(
                    "Standard File key {key:#x} with modifiers {:#x} is not implemented",
                    event.modifiers
                ));
            }
            if !p.opening {
                dialogs.set_text(m, heap, p.dialog, 10, &p.name)?;
            }
            dialogs.draw(m, heap, q, fonts, controls, p.dialog)?;
            Self::draw_list(m, q, fonts, dialogs, p)?;
            return Ok(true);
        }
        if p.phase == 4 {
            for i in 0..88 {
                m.write_byte(p.reply + i, 0);
            }
            if !p.opening {
                m.write_word(p.reply + 76, 0xffff);
            }
            m.write_word(p.reply + 6, 0xffff);
            m.write_long(p.reply + 8, 2);
            m.write_byte(p.reply + 12, p.name.len() as u8);
            for (i, b) in p.name.iter().enumerate() {
                m.write_byte(p.reply + 13 + i as u32, *b);
            }
            if p.action == 1 {
                m.write_byte(p.reply, 1);
                if p.opening {
                    if let Ok(info) = files.finder(&p.name) {
                        for (i, b) in info[..4].iter().enumerate() {
                            m.write_byte(p.reply + 2 + i as u32, *b);
                        }
                    }
                } else {
                    m.write_byte(p.reply + 1, p.replace_confirmed as u8);
                }
                m.write_word(0x214, 1);
                m.write_long(0x398, 2);
            }
            if p.filter_buffer != 0 {
                heap.dispose_ptr(p.filter_buffer)
                    .map_err(|e| e.to_string())?;
            }
            let dialog = p.dialog;
            let old_port = p.old_port;
            let old_front = p.old_front;
            let accepted = p.action == 1;
            let pop = if p.opening { 42 } else { 40 };
            dialogs.dispose(m, heap, q, fonts, windows, controls, events, dialog)?;
            if old_front != 0 {
                windows.select(m, q, fonts, events, old_front)?;
            }
            q.set_port(m, old_port)?;
            if windows.windows.iter().any(|w| w.address == old_port) {
                let r = crate::quickdraw::read_rect(m, old_port + 16);
                windows.invalidate(m, heap, q, events, r)?;
            }
            self.put = None;
            cpu.dar[15] += pop;
            cpu.pc += 2;
            if self.trace {
                println!("Standard File: dialog closed, accepted={accepted}");
            }
            return Ok(true);
        }
        Ok(true)
    }
}

/// Replay the native probe's three button-choice scenarios through production
/// Standard File, with a real 68K identity hook and application-memory replies.
pub fn probe<B: AddressBus>(m: &mut B, resources: &[Resource<'_>]) -> Result<String, String> {
    use crate::{files::Fork, quickdraw::read_rect};
    use serde_json::{json, Map};
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(m, &mut heap, &q)?;
    let mut controls = Controls::new();
    let mut dialogs = Dialogs::new();
    let mut files = Files::new();
    let mut info = [0; 16];
    info[..8].copy_from_slice(b"TEXTttxt");
    files
        .create(b"existing.txt", info)
        .map_err(|e| e.to_string())?;
    let file = files
        .open(b"existing.txt", Fork::Data, 0)
        .map_err(|e| e.to_string())?;
    files.write(file, b"ORIGINAL").map_err(|e| e.to_string())?;
    files.close(file).map_err(|e| e.to_string())?;
    let mut observations = Map::new();
    let mut replacement = None;
    let trap = 0x19000;
    let hook = 0x19010;
    // Pascal identity hook: return the item number, discard ten argument bytes.
    for (i, word) in [0x3f6f, 0x000c, 0x000e, 0x205f, 0xdefc, 0x000a, 0x4ed0]
        .iter()
        .enumerate()
    {
        m.write_word(hook + i as u32 * 2, *word);
    }
    for (label, name, keys) in [
        ("new", b"fresh.txt".as_slice(), vec![13u8]),
        ("replace_accept", b"existing.txt".as_slice(), vec![13, 13]),
        (
            "replace_cancel",
            b"existing.txt".as_slice(),
            vec![13, 27, 27],
        ),
    ] {
        let mut standard = StandardFile {
            put: None,
            trace: false,
        };
        let mut callbacks = Callbacks::silent();
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(m68k::CpuType::M68020);
        let sp = 0x7e0000;
        cpu.dar[15] = sp;
        cpu.pc = trap;
        let reply = 0x110400;
        let filename = 0x110100;
        m.write_byte(filename, name.len() as u8);
        for (i, b) in name.iter().enumerate() {
            m.write_byte(filename + 1 + i as u32, *b);
        }
        for i in 0..88 {
            m.write_byte(reply + i, 0x5a);
        }
        for i in 0..40 {
            m.write_byte(sp + i, 0);
        }
        m.write_word(sp, 7);
        m.write_long(sp + 18, hook);
        m.write_long(sp + 22, u32::MAX);
        m.write_word(sp + 26, 399);
        m.write_long(sp + 28, reply);
        m.write_long(sp + 32, filename);
        let mut script: VecDeque<_> = keys
            .into_iter()
            .map(|key| Event {
                what: 3,
                message: key as u32,
                modifiers: 0x80,
                ..Event::default()
            })
            .collect();
        let mut history = Vec::new();
        let mut finished = false;
        for _ in 0..10000 {
            // Stress modal scheduling: ticks advance even while a hook runs.
            // Pending button input must still make progress without starvation.
            events.ticks = events.ticks.wrapping_add(1);
            if let Some((token, result)) = callbacks.finish(&mut cpu, m)? {
                standard.callback_result(token, result)?;
                continue;
            }
            if cpu.pc == trap {
                standard.dispatch(
                    0xa9ea,
                    &mut cpu,
                    m,
                    &mut heap,
                    &q,
                    &fonts,
                    &mut windows,
                    &mut controls,
                    &mut events,
                    &mut dialogs,
                    &mut callbacks,
                    &files,
                    resources,
                    &mut script,
                )?;
                if cpu.pc == trap + 2 {
                    finished = true;
                    break;
                }
                continue;
            }
            if cpu.pc == hook {
                let dialog = m.read_long(cpu.dar[15] + 8);
                let item = m.read_word(cpu.dar[15] + 12) as i16;
                let refcon = m.read_long(dialog + 152);
                history.push(json!([refcon, item]));
                if refcon == u32::from_be_bytes(*b"rplc") && item == -1 {
                    let r = read_rect(m, dialog + 16);
                    let mut items = Vec::new();
                    for item in &dialogs.records[&dialog].items {
                        let r = item.rect;
                        let data = m.read_long(item.handle);
                        let (p, n) = if item.kind & 0x7f == 4 {
                            (data + 41, m.read_byte(data + 40) as u32)
                        } else {
                            (data, heap.size(item.handle).map_err(|e| e.to_string())?)
                        };
                        let text: Vec<u8> = (0..n).map(|i| m.read_byte(p + i)).collect();
                        items.push(json!({"kind":item.kind,"rect":[r.top,r.left,r.bottom,r.right],"text":text}));
                    }
                    replacement =
                        Some(json!({"bounds":[r.top,r.left,r.bottom,r.right],"items":items}));
                }
            }
            let pc = cpu.pc;
            let step = cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!(
                    "Save-confirm identity hook CPU stopped at {pc:#x}: {step:?}"
                ));
            }
        }
        if !finished || !script.is_empty() || cpu.dar[15] != sp + 40 {
            return Err("Save-confirm scenario did not complete".into());
        }
        let n = m.read_byte(reply + 12) as u32;
        let name: Vec<u8> = (0..n).map(|i| m.read_byte(reply + 13 + i)).collect();
        observations.insert(label.into(),json!({"good":m.read_byte(reply),"replacing":m.read_byte(reply+1),"type":m.read_long(reply+2),"script":m.read_word(reply+76) as i16,"flags":m.read_word(reply+78) as i16,"folder_volume":[m.read_byte(reply+80),m.read_byte(reply+81)],"name":name,"same_parent":(m.read_word(reply+6)==0xffff&&m.read_long(reply+8)==2) as u8,"hook":history}));
    }
    observations.insert(
        "original_unchanged".into(),
        json!(
            (files
                .fork_bytes(b"existing.txt", Fork::Data)
                .map_err(|e| e.to_string())?
                == b"ORIGINAL") as u8
        ),
    );
    observations.insert(
        "replace_dialog".into(),
        replacement.ok_or("replacement dialog was not exercised")?,
    );
    Ok(json!({"schema":1,"probe":"save-confirm-v1","save":observations}).to_string())
}

/// Owned native filter scenarios, replayed through real 68K Pascal callbacks.
/// This observer reads actual callback arguments and post-execution memory.
pub fn modal_filter_probe<B: AddressBus>(
    m: &mut B,
    resources: &[Resource<'_>],
    hook_code: &[u8],
    filter_code: &[u8],
) -> Result<String, String> {
    use serde_json::json;
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(m, &mut heap, &q)?;
    let mut controls = Controls::new();
    let mut dialogs = Dialogs::new();
    let files = Files::new();
    let mut rows = Vec::new();
    let trap = 0x19000;
    let hook = 0x19100;
    let filter = 0x19200;
    for (base, code) in [(hook, hook_code), (filter, filter_code)] {
        for (i, b) in code.iter().enumerate() {
            m.write_byte(base + i as u32, *b);
        }
    }
    for mode in 0..3u16 {
        let state = 0x110500;
        let reply = 0x110400;
        let filename = 0x110100;
        for i in 0..32 {
            m.write_byte(state + i, 0);
        }
        m.write_word(state, mode);
        m.write_long(state + 10, 0x13572468);
        m.write_byte(filename, 10);
        for (i, b) in b"filter.txt".iter().enumerate() {
            m.write_byte(filename + 1 + i as u32, *b);
        }
        for i in 0..88 {
            m.write_byte(reply + i, 0x5a);
        }
        let sp = 0x7e0000;
        for i in 0..40 {
            m.write_byte(sp + i, 0);
        }
        m.write_word(sp, 7);
        m.write_long(sp + 2, state);
        m.write_long(sp + 14, filter);
        m.write_long(sp + 18, hook);
        m.write_long(sp + 22, u32::MAX);
        m.write_word(sp + 26, 399);
        m.write_long(sp + 28, reply);
        m.write_long(sp + 32, filename);
        let mut cpu = CpuCore::new();
        cpu.set_cpu_type(m68k::CpuType::M68020);
        cpu.pc = trap;
        cpu.dar[15] = sp;
        let mut standard = StandardFile {
            put: None,
            trace: false,
        };
        let mut callbacks = Callbacks::silent();
        let mut script = VecDeque::new();
        let mut calls = Vec::new();
        let mut idles = 0;
        let mut pending = None;
        let mut finished = false;
        for _ in 0..20000 {
            events.ticks = events.ticks.wrapping_add(1);
            if let Some((token, result)) = callbacks.finish(&mut cpu, m)? {
                if token == 3 {
                    if let Some((refcon, user)) = pending.take() {
                        calls.push(json!([2, m.read_word(state + 2), refcon, user]));
                    }
                }
                standard.callback_result(token, result)?;
                continue;
            }
            if cpu.pc == trap {
                standard.dispatch(
                    0xa9ea,
                    &mut cpu,
                    m,
                    &mut heap,
                    &q,
                    &fonts,
                    &mut windows,
                    &mut controls,
                    &mut events,
                    &mut dialogs,
                    &mut callbacks,
                    &files,
                    resources,
                    &mut script,
                )?;
                if cpu.pc == trap + 2 {
                    finished = true;
                    break;
                }
                continue;
            }
            if cpu.pc == hook {
                let entry = cpu.dar[15];
                let user = m.read_long(entry + 4);
                let dialog = m.read_long(entry + 8);
                let item = m.read_word(entry + 12) as i16;
                if item == 100 {
                    idles += 1;
                } else {
                    calls.push(json!([
                        1,
                        item,
                        (m.read_long(dialog + 152) == u32::from_be_bytes(*b"stdf")) as u8,
                        (user == state && m.read_long(user + 10) == 0x13572468) as u8
                    ]));
                }
            }
            if cpu.pc == filter {
                let entry = cpu.dar[15];
                let event = m.read_long(entry + 12);
                if m.read_word(event) == 0 {
                    let user = m.read_long(entry + 4);
                    let dialog = m.read_long(entry + 16);
                    pending = Some((
                        (m.read_long(dialog + 152) == u32::from_be_bytes(*b"stdf")) as u8,
                        (user == state && m.read_long(user + 10) == 0x13572468) as u8,
                    ));
                }
            }
            let pc = cpu.pc;
            let step = cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!(
                    "modal filter fixture CPU stopped at {pc:#x}: {step:?}"
                ));
            }
        }
        if !finished || cpu.dar[15] != sp + 40 || pending.is_some() {
            return Err("modal filter scenario did not complete".into());
        }
        let name: Vec<_> = (0..m.read_byte(reply + 12) as u32)
            .map(|i| m.read_byte(reply + 13 + i))
            .collect();
        rows.push(json!({"mode":mode,"good":m.read_byte(reply),"null_calls":m.read_word(state+2),"idle_hooks":idles,"name":name,"calls":calls}));
    }
    let d = dialogs.create(
        m,
        &mut heap,
        &q,
        &fonts,
        &mut windows,
        &mut controls,
        &mut events,
        resources,
        1001,
        0,
    )?;
    fn item_api<B: AddressBus>(
        dialogs: &mut Dialogs,
        m: &mut B,
        heap: &mut Heap,
        d: u32,
        set: Option<(u16, u32, crate::managers::Rect)>,
    ) -> Result<(), String> {
        let sp = 0x7d0000;
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = sp;
        let (opcode, size) = if let Some((kind, handle, rect)) = set {
            crate::quickdraw::write_rect(m, 0x110600, rect);
            m.write_long(sp, 0x110600);
            m.write_long(sp + 4, handle);
            m.write_word(sp + 8, kind);
            m.write_word(sp + 10, 12);
            m.write_long(sp + 12, d);
            (0xa98e, 16)
        } else {
            m.write_long(sp, 0x110600);
            m.write_long(sp + 4, 0x110610);
            m.write_long(sp + 8, 0x110614);
            m.write_word(sp + 12, 12);
            m.write_long(sp + 14, d);
            (0xa98d, 18)
        };
        if !dialogs.dispatch(opcode, &mut cpu, m, heap)?
            || cpu.pc != 0x19002
            || cpu.dar[15] != sp + size
        {
            return Err("dialog item Pascal ABI mismatch".into());
        }
        Ok(())
    }
    fn item_state<B: AddressBus>(
        dialogs: &mut Dialogs,
        m: &mut B,
        heap: &mut Heap,
        d: u32,
        expected: u32,
    ) -> Result<serde_json::Value, String> {
        item_api(dialogs, m, heap, d, None)?;
        let kind = m.read_word(0x110614);
        let handle = m.read_long(0x110610);
        let rect = crate::quickdraw::read_rect(m, 0x110600);
        let list = m.read_long(d + 156);
        let data = m.read_long(list);
        let mut offset = 2;
        for _ in 1..12 {
            let n = m.read_byte(data + offset + 13) as u32;
            offset += 14 + n + (n & 1);
        }
        let raw_rect = crate::quickdraw::read_rect(m, data + offset + 4);
        let memory_matches = m.read_long(data + offset) == handle
            && m.read_byte(data + offset + 12) == kind as u8
            && raw_rect == rect;
        Ok(
            json!({"kind":kind,"rect":[rect.top,rect.left,rect.bottom,rect.right],"handle_matches":(handle==expected) as u8,"memory_matches":memory_matches as u8}),
        )
    }
    item_api(&mut dialogs, m, &mut heap, d, None)?;
    let old_kind = m.read_word(0x110614);
    let old_handle = m.read_long(0x110610);
    let old_rect = crate::quickdraw::read_rect(m, 0x110600);
    let initial = item_state(&mut dialogs, m, &mut heap, d, old_handle)?;
    item_api(
        &mut dialogs,
        m,
        &mut heap,
        d,
        Some((
            128,
            0x19a00,
            crate::managers::Rect {
                top: 40,
                left: 55,
                bottom: 65,
                right: 125,
            },
        )),
    )?;
    let changed = item_state(&mut dialogs, m, &mut heap, d, 0x19a00)?;
    item_api(
        &mut dialogs,
        m,
        &mut heap,
        d,
        Some((old_kind, old_handle, old_rect)),
    )?;
    let restored = item_state(&mut dialogs, m, &mut heap, d, old_handle)?;
    dialogs.dispose(
        m,
        &mut heap,
        &q,
        &fonts,
        &mut windows,
        &mut controls,
        &mut events,
        d,
    )?;
    Ok(json!({"schema":1,"probe":"modal-filter-v1","dialogs":{"cases":rows,"item_changes":{"initial":initial,"changed":changed,"restored":restored}}}).to_string())
}

pub fn standard_filter_probe<B: AddressBus>(
    m: &mut B,
    resources: &[Resource<'_>],
) -> Result<String, String> {
    use serde_json::json;
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(m, &mut heap, &q)?;
    let mut controls = Controls::new();
    let mut dialogs = Dialogs::new();
    // Stop in the first null-event filter callback of the actual modal
    // package, matching the native fixture's context before the API matrix.
    let mut standard = StandardFile {
        put: None,
        trace: false,
    };
    let mut callbacks = Callbacks::silent();
    let files = Files::new();
    let mut script = VecDeque::new();
    let mut modal_cpu = CpuCore::new();
    modal_cpu.set_cpu_type(m68k::CpuType::M68020);
    let entry = 0x7e0000;
    modal_cpu.pc = 0x19000;
    modal_cpu.dar[15] = entry;
    let filter = 0x19b00;
    for (i, word) in [
        0x206fu16, 0x000c, 0x4a50, 0x660e, 0x206f, 0x0008, 0x30bc, 0x0002, 0x1f7c, 0x0001, 0x0014,
        0x205f, 0xdefc, 0x0010, 0x4ed0,
    ]
    .iter()
    .enumerate()
    {
        m.write_word(filter + 2 * i as u32, *word);
    }
    for i in 0..40 {
        m.write_byte(entry + i, 0);
    }
    m.write_word(entry, 7);
    m.write_long(entry + 14, filter);
    m.write_long(entry + 22, u32::MAX);
    m.write_word(entry + 26, 399);
    m.write_long(entry + 28, 0x110400);
    m.write_long(entry + 32, 0x110500);
    m.write_byte(0x110500, 10);
    for (i, b) in b"filter.txt".iter().enumerate() {
        m.write_byte(0x110501 + i as u32, *b);
    }
    let mut context = false;
    for _ in 0..20000 {
        if let Some((token, result)) = callbacks.finish(&mut modal_cpu, m)? {
            standard.callback_result(token, result)?;
            continue;
        }
        if modal_cpu.pc == filter {
            let ep = m.read_long(modal_cpu.dar[15] + 12);
            if m.read_word(ep) == 0 {
                context = true;
                break;
            }
        }
        if modal_cpu.pc == 0x19000 {
            standard.dispatch(
                0xa9ea,
                &mut modal_cpu,
                m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                &files,
                resources,
                &mut script,
            )?;
        } else {
            let step = modal_cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!(
                    "standard-filter context fixture CPU stopped: {step:?}"
                ));
            }
        }
    }
    if !context {
        return Err("standard-filter modal context not reached".into());
    }
    let d = standard.put.as_ref().ok_or("missing modal context")?.dialog;
    let sp = 0x7d0000;
    let output = 0x110000;
    let mut cpu = CpuCore::new();
    cpu.pc = 0x19000;
    cpu.dar[15] = sp;
    cpu.dar[0] = 0x0203;
    m.write_long(sp, output);
    m.write_word(sp + 4, 0xa5a5);
    if !dialogs.dispatch_filter(0xaa68, &mut cpu, m, &mut heap, &q, &fonts, &controls)?
        || cpu.pc != 0x19002
        || cpu.dar[15] != sp + 4
    {
        return Err("GetStdFilterProc Pascal ABI mismatch".into());
    }
    let error = m.read_word(sp + 4) as i16;
    let proc = m.read_long(output);
    let mut rows = Vec::new();
    let mut setter_errors = Vec::new();
    for (i, (what, key, mods)) in [
        (0u16, 0u32, 128u16),
        (3, 13, 128),
        (3, 3, 128),
        (3, 27, 128),
        (3, 46, 384),
        (3, 120, 128),
        (3, 13, 128),
        (3, 13, 128),
        (5, 13, 128),
        (3, 27, 128),
        (3, 46, 384),
        (3, 27, 128),
        (3, 27, 128),
        (3, 46, 384),
    ]
    .iter()
    .enumerate()
    {
        if i == 6 || i == 7 {
            let h = dialogs.records[&d].items[0].handle;
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            m.write_word(sp, if i == 6 { 255 } else { 0 });
            m.write_long(sp + 2, h);
            if !controls.dispatch(0xa95d, &mut cpu, m, &mut heap, &q)? || cpu.dar[15] != sp + 6 {
                return Err("filter fixture control ABI mismatch".into());
            }
        }
        if i == 7 {
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            cpu.dar[0] = 0x0304;
            m.write_word(sp, 2);
            m.write_long(sp + 2, d);
            m.write_word(sp + 6, 0xa5a5);
            if !dialogs.dispatch_filter(0xaa68, &mut cpu, m, &mut heap, &q, &fonts, &controls)?
                || cpu.pc != 0x19002
                || cpu.dar[15] != sp + 6
                || m.read_word(sp + 6) != 0
            {
                return Err("SetDialogDefaultItem Pascal ABI mismatch".into());
            }
            setter_errors.push(m.read_word(sp + 6) as i16);
        }
        if i == 9 || i == 12 {
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            cpu.dar[0] = 0x0305;
            m.write_word(sp, if i == 9 { 2 } else { 1 });
            m.write_long(sp + 2, d);
            m.write_word(sp + 6, 0xa5a5);
            if !dialogs.dispatch_filter(0xaa68, &mut cpu, m, &mut heap, &q, &fonts, &controls)?
                || cpu.pc != 0x19002
                || cpu.dar[15] != sp + 6
                || m.read_word(sp + 6) != 0
            {
                return Err("SetDialogCancelItem Pascal ABI mismatch".into());
            }
            setter_errors.push(m.read_word(sp + 6) as i16);
        }
        if i == 11 || i == 12 {
            let h = dialogs.records[&d].items[1].handle;
            cpu.pc = 0x19000;
            cpu.dar[15] = sp;
            m.write_word(sp, if i == 11 { 255 } else { 0 });
            m.write_long(sp + 2, h);
            if !controls.dispatch(0xa95d, &mut cpu, m, &mut heap, &q)? || cpu.dar[15] != sp + 6 {
                return Err("cancel control ABI mismatch".into());
            }
        }
        let event = 0x110100;
        let item = 0x110120;
        for j in 0..16 {
            m.write_byte(event + j, 0);
        }
        m.write_word(event, *what);
        m.write_long(event + 2, *key);
        m.write_word(event + 14, *mods);
        m.write_word(item, -1234i16 as u16);
        m.write_long(sp, 0x19000);
        m.write_long(sp + 4, item);
        m.write_long(sp + 8, event);
        m.write_long(sp + 12, d);
        m.write_word(sp + 16, 0xa5a5);
        cpu.pc = proc;
        cpu.dar[15] = sp;
        let op = m.read_word(proc);
        if !dialogs.dispatch_filter(op, &mut cpu, m, &mut heap, &q, &fonts, &controls)?
            || cpu.pc != 0x19000
            || cpu.dar[15] != sp + 16
        {
            return Err("standard filter callable Pascal ABI mismatch".into());
        }
        rows.push(json!([
            m.read_byte(sp + 16),
            m.read_word(item) as i16,
            m.read_word(event),
            m.read_long(event + 2),
            m.read_word(event + 14)
        ]));
    }
    let mut closed = false;
    for _ in 0..20000 {
        if let Some((token, result)) = callbacks.finish(&mut modal_cpu, m)? {
            standard.callback_result(token, result)?;
            continue;
        }
        if modal_cpu.pc == 0x19000 {
            standard.dispatch(
                0xa9ea,
                &mut modal_cpu,
                m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                &files,
                resources,
                &mut script,
            )?;
            if modal_cpu.pc == 0x19002 {
                closed = true;
                break;
            }
        } else {
            let step = modal_cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!("standard-filter context cleanup stopped: {step:?}"));
            }
        }
    }
    if !closed || modal_cpu.dar[15] != entry + 40 {
        return Err("standard-filter context cleanup failed".into());
    }
    Ok(json!({"schema":1,"probe":"standard-filter-v1","dialogs":{"error":error,"nonnull":(proc!=0) as u8,"cases":rows,"setter_errors":setter_errors}}).to_string())
}

pub fn doc_lifecycle_probe<B: AddressBus>(
    m: &mut B,
    resources: &[Resource<'_>],
) -> Result<String, String> {
    use serde_json::json;
    let mut heap = Heap::new(0x20000, 0xe0000);
    let mut q = QuickDraw::new();
    q.init(m, 0x100000)?;
    let mut fonts = Fonts::new();
    fonts.init()?;
    let mut events = Events::new();
    let mut windows = Windows::new();
    windows.init(m, &mut heap, &q)?;
    let mut close_rows = Vec::new();
    for mode in 0..2 {
        let before = windows.front();
        let port = q.get_port(m)?;
        let storage = if mode == 0 {
            0
        } else {
            heap.new_ptr(m, 220, true).map_err(|e| e.to_string())?
        };
        let w = windows.create(
            m,
            &mut heap,
            &q,
            &fonts,
            &mut events,
            storage,
            crate::managers::Rect {
                top: 40,
                left: 30,
                bottom: 140,
                right: 230,
            },
            b"Close",
            true,
            0,
            u32::MAX,
            true,
            0,
        )?;
        let size = heap.ptr_size(w).map_err(|e| e.to_string())?;
        let front = windows.front() == w;
        let mut cpu = CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x7d0000;
        m.write_long(cpu.dar[15], w);
        if !windows.dispatch(
            0xa92d,
            &mut cpu,
            m,
            &mut heap,
            &mut q,
            &fonts,
            &mut events,
            &[],
            |_, _, _| Err("unexpected resource load".into()),
        )? || cpu.pc != 0x19002
            || cpu.dar[15] != 0x7d0004
        {
            return Err("CloseWindow ABI mismatch".into());
        }
        let restored = windows.front() == before;
        let retained = heap.ptr_size(w) == Ok(size);
        let size_error = heap.error;
        let after = q.get_port(m)?;
        q.set_port(m, port)?;
        let _ = heap.dispose_ptr(w);
        close_rows.push(json!([
            front as u8,
            restored as u8,
            retained as u8,
            size_error,
            heap.error,
            (after == w) as u8,
            (after == port) as u8
        ]));
    }
    let mut controls = Controls::new();
    let mut dialogs = Dialogs::new();
    // Stop in the first null-event filter callback of the actual modal
    // package, matching the native fixture's context before the API matrix.
    let mut standard = StandardFile {
        put: None,
        trace: false,
    };
    let mut callbacks = Callbacks::silent();
    let files = Files::new();
    let mut script = VecDeque::new();
    let mut modal_cpu = CpuCore::new();
    modal_cpu.set_cpu_type(m68k::CpuType::M68020);
    let entry = 0x7e0000;
    modal_cpu.pc = 0x19000;
    modal_cpu.dar[15] = entry;
    let filter = 0x19b00;
    for (i, word) in [
        0x206fu16, 0x000c, 0x4a50, 0x660e, 0x206f, 0x0008, 0x30bc, 0x0002, 0x1f7c, 0x0001, 0x0014,
        0x205f, 0xdefc, 0x0010, 0x4ed0,
    ]
    .iter()
    .enumerate()
    {
        m.write_word(filter + 2 * i as u32, *word);
    }
    for i in 0..42 {
        m.write_byte(entry + i, 0);
    }
    m.write_word(entry, 8);
    m.write_long(entry + 14, filter);
    m.write_long(entry + 22, u32::MAX);
    m.write_word(entry + 26, 0);
    m.write_word(entry + 36, u16::MAX);
    m.write_long(entry + 28, 0x110400);
    m.write_long(entry + 32, 0x110500);
    m.write_byte(0x110500, 10);
    for (i, b) in b"filter.txt".iter().enumerate() {
        m.write_byte(0x110501 + i as u32, *b);
    }
    let mut context = false;
    for _ in 0..20000 {
        if let Some((token, result)) = callbacks.finish(&mut modal_cpu, m)? {
            standard.callback_result(token, result)?;
            continue;
        }
        if modal_cpu.pc == filter {
            let ep = m.read_long(modal_cpu.dar[15] + 12);
            if m.read_word(ep) == 0 {
                context = true;
                break;
            }
        }
        if modal_cpu.pc == 0x19000 {
            standard.dispatch(
                0xa9ea,
                &mut modal_cpu,
                m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                &files,
                resources,
                &mut script,
            )?;
        } else {
            let step = modal_cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!(
                    "standard-filter context fixture CPU stopped: {step:?}"
                ));
            }
        }
    }
    if !context {
        return Err("standard-filter modal context not reached".into());
    }
    let d = standard.put.as_ref().ok_or("missing modal context")?.dialog;
    let list_handle = m.read_long(d + 156);
    let list = m.read_long(list_handle);
    let count = m.read_word(list) as usize + 1;
    let mut items = Vec::new();
    let mut at = list + 2;
    for number in 1..=count {
        let h = m.read_long(at);
        let kind = m.read_byte(at + 12);
        let r = crate::quickdraw::read_rect(m, at + 4);
        let text = match kind & 127 {
            4..=6 => {
                let p = m.read_long(h);
                let n = m.read_byte(p + 40) as u32;
                (0..n).map(|i| m.read_byte(p + 41 + i)).collect::<Vec<_>>()
            }
            8 | 16 => {
                let p = m.read_long(h);
                let n = m.read_byte(p) as u32;
                (0..n).map(|i| m.read_byte(p + 1 + i)).collect()
            }
            _ => Vec::new(),
        };
        items.push(
            json!({"number":number,"kind":kind,"rect":[r.top,r.left,r.bottom,r.right],"text":text}),
        );
        let n = m.read_byte(at + 13) as u32;
        at += 14 + n + (n & 1);
    }
    let port = crate::quickdraw::read_rect(m, d + 16);
    let default_open =
        json!({"count":count,"port":[port.top,port.left,port.bottom,port.right],"items":items});
    let mut closed = false;
    for _ in 0..20000 {
        if let Some((token, result)) = callbacks.finish(&mut modal_cpu, m)? {
            standard.callback_result(token, result)?;
            continue;
        }
        if modal_cpu.pc == 0x19000 {
            standard.dispatch(
                0xa9ea,
                &mut modal_cpu,
                m,
                &mut heap,
                &q,
                &fonts,
                &mut windows,
                &mut controls,
                &mut events,
                &mut dialogs,
                &mut callbacks,
                &files,
                resources,
                &mut script,
            )?;
            if modal_cpu.pc == 0x19002 {
                closed = true;
                break;
            }
        } else {
            let step = modal_cpu.step(m);
            if !matches!(step, m68k::StepResult::Ok { .. }) {
                return Err(format!("standard-filter context cleanup stopped: {step:?}"));
            }
        }
    }
    if !closed || modal_cpu.dar[15] != entry + 42 {
        return Err("standard-filter context cleanup failed".into());
    }
    Ok(json!({"schema":1,"probe":"doc-lifecycle-v1","windows":{"close":close_rows},"dialogs":{"default_open":default_open,"cancelled":(m.read_byte(0x110400)==0) as u8}}).to_string())
}
