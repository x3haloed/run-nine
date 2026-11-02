//! Standard scroll controls, linked through native Window and Control records.
use crate::{
    managers::{Heap, Rect},
    quickdraw::{read_rect, write_rect, QuickDraw},
};
use m68k::AddressBus;
use std::collections::{BTreeMap, BTreeSet};
pub struct Controls {
    handles: BTreeSet<u32>,
    kinds: BTreeMap<u32, i16>,
    tracking: Option<Tracking>,
}
#[derive(Clone, Copy)]
struct Tracking {
    handle: u32,
    action: u32,
    part: i16,
    start: (i16, i16),
    hilite: u8,
    last_callback: Option<u32>,
    indicator: Option<Indicator>,
}
#[derive(Clone, Copy)]
struct Indicator {
    bar: Rect,
    thumb: Rect,
    vertical: bool,
    travel: i32,
    min: i16,
    max: i16,
    outline: Option<Rect>,
}
/// The measured OS 9 profile groups decrement/increment arrows at the end.
/// Drawing, hit testing and subsequent indicator tracking share this geometry.
struct Scrollbar {
    vertical: bool,
    up: Rect,
    down: Rect,
    thumb: Option<Rect>,
}
impl Scrollbar {
    fn new(r: Rect, value: i16, min: i16, max: i16) -> Option<Self> {
        let width = r.right as i32 - r.left as i32;
        let height = r.bottom as i32 - r.top as i32;
        if width <= 0 || height <= 0 {
            return None;
        }
        let vertical = height >= width;
        let (first, last, thickness) = if vertical {
            (r.top as i32, r.bottom as i32, width)
        } else {
            (r.left as i32, r.right as i32, height)
        };
        let make = |start: i32, end: i32| {
            if vertical {
                Rect {
                    top: start as i16,
                    bottom: end as i16,
                    ..r
                }
            } else {
                Rect {
                    left: start as i16,
                    right: end as i16,
                    ..r
                }
            }
        };
        let up = (last - 2 * thickness + 1).max(first);
        let down = (last - thickness + 1).max(first);
        let travel = (last - first - 3 * thickness + 1).max(0);
        let thumb = if max > min && up > first {
            let range = max as i32 - min as i32;
            let value = (value as i32 - min as i32).clamp(0, range);
            let at =
                first + ((travel as i64 * value as i64 + range as i64 / 2) / range as i64) as i32;
            Some(make(at, (at + thickness + 1).min(up)))
        } else {
            None
        };
        Some(Self {
            vertical,
            up: make(up, down),
            down: make(down, last),
            thumb,
        })
    }
    fn hit(&self, v: i16, h: i16) -> i16 {
        if self.up.contains(v, h) {
            20
        } else if self.down.contains(v, h) {
            21
        } else if let Some(thumb) = self.thumb {
            if thumb.contains(v, h) {
                129
            } else if if self.vertical {
                v < thumb.top
            } else {
                h < thumb.left
            } {
                22
            } else {
                23
            }
        } else {
            0
        }
    }
}
impl Controls {
    pub fn new() -> Self {
        Self {
            handles: BTreeSet::new(),
            kinds: BTreeMap::new(),
            tracking: None,
        }
    }
    pub fn is_tracking(&self) -> bool {
        self.tracking.is_some()
    }
    pub fn callback_result(&self) -> Result<(), String> {
        if self.tracking.is_none() {
            return Err("control callback return without tracking".into());
        }
        Ok(())
    }
    /// Suspends the Pascal trap while tracking and while real application action
    /// procedures run. The host supplies only ordinary global pointer events.
    pub fn track<B: AddressBus>(
        &mut self,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        q: &QuickDraw,
        events: &crate::events::Events,
        callbacks: &mut crate::callbacks::Callbacks,
    ) -> Result<(), String> {
        let sp = cpu.dar[15];
        let h = m.read_long(sp + 8);
        let p = self.record(m, h)?;
        let mut tracking = if let Some(t) = self.tracking {
            if t.handle != h {
                return Err("reentrant TrackControl".into());
            }
            t
        } else {
            let start = (m.read_word(sp + 4) as i16, m.read_word(sp + 6) as i16);
            let part = self.test(m, h, start.0, start.1)?;
            let action = m.read_long(sp);
            Tracking {
                handle: h,
                action: if action == u32::MAX {
                    m.read_long(p + 32)
                } else {
                    action
                },
                part,
                start,
                hilite: m.read_byte(p + 17),
                last_callback: None,
                indicator: if part == 129 {
                    if m.read_long(0x9f6) != 0 {
                        return Err("indicator DragHook is not implemented".into());
                    }
                    let bar = read_rect(m, p + 8);
                    let value = m.read_word(p + 18) as i16;
                    let min = m.read_word(p + 20) as i16;
                    let max = m.read_word(p + 22) as i16;
                    let layout = Scrollbar::new(bar, value, min, max)
                        .ok_or("indicator without scrollbar geometry")?;
                    let thickness = if layout.vertical {
                        bar.right as i32 - bar.left as i32
                    } else {
                        bar.bottom as i32 - bar.top as i32
                    };
                    let length = if layout.vertical {
                        bar.bottom as i32 - bar.top as i32
                    } else {
                        bar.right as i32 - bar.left as i32
                    };
                    Some(Indicator {
                        bar,
                        thumb: layout.thumb.ok_or("indicator without thumb")?,
                        vertical: layout.vertical,
                        travel: (length - 3 * thickness + 1).max(0),
                        min,
                        max,
                        outline: None,
                    })
                } else {
                    None
                },
            }
        };
        if tracking.action == u32::MAX {
            return Err("custom CDEF auto tracking is not implemented".into());
        }
        let owner = m.read_long(p + 4);
        let bitmap = q.port_bitmap(m, owner)?;
        let origin = read_rect(m, bitmap + 6);
        let point = if self.tracking.is_none() {
            tracking.start
        } else {
            (
                events.mouse.0.wrapping_add(origin.top),
                events.mouse.1.wrapping_add(origin.left),
            )
        };
        if let Some(mut indicator) = tracking.indicator {
            let delta = if indicator.vertical {
                point.0 as i32 - tracking.start.0 as i32
            } else {
                point.1 as i32 - tracking.start.1 as i32
            };
            let first = if indicator.vertical {
                indicator.bar.top
            } else {
                indicator.bar.left
            } as i32;
            let initial = if indicator.vertical {
                indicator.thumb.top
            } else {
                indicator.thumb.left
            } as i32;
            let at = (initial + delta).clamp(first, first + indicator.travel);
            // Native TrackControl permits a 100-pixel slop rectangle. Use
            // widened arithmetic so controls near signed coordinate limits
            // cannot wrap the acceptance bounds.
            let accepted = point.0 as i32 >= indicator.bar.top as i32 - 100
                && (point.0 as i32) < indicator.bar.bottom as i32 + 100
                && point.1 as i32 >= indicator.bar.left as i32 - 100
                && (point.1 as i32) < indicator.bar.right as i32 + 100;
            let offset = at - initial;
            let outline = if accepted {
                Some(if indicator.vertical {
                    Rect {
                        top: (indicator.thumb.top as i32 + offset) as i16,
                        bottom: (indicator.thumb.bottom as i32 + offset) as i16,
                        ..indicator.thumb
                    }
                } else {
                    Rect {
                        left: (indicator.thumb.left as i32 + offset) as i16,
                        right: (indicator.thumb.right as i32 + offset) as i16,
                        ..indicator.thumb
                    }
                })
            } else {
                None
            };
            if events.modifiers & 0x80 != 0 {
                let moved = accepted && at != initial;
                if moved && indicator.travel > 0 {
                    let range = indicator.max as i64 - indicator.min as i64;
                    let value = indicator.min as i64
                        + ((at - first) as i64 * range + indicator.travel as i64 / 2)
                            / indicator.travel as i64;
                    m.write_word(p + 18, value as i16 as u16);
                }
                m.write_byte(p + 17, tracking.hilite);
                self.draw(m, q, h)?;
                self.tracking = None;
                m.write_word(sp + 12, if moved { 129 } else { 0 });
                cpu.dar[15] += 12;
                cpu.pc += 2;
                return Ok(());
            }
            // The public value remains unchanged while the procedural drag
            // outline moves. Draw only when it moves; callbacks run on the CPU.
            if self.tracking.is_none() || indicator.outline != outline {
                self.draw(m, q, h)?;
                if let Some(r) = outline {
                    for v in r.top..r.bottom {
                        for x in r.left..r.right {
                            if (v == r.top || v == r.bottom - 1 || x == r.left || x == r.right - 1)
                                && ((v as i32 + x as i32) & 1 == 0)
                            {
                                q.transfer(m, owner, x, v, true, 2)?;
                            }
                        }
                    }
                }
                indicator.outline = outline;
            }
            let call = tracking.action != 0 && tracking.last_callback != Some(events.ticks);
            if call {
                tracking.last_callback = Some(events.ticks);
            }
            tracking.indicator = Some(indicator);
            self.tracking = Some(tracking);
            if call {
                callbacks.call_void(cpu, m, tracking.action, &[], 5)?;
            }
            return Ok(());
        }
        let hit = self.test(m, h, point.0, point.1)?;
        if tracking.part == 0 || events.modifiers & 0x80 != 0 {
            // The measured standard scrollbar CDEF retains its tracked part
            // on outside release; button cancellation uses the final hit.
            let result = if self.kinds[&h] == 16 {
                tracking.part
            } else if hit == tracking.part {
                hit
            } else {
                0
            };
            m.write_byte(p + 17, tracking.hilite);
            self.draw(m, q, h)?;
            self.tracking = None;
            m.write_word(sp + 12, result as u16);
            cpu.dar[15] += 12;
            cpu.pc += 2;
            return Ok(());
        }
        m.write_byte(
            p + 17,
            if hit == tracking.part {
                tracking.part as u8
            } else {
                0
            },
        );
        self.draw(m, q, h)?;
        let call = tracking.action != 0
            && tracking.part != 129
            && hit == tracking.part
            && tracking.last_callback != Some(events.ticks);
        if call {
            tracking.last_callback = Some(events.ticks);
        }
        self.tracking = Some(tracking);
        if call {
            callbacks.call_void(
                cpu,
                m,
                tracking.action,
                &[
                    crate::callbacks::Arg::Long(h),
                    crate::callbacks::Arg::Word(tracking.part as u16),
                ],
                5,
            )?;
        }
        Ok(())
    }
    fn record<B: AddressBus>(&self, m: &mut B, h: u32) -> Result<u32, String> {
        if !self.handles.contains(&h) {
            return Err(format!("unknown control {h:#x}"));
        }
        Ok(m.read_long(h))
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        owner: u32,
        rect: Rect,
        title: &[u8],
        visible: bool,
        value: i16,
        min: i16,
        max: i16,
        proc_id: i16,
        refcon: u32,
    ) -> Result<u32, String> {
        if ![0, 1, 2, 16].contains(&proc_id) {
            return Err(format!("unsupported control definition {proc_id}"));
        }
        if owner == 0 {
            return Err("NewControl without an owner window".into());
        }
        let h = heap
            .new_handle(m, 296, true)
            .map_err(|e| format!("NewControl: {e}"))?;
        let p = heap.data(h).map_err(|e| e.to_string())?;
        let previous = m.read_long(owner + 140);
        m.write_long(p, previous);
        m.write_long(owner + 140, h);
        m.write_long(p + 4, owner);
        write_rect(m, p + 8, rect);
        m.write_byte(p + 16, if visible { 255 } else { 0 });
        m.write_word(p + 18, value as u16);
        m.write_word(p + 20, min as u16);
        m.write_word(p + 22, max as u16);
        m.write_long(p + 36, refcon);
        let len = title.len().min(255);
        m.write_byte(p + 40, len as u8);
        for (i, b) in title[..len].iter().enumerate() {
            m.write_byte(p + 41 + i as u32, *b);
        }
        self.handles.insert(h);
        self.kinds.insert(h, proc_id);
        Ok(h)
    }
    pub fn dispose<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let owner = m.read_long(p + 4);
        let next = m.read_long(p);
        let mut link = owner + 140;
        let mut count = 0;
        while m.read_long(link) != h {
            let current = m.read_long(link);
            if current == 0 || count > self.handles.len() {
                return Err("broken window control chain".into());
            }
            link = self.record(m, current)?;
            count += 1;
        }
        m.write_long(link, next);
        heap.dispose(m, h).map_err(|e| e.to_string())?;
        self.handles.remove(&h);
        self.kinds.remove(&h);
        Ok(())
    }
    pub(crate) fn test<B: AddressBus>(
        &self,
        m: &mut B,
        h: u32,
        v: i16,
        x: i16,
    ) -> Result<i16, String> {
        let p = self.record(m, h)?;
        let r = read_rect(m, p + 8);
        if m.read_byte(p + 16) == 0 || m.read_byte(p + 17) == 255 || !r.contains(v, x) {
            return Ok(0);
        }
        match self.kinds[&h] {
            0 => Ok(10),
            1 | 2 => Ok(11),
            16 => {
                let min = m.read_word(p + 20) as i16;
                let max = m.read_word(p + 22) as i16;
                if max <= min {
                    return Ok(0);
                }
                Ok(Scrollbar::new(r, m.read_word(p + 18) as i16, min, max)
                    .map_or(0, |s| s.hit(v, x)))
            }
            kind => Err(format!("unsupported control definition {kind}")),
        }
    }
    fn chain<B: AddressBus>(&self, m: &mut B, w: u32) -> Result<Vec<u32>, String> {
        let mut h = m.read_long(w + 140);
        let mut visited = BTreeSet::new();
        let mut list = Vec::new();
        while h != 0 {
            if !visited.insert(h) {
                return Err("cyclic control chain".into());
            }
            list.push(h);
            let p = self.record(m, h)?;
            h = m.read_long(p);
        }
        Ok(list)
    }
    fn find<B: AddressBus>(&self, m: &mut B, w: u32, v: i16, x: i16) -> Result<(i16, u32), String> {
        for h in self.chain(m, w)?.into_iter().rev() {
            let part = self.test(m, h, v, x)?;
            if part != 0 {
                return Ok((part, h));
            }
        }
        Ok((0, 0))
    }
    pub fn draw<B: AddressBus>(&self, m: &mut B, q: &QuickDraw, h: u32) -> Result<(), String> {
        let p = self.record(m, h)?;
        let owner = m.read_long(p + 4);
        if m.read_byte(p + 16) == 0 || m.read_byte(owner + 110) == 0 {
            return Ok(());
        }
        let r = read_rect(m, p + 8);
        if r.top >= r.bottom || r.left >= r.right {
            return Ok(());
        }
        let old = q.get_port(m)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(owner + i)).collect();
        q.set_port(m, owner)?;
        q.pen_size(m, 1, 1)?;
        q.set_word(m, 56, 8)?;
        q.set_word(m, 66, 0)?;
        let render = (|| {
            q.paint_rect(m, r, [0; 8], 8)?;
            if self.kinds[&h] != 16 {
                let kind = self.kinds[&h];
                let box_rect = if kind == 0 {
                    r
                } else {
                    Rect {
                        top: r.top + (r.bottom - r.top - 12) / 2,
                        left: r.left,
                        bottom: r.top + (r.bottom - r.top - 12) / 2 + 12,
                        right: r.left + 12,
                    }
                };
                if kind == 2 {
                    let selected = m.read_word(p + 18) != 0;
                    for y in 0..12i16 {
                        for x in 0..12i16 {
                            let distance =
                                (2 * x - 11) * (2 * x - 11) + (2 * y - 11) * (2 * y - 11);
                            if (100..=144).contains(&distance) || (selected && distance <= 20) {
                                q.transfer(m, owner, box_rect.left + x, box_rect.top + y, true, 0)?;
                            }
                        }
                    }
                    return Ok(());
                }
                q.frame_rect(m, box_rect)?;
                if kind != 0 && m.read_word(p + 18) != 0 {
                    q.paint_rect(
                        m,
                        Rect {
                            top: box_rect.top + 3,
                            left: box_rect.left + 3,
                            bottom: box_rect.bottom - 3,
                            right: box_rect.right - 3,
                        },
                        [255; 8],
                        8,
                    )?;
                }
                return Ok(());
            }
            q.frame_rect(m, r)?;
            let min = m.read_word(p + 20) as i16;
            let max = m.read_word(p + 22) as i16;
            let Some(layout) = Scrollbar::new(r, m.read_word(p + 18) as i16, min, max) else {
                return Ok(());
            };
            let vertical = layout.vertical;
            for (end, box_rect) in [(false, layout.up), (true, layout.down)] {
                q.frame_rect(m, box_rect)?;
                if m.read_byte(p + 17) != 255
                    && max > min
                    && box_rect.right as i32 - box_rect.left as i32 >= 5
                    && box_rect.bottom as i32 - box_rect.top as i32 >= 5
                {
                    let cx = (box_rect.left as i32 + box_rect.right as i32) / 2;
                    let cy = (box_rect.top as i32 + box_rect.bottom as i32) / 2;
                    for d in 0..3 {
                        let at = if end { d - 1 } else { 1 - d };
                        let arrow = if vertical {
                            Rect {
                                top: cy as i16 + at,
                                left: cx as i16 - d,
                                bottom: cy as i16 + at + 1,
                                right: cx as i16 + d + 1,
                            }
                        } else {
                            Rect {
                                left: cx as i16 + at,
                                top: cy as i16 - d,
                                right: cx as i16 + at + 1,
                                bottom: cy as i16 + d + 1,
                            }
                        };
                        q.paint_rect(m, arrow, [255; 8], 8)?;
                    }
                }
            }
            if max > min && m.read_byte(p + 17) != 255 {
                if let Some(thumb) = layout.thumb {
                    q.frame_rect(m, thumb)?;
                }
            }
            Ok(())
        })();
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(owner + i as u32, *b);
        }
        q.set_port(m, old)?;
        render
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
    ) -> Result<bool, String> {
        let sp = cpu.dar[15];
        let pop;
        let mut redraw = None;
        match op {
            0xa966 => {
                let v = m.read_word(sp) as i16;
                let x = m.read_word(sp + 2) as i16;
                let h = m.read_long(sp + 4);
                let part = self.test(m, h, v, x)?;
                m.write_word(sp + 8, part as u16);
                pop = 8;
            }
            0xa96c => {
                let output = m.read_long(sp);
                let w = m.read_long(sp + 4);
                let v = m.read_word(sp + 8) as i16;
                let x = m.read_word(sp + 10) as i16;
                let (part, h) = self.find(m, w, v, x)?;
                m.write_long(output, h);
                m.write_word(sp + 12, part as u16);
                pop = 12;
            }
            0xa954 => {
                let refcon = m.read_long(sp);
                let proc_id = m.read_word(sp + 4) as i16;
                let max = m.read_word(sp + 6) as i16;
                let min = m.read_word(sp + 8) as i16;
                let value = m.read_word(sp + 10) as i16;
                let vis = m.read_byte(sp + 12) != 0;
                let title = m.read_long(sp + 14);
                let rect = m.read_long(sp + 18);
                let owner = m.read_long(sp + 22);
                let len = m.read_byte(title) as u32;
                let title: Vec<u8> = (0..len).map(|i| m.read_byte(title + 1 + i)).collect();
                let rect = read_rect(m, rect);
                let h = self.create(
                    m, heap, owner, rect, &title, vis, value, min, max, proc_id, refcon,
                )?;
                m.write_long(sp + 26, h);
                pop = 26;
                redraw = Some(h);
            }
            0xa955 => {
                let h = m.read_long(sp);
                self.dispose(m, heap, h)?;
                pop = 4;
            }
            0xa960..=0xa962 => {
                let h = m.read_long(sp);
                let p = self.record(m, h)?;
                let offset = 18 + (op - 0xa960) as u32 * 2;
                let value = m.read_word(p + offset);
                m.write_word(sp + 4, value);
                pop = 4;
            }
            0xa963..=0xa965 => {
                let value = m.read_word(sp) as i16;
                let h = m.read_long(sp + 2);
                let p = self.record(m, h)?;
                let offset = 18 + (op - 0xa963) as u32 * 2;
                m.write_word(p + offset, value as u16);
                let min = m.read_word(p + 20) as i16;
                let max = m.read_word(p + 22) as i16;
                let value = m.read_word(p + 18) as i16;
                m.write_word(p + 18, value.max(min).min(max) as u16);
                pop = 6;
                redraw = Some(h);
            }
            0xa95d => {
                let value = m.read_word(sp);
                let h = m.read_long(sp + 2);
                let p = self.record(m, h)?;
                m.write_byte(p + 17, value as u8);
                pop = 6;
                redraw = Some(h);
            }
            0xa957 | 0xa958 => {
                let h = m.read_long(sp);
                let p = self.record(m, h)?;
                m.write_byte(p + 16, if op == 0xa957 { 255 } else { 0 });
                pop = 4;
                redraw = Some(h);
            }
            0xa959 | 0xa95c => {
                let v = m.read_word(sp) as i16;
                let u = m.read_word(sp + 2) as i16;
                let h = m.read_long(sp + 4);
                let p = self.record(m, h)?;
                let mut r = read_rect(m, p + 8);
                if op == 0xa959 {
                    r.bottom = v.wrapping_add(r.bottom - r.top);
                    r.right = u.wrapping_add(r.right - r.left);
                    r.top = v;
                    r.left = u;
                } else {
                    r.bottom = r.top.wrapping_add(v);
                    r.right = r.left.wrapping_add(u);
                }
                write_rect(m, p + 8, r);
                pop = 8;
                redraw = Some(h);
            }
            0xa96d => {
                let h = m.read_long(sp);
                pop = 4;
                redraw = Some(h);
            }
            0xa969 => {
                let owner = m.read_long(sp);
                for h in self.chain(m, owner)? {
                    self.draw(m, q, h)?;
                }
                pop = 4;
            }
            _ => return Ok(false),
        }
        if let Some(h) = redraw {
            self.draw(m, q, h)?;
        }
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
