//! Window records, regions and standard document windows in Macintosh memory.
use crate::regions::{Operation, Region};
use crate::{
    events::{Event, Events},
    fonts::Fonts,
    managers::{Heap, Rect},
    quickdraw::{write_rect, QuickDraw},
};
use m68k::AddressBus;
use std::collections::BTreeMap;
pub struct Window {
    pub address: u32,
    pub bounds: Rect,
    pub title: Vec<u8>,
    pub visible: bool,
    pub owned: bool,
    pub proc_id: i16,
}
pub struct Windows {
    pub manager_port: Option<u32>,
    pub windows: Vec<Window>,
    pub menu_height: i16,
    updating: BTreeMap<u32, Region>,
    tracking: Option<Tracking>,
    auxiliary: Vec<(u32, u32)>,
    default_table: Option<u32>,
    default_auxiliary: Option<u32>,
}
struct Tracking {
    opcode: u16,
    sp: u32,
    window: u32,
    start: (i16, i16),
    limits: Rect,
    bounds: Rect,
    command: bool,
    background: Vec<u8>,
    last: Option<[i32; 4]>,
}
fn region<B: AddressBus>(m: &mut B, heap: &mut Heap, r: Rect) -> Result<u32, String> {
    let h = heap.new_handle(m, 10, true).map_err(|e| e.to_string())?;
    let p = heap.data(h).map_err(|e| e.to_string())?;
    m.write_word(p, 10);
    write_rect(m, p + 2, r);
    Ok(h)
}
pub(crate) fn structure_bounds(bounds: Rect, proc_id: i16) -> Rect {
    if proc_id == 2 {
        return Rect {
            top: bounds.top - 1,
            left: bounds.left - 1,
            bottom: bounds.bottom + 1,
            right: bounds.right + 1,
        };
    }
    if proc_id == 1 {
        Rect {
            top: bounds.top - 6,
            left: bounds.left - 6,
            bottom: bounds.bottom + 7,
            right: bounds.right + 7,
        }
    } else {
        Rect {
            top: bounds.top - 22,
            left: bounds.left - 6,
            bottom: bounds.bottom + 7,
            right: bounds.right + 7,
        }
    }
}
impl Windows {
    pub fn new() -> Self {
        Self {
            manager_port: None,
            windows: Vec::new(),
            menu_height: 20,
            updating: BTreeMap::new(),
            tracking: None,
            auxiliary: Vec::new(),
            default_table: None,
            default_auxiliary: None,
        }
    }
    pub fn init<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
    ) -> Result<(), String> {
        if let Some(p) = self.manager_port {
            return q.set_port(m, p);
        }
        let p = heap.new_ptr(m, 108, true).map_err(|e| e.to_string())?;
        q.open_port(m, heap, p)?;
        m.write_long(0x9de, p);
        m.write_long(0x9d6, 0);
        m.write_word(0xbaa, self.menu_height as u16);
        let gray = region(
            m,
            heap,
            Rect {
                top: self.menu_height,
                left: 0,
                bottom: q.height,
                right: q.width,
            },
        )?;
        m.write_long(0x9ee, gray);
        self.manager_port = Some(p);
        q.paint_rect(
            m,
            Rect {
                top: self.menu_height,
                left: 0,
                bottom: q.height,
                right: q.width,
            },
            [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55],
            8,
        )?;
        q.paint_rect(
            m,
            Rect {
                top: 0,
                left: 0,
                bottom: self.menu_height,
                right: q.width,
            },
            [0; 8],
            8,
        )?;
        Ok(())
    }
    /// Resource-window entry points share the actual Resource Manager handles
    /// with applications; changes to a loaded WIND are visible here.
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<B: AddressBus, F>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &mut QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        resources: &[crate::resource::Resource<'_>],
        mut load: F,
    ) -> Result<bool, String>
    where
        F: FnMut(&crate::resource::Resource<'_>, &mut B, &mut Heap) -> Result<u32, String>,
    {
        if self.damage_dispatch(op, cpu, m, heap, q, events)? {
            return Ok(true);
        }
        if op == 0xaa41 {
            let sp = cpu.dar[15];
            let table = m.read_long(sp);
            let w = m.read_long(sp + 4);
            self.set_window_color(m, heap, q, fonts, w, table)?;
            cpu.dar[15] += 8;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xaa42 {
            let sp = cpu.dar[15];
            let target = m.read_long(sp);
            let w = m.read_long(sp + 4);
            let (found, h) = self.get_auxiliary(m, heap, w)?;
            m.write_long(target, h);
            m.write_byte(sp + 8, found as u8);
            cpu.dar[15] += 8;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xa80a {
            let sp = cpu.dar[15];
            let w = m.read_long(sp);
            let variant = self
                .windows
                .iter()
                .find(|window| window.address == w)
                .ok_or("GetWVariant on unknown window")?
                .proc_id
                & 15;
            m.write_word(sp + 4, variant as u16);
            cpu.dar[15] += 4;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xa925 || op == 0xa92b {
            self.track(op, cpu, m, heap, q, fonts, events)?;
            return Ok(true);
        }
        if op == 0xa91b || op == 0xa91d {
            let sp = cpu.dar[15];
            let flag = m.read_byte(sp) != 0;
            let v = m.read_word(sp + 2) as i16;
            let h = m.read_word(sp + 4) as i16;
            let w = m.read_long(sp + 6);
            if op == 0xa91b {
                self.move_window(m, heap, q, fonts, events, w, h, v, flag)?;
            } else {
                self.resize(m, heap, q, fonts, events, w, h, v, flag)?;
            }
            cpu.dar[15] += 10;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xa92d {
            let w = m.read_long(cpu.dar[15]);
            self.visible(m, heap, q, fonts, events, w, false)?;
            self.remove(m, heap, q, events, w, false)?;
            cpu.dar[15] += 4;
            cpu.pc += 2;
            return Ok(true);
        }
        if [0xa917, 0xa918, 0xa919, 0xa91a].contains(&op) {
            let sp = cpu.dar[15];
            let w = m.read_long(
                sp + match op {
                    0xa917 => 0,
                    _ => 4,
                },
            );
            if !self.windows.iter().any(|window| window.address == w) {
                return Err("window accessor on unknown window".into());
            }
            match op {
                0xa917 => {
                    let value = m.read_long(w + 152);
                    m.write_long(sp + 4, value);
                    cpu.dar[15] += 4;
                }
                0xa918 => {
                    let value = m.read_long(sp);
                    m.write_long(w + 152, value);
                    cpu.dar[15] += 8;
                }
                0xa919 => {
                    let output = m.read_long(sp);
                    let h = m.read_long(w + 134);
                    let p = heap.data(h).map_err(|e| e.to_string())?;
                    let n = m.read_byte(p) as u32;
                    if heap.size(h).map_err(|e| e.to_string())? < n + 1 {
                        return Err("truncated window title handle".into());
                    }
                    let bytes: Vec<u8> = (0..=n).map(|i| m.read_byte(p + i)).collect();
                    for (i, b) in bytes.iter().enumerate() {
                        m.write_byte(output + i as u32, *b);
                    }
                    cpu.dar[15] += 8;
                }
                0xa91a => {
                    let text = m.read_long(sp);
                    let n = m.read_byte(text) as u32;
                    let title: Vec<u8> = (0..n).map(|i| m.read_byte(text + 1 + i)).collect();
                    self.set_title(m, heap, q, fonts, w, &title)?;
                    cpu.dar[15] += 8;
                }
                _ => unreachable!(),
            }
            cpu.pc += 2;
            return Ok(true);
        }
        if ![0xa9bd, 0xaa46].contains(&op) {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let behind = m.read_long(sp);
        let storage = m.read_long(sp + 4);
        let id = m.read_word(sp + 8) as i16;
        let window = if let Some(resource) =
            resources.iter().find(|r| r.kind == *b"WIND" && r.id == id)
        {
            let h = load(resource, m, heap)?;
            let p = heap.data(h).map_err(|e| e.to_string())?;
            let size = heap.size(h).map_err(|e| e.to_string())?;
            if size < 19 {
                return Err("truncated WIND header".into());
            }
            let n = m.read_byte(p + 18) as u32;
            let end = 19 + n;
            if end > size {
                return Err("truncated WIND title".into());
            }
            let position_offset = (end + 1) & !1;
            if size > position_offset && size < position_offset + 2 {
                return Err("truncated WIND position".into());
            }
            if size >= position_offset + 2 && m.read_word(p + position_offset) != 0 {
                return Err("automatic resource-window positioning is not implemented".into());
            }
            let bounds = crate::quickdraw::read_rect(m, p);
            let proc_id = m.read_word(p + 8) as i16;
            let visible = m.read_byte(p + 10) != 0;
            let go_away = m.read_byte(p + 12) != 0;
            let refcon = m.read_long(p + 14);
            let title: Vec<u8> = (0..n).map(|i| m.read_byte(p + 19 + i)).collect();
            let w = self.create(
                m, heap, q, fonts, events, storage, bounds, &title, visible, proc_id, behind,
                go_away, refcon,
            )?;
            if op == 0xaa46 {
                let colors =
                    if let Some(r) = resources.iter().find(|r| r.kind == *b"wctb" && r.id == id) {
                        Some(load(r, m, heap)?)
                    } else {
                        None
                    };
                self.colorize(m, heap, q, fonts, w, colors)?;
            }
            w
        } else {
            0
        };
        // Native GetNew[ C ]Window leaves ResError at noErr even when missing.
        m.write_word(0xa60, 0);
        m.write_long(sp + 10, window);
        cpu.dar[15] += 10;
        cpu.pc += 2;
        Ok(true)
    }
    pub fn front(&self) -> u32 {
        self.windows
            .iter()
            .find(|w| w.visible)
            .map(|w| w.address)
            .unwrap_or(0)
    }
    pub fn find<B: AddressBus>(&self, m: &mut B, v: i16, h: i16) -> Result<(i16, u32), String> {
        if v >= 0 && v < self.menu_height {
            return Ok((1, 0));
        }
        for w in self.windows.iter().filter(|w| w.visible) {
            let b = w.bounds;
            let handle = m.read_long(w.address + 114);
            if !crate::regions::contains(m, handle, v, h)? {
                continue;
            }
            let close = Rect {
                top: b.top - 17,
                left: b.left - 1,
                bottom: b.top - 6,
                right: b.left + 10,
            };
            let grow = Rect {
                top: b.bottom - 15,
                left: b.right - 15,
                bottom: b.bottom + 7,
                right: b.right + 7,
            };
            let part = if m.read_byte(w.address + 112) != 0
                && m.read_byte(w.address + 111) != 0
                && close.contains(v, h)
            {
                6
            } else if w.proc_id & 4 == 0 && grow.contains(v, h) {
                5
            } else if b.contains(v, h) {
                3
            } else {
                4
            };
            return Ok((part, w.address));
        }
        Ok((0, 0))
    }
    pub fn set_title<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        address: u32,
        title: &[u8],
    ) -> Result<(), String> {
        let index = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("unknown window title target")?;
        let len = title.len().min(255);
        let h = m.read_long(address + 134);
        heap.resize(m, h, (len + 1) as u32)
            .map_err(|e| format!("SetWTitle: {e}"))?;
        let p = m.read_long(h);
        m.write_byte(p, len as u8);
        for (i, b) in title[..len].iter().enumerate() {
            m.write_byte(p + 1 + i as u32, *b);
        }
        m.write_word(address + 138, fonts.title_width(&title[..len])? as u16);
        self.windows[index].title = title[..len].to_vec();
        self.draw(m, q, fonts, address)
    }
    pub fn is_tracking(&self) -> bool {
        self.tracking.is_some()
    }
    #[allow(clippy::too_many_arguments)]
    fn track<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
    ) -> Result<(), String> {
        if m.read_long(0x9f6) != 0 {
            return Err("Window Manager DragHook callbacks are not implemented".into());
        }
        let mut t = if let Some(t) = self.tracking.take() {
            t
        } else {
            let sp = cpu.dar[15];
            let limits = m.read_long(sp);
            let start = (m.read_word(sp + 4) as i16, m.read_word(sp + 6) as i16);
            let window = m.read_long(sp + 8);
            let w = self
                .windows
                .iter()
                .find(|w| w.address == window)
                .ok_or("window tracking on unknown window")?;
            let limits = crate::quickdraw::read_rect(m, limits);
            if op == 0xa92b
                && (limits.left as u16 > limits.right as u16
                    || limits.top as u16 > limits.bottom as u16)
            {
                return Err("inverted GrowWindow size limits".into());
            }
            Tracking {
                opcode: op,
                sp,
                window,
                start,
                limits,
                bounds: w.bounds,
                command: events.mouse_down_modifiers & 0x100 != 0,
                background: (0..q.row_bytes() as u32 * q.height as u32)
                    .map(|i| m.read_byte(q.screen_base + i))
                    .collect(),
                last: None,
            }
        };
        if t.opcode != op || t.sp != cpu.dar[15] {
            return Err("reentrant window tracking".into());
        }
        let dy = events.mouse.0 as i32 - t.start.0 as i32;
        let dx = events.mouse.1 as i32 - t.start.1 as i32;
        let width = t.bounds.right as i32 - t.bounds.left as i32;
        let height = t.bounds.bottom as i32 - t.bounds.top as i32;
        let grow_width = if op == 0xa92b {
            (width + dx).clamp(t.limits.left as u16 as i32, t.limits.right as u16 as i32)
        } else {
            width
        };
        let grow_height = if op == 0xa92b {
            (height + dy).clamp(t.limits.top as u16 as i32, t.limits.bottom as u16 as i32)
        } else {
            height
        };
        let valid = op == 0xa92b || t.limits.contains(events.mouse.0, events.mouse.1);
        let outline = if !valid {
            None
        } else if op == 0xa925 {
            Some([
                t.bounds.top as i32 - 22 + dy,
                t.bounds.left as i32 - 6 + dx,
                t.bounds.bottom as i32 + 7 + dy,
                t.bounds.right as i32 + 7 + dx,
            ])
        } else {
            Some([
                t.bounds.top as i32 - 22,
                t.bounds.left as i32 - 6,
                t.bounds.top as i32 + grow_height + 7,
                t.bounds.left as i32 + grow_width + 7,
            ])
        };
        if events.modifiers & 0x80 != 0 {
            for (i, b) in t.background.iter().enumerate() {
                m.write_byte(q.screen_base + i as u32, *b);
            }
            if op == 0xa925 {
                if valid {
                    let left = i16::try_from(t.bounds.left as i32 + dx)
                        .map_err(|_| "drag horizontal overflow")?;
                    let top = i16::try_from(t.bounds.top as i32 + dy)
                        .map_err(|_| "drag vertical overflow")?;
                    self.move_window(m, heap, q, fonts, events, t.window, left, top, !t.command)?;
                }
            } else {
                let result = if grow_width == width && grow_height == height {
                    0
                } else {
                    ((grow_height as u32) << 16) | grow_width as u32
                };
                m.write_long(t.sp + 12, result);
            }
            events.flush(0x6, 0);
            cpu.dar[15] += 12;
            cpu.pc += 2;
            return Ok(());
        }
        if t.last != outline {
            for (i, b) in t.background.iter().enumerate() {
                m.write_byte(q.screen_base + i as u32, *b);
            }
            if let Some([top, left, bottom, right]) = outline {
                for y in top.max(self.menu_height as i32)..bottom.min(q.height as i32) {
                    for x in left.max(0)..right.min(q.width as i32) {
                        let frame = y == top || y == bottom - 1 || x == left || x == right - 1;
                        let grow = op == 0xa92b && (x == right - 23 || y == bottom - 23);
                        if (frame || grow) && (x + y) & 1 == 0 {
                            let at = q.screen_base + y as u32 * q.row_bytes() as u32 + x as u32 / 8;
                            let b = m.read_byte(at);
                            m.write_byte(at, b ^ (0x80 >> (x & 7)));
                        }
                    }
                }
            }
            t.last = outline;
        }
        self.tracking = Some(t);
        Ok(())
    }
    pub fn move_hidden<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        address: u32,
        left: i16,
        top: i16,
        front: bool,
    ) -> Result<(), String> {
        if self
            .windows
            .iter()
            .any(|w| w.address == address && w.visible)
        {
            return Err(
                "MoveWindow pixel relocation for visible windows is not implemented".into(),
            );
        }
        self.move_geometry(m, heap, q, address, left, top, front)
    }
    fn move_geometry<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        address: u32,
        left: i16,
        top: i16,
        front: bool,
    ) -> Result<(), String> {
        let index = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("MoveWindow on unknown window")?;
        let old = self.windows[index].bounds;
        let dh = left.checked_sub(old.left).ok_or("window move overflow")?;
        let dv = top.checked_sub(old.top).ok_or("window move overflow")?;
        self.windows[index].bounds = Rect {
            top,
            left,
            bottom: old.bottom.checked_add(dv).ok_or("window move overflow")?,
            right: old.right.checked_add(dh).ok_or("window move overflow")?,
        };
        let bitmap = q.port_bitmap(m, address)?;
        let b = crate::quickdraw::read_rect(m, bitmap + 6);
        write_rect(
            m,
            bitmap + 6,
            Rect {
                top: b.top.wrapping_sub(dv),
                left: b.left.wrapping_sub(dh),
                bottom: b.bottom.wrapping_sub(dv),
                right: b.right.wrapping_sub(dh),
            },
        );
        for offset in [114, 118, 122] {
            let h = m.read_long(address + offset);
            crate::regions::offset_handle(m, heap, h, dh, dv)?;
        }
        if front {
            self.bring_front(m, address)?;
        }
        Ok(())
    }
    /// Move screen bits that are actually visible; exposed content is invalidated
    /// for application redraw instead of inventing backing-store pixels.
    #[allow(clippy::too_many_arguments)]
    pub fn move_window<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        address: u32,
        left: i16,
        top: i16,
        front: bool,
    ) -> Result<(), String> {
        let old = self
            .windows
            .iter()
            .find(|w| w.address == address)
            .ok_or("MoveWindow on unknown window")?
            .bounds;
        if !self
            .windows
            .iter()
            .any(|w| w.address == address && w.visible)
        {
            return self.move_geometry(m, heap, q, address, left, top, front);
        }
        let (pixels, previous) = self.geometry_snapshot(m, q)?;
        self.move_geometry(m, heap, q, address, left, top, front)?;
        let dh = left - old.left;
        let dv = top - old.top;
        self.recompose_geometry(
            m, heap, q, fonts, events, address, dh, dv, true, &pixels, &previous,
        )
    }
    fn geometry_snapshot<B: AddressBus>(
        &self,
        m: &mut B,
        q: &QuickDraw,
    ) -> Result<(Vec<u8>, BTreeMap<u32, (Rect, Region)>), String> {
        let pixels: Vec<u8> = (0..q.row_bytes() as u32 * q.height as u32)
            .map(|i| m.read_byte(q.screen_base + i))
            .collect();
        let mut previous = BTreeMap::new();
        let desktop = Region::rectangle(Rect {
            top: self.menu_height,
            left: 0,
            bottom: q.height,
            right: q.width,
        });
        let mut cover = Region::default();
        for w in self.windows.iter().filter(|w| w.visible) {
            let h = m.read_long(w.address + 24);
            let area = Region::read(m, h)?
                .offset(w.bounds.left, w.bounds.top)?
                .combine(&desktop, Operation::Intersection)
                .combine(&cover, Operation::Difference);
            let structure = m.read_long(w.address + 114);
            cover = cover.combine(&Region::read(m, structure)?, Operation::Union);
            previous.insert(w.address, (w.bounds, area));
        }
        Ok((pixels, previous))
    }
    #[allow(clippy::too_many_arguments)]
    fn recompose_geometry<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        address: u32,
        dh: i16,
        dv: i16,
        invalidate: bool,
        pixels: &[u8],
        previous: &BTreeMap<u32, (Rect, Region)>,
    ) -> Result<(), String> {
        let desktop = Rect {
            top: self.menu_height,
            left: 0,
            bottom: q.height,
            right: q.width,
        };
        let mut cover = Region::default();
        let mut visible = BTreeMap::new();
        for w in self.windows.iter().filter(|w| w.visible) {
            let area = Region::rectangle(w.bounds)
                .combine(&Region::rectangle(desktop), Operation::Intersection)
                .combine(&cover, Operation::Difference);
            let structure = m.read_long(w.address + 114);
            cover = cover.combine(&Region::read(m, structure)?, Operation::Union);
            let vis = m.read_long(w.address + 24);
            area.offset(-w.bounds.left, -w.bounds.top)?
                .store(m, heap, vis)?;
            let (_, prior) = &previous[&w.address];
            let copied = if w.address == address {
                prior.offset(dh, dv)?
            } else {
                prior.clone()
            };
            let exposed = area.combine(&copied, Operation::Difference);
            if !exposed.bands.is_empty() && (w.address != address || invalidate) {
                let update = m.read_long(w.address + 122);
                Region::read(m, update)?
                    .combine(&exposed, Operation::Union)
                    .store(m, heap, update)?;
                Self::event(events, 6, w.address, false)?;
            }
            visible.insert(w.address, (area, copied));
        }
        let port = q.get_port(m)?;
        q.set_port(m, self.manager_port.ok_or("no Window Manager port")?)?;
        q.paint_rect(
            m,
            desktop,
            [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55],
            8,
        )?;
        q.set_port(m, port)?;
        self.hilites(m);
        for w in self.windows.iter().rev().filter(|w| w.visible) {
            self.draw(m, q, fonts, w.address)?;
            let (area, copied) = &visible[&w.address];
            let (dx, dy) = if w.address == address {
                (dh, dv)
            } else {
                (0, 0)
            };
            for band in &area.bands {
                for y in band.top..band.bottom {
                    for &(left, right) in &band.spans {
                        for x in left..right {
                            let sx = x - dx;
                            let sy = y - dy;
                            let black = copied.contains(y, x)
                                && sx >= 0
                                && sy >= 0
                                && sx < q.width
                                && sy < q.height
                                && pixels
                                    [(sy as u32 * q.row_bytes() as u32 + sx as u32 / 8) as usize]
                                    & (0x80 >> (sx & 7))
                                    != 0;
                            let at = q.screen_base + y as u32 * q.row_bytes() as u32 + x as u32 / 8;
                            let byte = m.read_byte(at);
                            let bit = 0x80 >> (x & 7);
                            m.write_byte(at, if black { byte | bit } else { byte & !bit });
                        }
                    }
                }
            }
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    pub fn resize<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        address: u32,
        width: i16,
        height: i16,
        update: bool,
    ) -> Result<(), String> {
        let index = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("SizeWindow on unknown window")?;
        if width < 0 || height < 0 {
            return Err("negative window dimensions".into());
        }
        if width == 0 && height == 0 {
            return Ok(());
        }
        let snapshot = if self.windows[index].visible {
            Some(self.geometry_snapshot(m, q)?)
        } else {
            None
        };
        let old = self.windows[index].bounds;
        let b = Rect {
            bottom: old
                .top
                .checked_add(height)
                .ok_or("window vertical overflow")?,
            right: old
                .left
                .checked_add(width)
                .ok_or("window horizontal overflow")?,
            ..old
        };
        self.windows[index].bounds = b;
        let local = Rect {
            top: 0,
            left: 0,
            bottom: height,
            right: width,
        };
        write_rect(m, address + 16, local);
        for (offset, rect) in [
            (118, b),
            (114, structure_bounds(b, self.windows[index].proc_id)),
        ] {
            let h = m.read_long(address + offset);
            if self.windows[index].visible {
                Region::rectangle(rect).store(m, heap, h)?;
            }
        }
        let h = m.read_long(address + 24);
        if self.windows[index].visible {
            Region::rectangle(local)
        } else {
            Region::default()
        }
        .store(m, heap, h)?;
        if let Some((pixels, previous)) = snapshot {
            self.recompose_geometry(
                m, heap, q, fonts, events, address, 0, 0, update, &pixels, &previous,
            )?;
        }
        Ok(())
    }
    fn links<B: AddressBus>(&self, m: &mut B) {
        m.write_long(0x9d6, self.windows.first().map(|w| w.address).unwrap_or(0));
        for (i, w) in self.windows.iter().enumerate() {
            m.write_long(
                w.address + 144,
                self.windows.get(i + 1).map(|w| w.address).unwrap_or(0),
            );
        }
    }
    fn hilites<B: AddressBus>(&self, m: &mut B) {
        let front = self.front();
        for w in &self.windows {
            m.write_byte(w.address + 111, (w.address == front) as u8);
        }
    }
    fn event(events: &mut Events, what: u16, address: u32, active: bool) -> Result<(), String> {
        let event = Event {
            what,
            message: address,
            when: events.ticks,
            v: events.mouse.0,
            h: events.mouse.1,
            modifiers: (events.modifiers & !1) | active as u16,
        };
        if what == 6 {
            events.post_update(event);
            Ok(())
        } else {
            events.post(event).map_err(|e| format!("window event: {e}"))
        }
    }

    pub fn bring_front<B: AddressBus>(&mut self, m: &mut B, address: u32) -> Result<(), String> {
        let i = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("unknown BringToFront window")?;
        let w = self.windows.remove(i);
        self.windows.insert(0, w);
        self.links(m);
        Ok(())
    }
    pub fn select<B: AddressBus>(
        &mut self,
        m: &mut B,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        address: u32,
    ) -> Result<(), String> {
        let old = self.front();
        self.bring_front(m, address)?;
        self.hilites(m);
        let new = self.front();
        if old != new {
            if old != 0 {
                Self::event(events, 8, old, false)?;
            }
            if new != 0 {
                Self::event(events, 8, new, true)?;
            }
        }
        for w in self.windows.iter().rev() {
            self.draw(m, q, fonts, w.address)?;
        }
        Ok(())
    }
    pub fn visible<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        address: u32,
        visible: bool,
    ) -> Result<(), String> {
        let i = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("unknown Show/Hide window")?;
        if self.windows[i].visible == visible {
            return Ok(());
        }
        let old = self.front();
        self.windows[i].visible = visible;
        if !visible {
            let w = self.windows.remove(i);
            self.windows.push(w);
            self.links(m);
        }
        m.write_byte(address + 110, visible as u8);
        let b = self
            .windows
            .iter()
            .find(|w| w.address == address)
            .unwrap()
            .bounds;
        let local = Rect {
            top: 0,
            left: 0,
            bottom: b.bottom - b.top,
            right: b.right - b.left,
        };
        let vis = m.read_long(address + 24);
        if visible {
            Region::rectangle(local)
        } else {
            Region::default()
        }
        .store(m, heap, vis)?;
        if !visible {
            for offset in [114, 118] {
                let h = m.read_long(address + offset);
                Region::default().store(m, heap, h)?;
            }
        }
        if visible {
            for (offset, rect) in [
                (118, b),
                (
                    114,
                    structure_bounds(
                        b,
                        self.windows
                            .iter()
                            .find(|w| w.address == address)
                            .unwrap()
                            .proc_id,
                    ),
                ),
            ] {
                let h = m.read_long(address + offset);
                Region::rectangle(rect).store(m, heap, h)?;
            }
        }
        let update = m.read_long(address + 122);
        if visible {
            Region::rectangle(b).store(m, heap, update)?;
            Self::event(events, 6, address, false)?;
        }
        if visible {
            self.paint_content(m, q, address)?;
        } else {
            let saved = q.get_port(m)?;
            q.set_port(m, self.manager_port.ok_or("missing Window Manager port")?)?;
            let rect = structure_bounds(
                b,
                self.windows
                    .iter()
                    .find(|w| w.address == address)
                    .ok_or("unknown window")?
                    .proc_id,
            );
            q.paint_rect(m, rect, [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55], 8)?;
            q.set_port(m, saved)?;
        }
        self.hilites(m);
        let new = self.front();
        if old != new {
            if old != 0 {
                Self::event(events, 8, old, false)?;
            }
            if new != 0 {
                Self::event(events, 8, new, true)?;
            }
        }
        for w in self.windows.iter().rev() {
            self.draw(m, q, fonts, w.address)?;
        }
        Ok(())
    }
    pub fn damage_dispatch<B: AddressBus>(
        &self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        events: &mut Events,
    ) -> Result<bool, String> {
        if !matches!(op, 0xa92a | 0xa929 | 0xa928 | 0xa927) {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let argument = m.read_long(sp);
        let mask = if matches!(op, 0xa92a | 0xa928) {
            Region::rectangle(crate::quickdraw::read_rect(m, argument))
        } else {
            Region::read(m, argument)?
        };
        let processed = self.damage(m, heap, q, events, &mask, matches!(op, 0xa928 | 0xa927))?;
        if processed {
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
        } else {
            cpu.set_ccr(0);
        }
        cpu.dar[15] += 4;
        cpu.pc += 2;
        Ok(true)
    }
    fn damage<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        events: &mut Events,
        local: &Region,
        invalidate: bool,
    ) -> Result<bool, String> {
        let address = q.get_port(m)?;
        if !self.windows.iter().any(|w| w.address == address) {
            return Err("window damage outside window port".into());
        }
        if local.bands.is_empty() {
            return Ok(false);
        }
        let bitmap = q.port_bitmap(m, address)?;
        let bounds = crate::quickdraw::read_rect(m, bitmap + 6);
        let dh = bounds
            .left
            .checked_neg()
            .ok_or("damage horizontal offset overflow")?;
        let dv = bounds
            .top
            .checked_neg()
            .ok_or("damage vertical offset overflow")?;
        let mask = local.offset(dh, dv)?;
        let h = m.read_long(address + 122);
        let old = Region::read(m, h)?;
        // The measured disjoint validation path exits before region work.
        if !invalidate && old.combine(&mask, Operation::Intersection).bands.is_empty() {
            return Ok(false);
        }
        old.combine(
            &mask,
            if invalidate {
                Operation::Union
            } else {
                Operation::Difference
            },
        )
        .store(m, heap, h)?;
        if Region::read(m, h)?.bands.is_empty() {
            events.remove_update(address);
        } else if invalidate {
            Self::event(events, 6, address, false)?;
        }
        Ok(true)
    }
    pub fn invalidate<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        events: &mut Events,
        r: Rect,
    ) -> Result<(), String> {
        self.damage(m, heap, q, events, &Region::rectangle(r), true)
            .map(|_| ())
    }
    /// Update events describe outstanding damage, rather than consuming it.
    pub fn next_event<B: AddressBus>(
        &self,
        m: &mut B,
        events: &mut Events,
        mask: u16,
        remove: bool,
    ) -> Result<(bool, Event), String> {
        let mut pending = Vec::new();
        for w in &self.windows {
            let h = m.read_long(w.address + 122);
            if Region::read(m, h)?.bands.is_empty() || !w.visible {
                events.remove_update(w.address);
            } else {
                pending.push(w.address);
            }
        }
        let (found, event) = events.next(mask, remove);
        if found {
            return Ok((found, event));
        }
        if mask & (1 << 6) != 0 {
            if let Some(&address) = pending.first() {
                return Ok((
                    true,
                    Event {
                        what: 6,
                        message: address,
                        when: events.ticks,
                        v: events.mouse.0,
                        h: events.mouse.1,
                        modifiers: events.modifiers & !1,
                    },
                ));
            }
        }
        Ok((false, event))
    }
    #[cfg(feature = "gui")]
    pub fn update_depth(&self) -> usize {
        self.updating.len()
    }
    pub fn begin_update<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        address: u32,
    ) -> Result<(), String> {
        if self.updating.contains_key(&address) {
            return Err("nested BeginUpdate for window".into());
        }
        if !self.windows.iter().any(|w| w.address == address) {
            return Err("unknown update window".into());
        }
        let vis = m.read_long(address + 24);
        let saved = Region::read(m, vis)?;
        let bitmap = q.port_bitmap(m, address)?;
        let bounds = crate::quickdraw::read_rect(m, bitmap + 6);
        let dh = bounds
            .left
            .checked_neg()
            .ok_or("update horizontal offset overflow")?;
        let dv = bounds
            .top
            .checked_neg()
            .ok_or("update vertical offset overflow")?;
        let h = m.read_long(address + 122);
        let update = Region::read(m, h)?;
        saved
            .offset(dh, dv)?
            .combine(&update, Operation::Intersection)
            .store(m, heap, vis)?;
        // Native offsets even empty rectangular headers after intersection.
        crate::regions::offset_handle(m, heap, vis, bounds.left, bounds.top)?;
        Region::default().store(m, heap, h)?;
        self.updating.insert(address, saved);
        Ok(())
    }
    pub fn end_update<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        address: u32,
    ) -> Result<(), String> {
        let saved = self
            .updating
            .remove(&address)
            .ok_or("EndUpdate without BeginUpdate")?;
        let vis = m.read_long(address + 24);
        saved.store(m, heap, vis)
    }
    pub fn grow_icon<B: AddressBus>(
        &self,
        m: &mut B,
        q: &QuickDraw,
        address: u32,
    ) -> Result<(), String> {
        if !self.windows.iter().any(|w| w.address == address) {
            return Err("DrawGrowIcon for unknown window".into());
        }
        let old = q.get_port(m)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(address + i)).collect();
        q.set_port(m, address)?;
        q.pen_size(m, 1, 1)?;
        q.set_word(m, 56, 8)?;
        q.set_word(m, 66, 0)?;
        let r = crate::quickdraw::read_rect(m, address + 16);
        let render = (|| {
            let square = Rect {
                top: r.bottom - 15,
                left: r.right - 15,
                bottom: r.bottom,
                right: r.right,
            };
            q.paint_rect(m, square, [0; 8], 8)?;
            q.frame_rect(m, square)?;
            for inset in [3, 6, 9] {
                q.frame_rect(
                    m,
                    Rect {
                        top: r.bottom - inset - 2,
                        left: r.right - inset - 2,
                        bottom: r.bottom - 2,
                        right: r.right - 2,
                    },
                )?;
            }
            Ok(())
        })();
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(address + i as u32, *b);
        }
        q.set_port(m, old)?;
        render
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &mut Events,
        storage: u32,
        bounds: Rect,
        title: &[u8],
        visible: bool,
        proc_id: i16,
        behind: u32,
        go_away: bool,
        refcon: u32,
    ) -> Result<u32, String> {
        if ![0, 1, 2, 4, 8, 12].contains(&proc_id) {
            return Err(format!("unsupported window definition {proc_id}"));
        }
        if bounds.bottom <= bounds.top || bounds.right <= bounds.left {
            return Err("empty window bounds".into());
        }
        if self.manager_port.is_none() {
            return Err("Window Manager not initialized".into());
        }
        let old = q.get_port(m)?;
        let owned = storage == 0;
        let p = if owned {
            heap.new_ptr(m, 156, true).map_err(|e| e.to_string())?
        } else {
            storage
        };
        for i in 0..156 {
            m.write_byte(p + i, 0);
        }
        q.open_port(m, heap, p)?;
        let local = Rect {
            top: 0,
            left: 0,
            bottom: bounds.bottom - bounds.top,
            right: bounds.right - bounds.left,
        };
        write_rect(m, p + 16, local);
        write_rect(
            m,
            p + 8,
            Rect {
                top: -bounds.top,
                left: -bounds.left,
                bottom: q.height - bounds.top,
                right: q.width - bounds.left,
            },
        );
        let vis = m.read_long(p + 24);
        if visible {
            Region::rectangle(local)
        } else {
            Region::default()
        }
        .store(m, heap, vis)?;
        m.write_word(p + 108, 8);
        m.write_byte(p + 110, visible as u8);
        m.write_byte(p + 112, go_away as u8);
        m.write_long(p + 152, refcon);
        let structure = structure_bounds(bounds, proc_id);
        for (offset, r) in [
            (114, structure),
            (118, bounds),
            (
                122,
                if visible {
                    bounds
                } else {
                    Rect {
                        top: 0,
                        left: 0,
                        bottom: 0,
                        right: 0,
                    }
                },
            ),
        ] {
            let r = if visible {
                r
            } else {
                Rect {
                    top: 0,
                    left: 0,
                    bottom: 0,
                    right: 0,
                }
            };
            let h = region(m, heap, r)?;
            m.write_long(p + offset, h);
        }
        let length = title.len().min(255);
        let title_h = heap
            .new_handle(m, (length + 1) as u32, false)
            .map_err(|e| e.to_string())?;
        let title_p = heap.data(title_h).map_err(|e| e.to_string())?;
        m.write_byte(title_p, length as u8);
        for (i, b) in title[..length].iter().enumerate() {
            m.write_byte(title_p + 1 + i as u32, *b);
        }
        m.write_long(p + 134, title_h);
        m.write_word(p + 138, fonts.text_width(0, 0, 0, &title[..length])? as u16);
        let index = if behind == u32::MAX {
            0
        } else if behind == 0 {
            self.windows.len()
        } else {
            self.windows
                .iter()
                .position(|w| w.address == behind)
                .ok_or("invalid window insertion target")?
                + 1
        };
        self.windows.insert(
            index,
            Window {
                address: p,
                bounds,
                title: title[..length].to_vec(),
                visible,
                owned,
                proc_id,
            },
        );
        self.links(m);
        self.hilites(m);
        q.set_port(m, old)?;
        if visible {
            let saved = q.get_port(m)?;
            q.set_port(m, self.manager_port.unwrap())?;
            q.paint_rect(m, bounds, [0; 8], 8)?;
            q.set_port(m, saved)?;
            self.draw(m, q, fonts, p)?;
            events
                .post(Event {
                    what: 6,
                    message: p,
                    when: events.ticks,
                    v: events.mouse.0,
                    h: events.mouse.1,
                    modifiers: events.modifiers,
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(p)
    }
    pub fn draw<B: AddressBus>(
        &self,
        m: &mut B,
        q: &QuickDraw,
        fonts: &Fonts,
        address: u32,
    ) -> Result<(), String> {
        let w = self
            .windows
            .iter()
            .find(|w| w.address == address)
            .ok_or("unknown window")?;
        if !w.visible {
            return Ok(());
        }
        let old = q.get_port(m)?;
        q.set_port(m, self.manager_port.ok_or("no window manager port")?)?;
        let manager = self.manager_port.ok_or("no window manager port")?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(manager + i)).collect();
        q.pen_size(m, 1, 1)?;
        q.set_word(m, 56, 8)?;
        q.set_word(m, 66, 0)?;
        m.write_long(manager + 80, 33);
        m.write_long(manager + 84, 30);
        let b = w.bounds;
        let structure = structure_bounds(b, w.proc_id);
        if w.proc_id == 2 {
            q.frame_rect(m, structure)?;
            for (i, value) in saved.iter().enumerate() {
                m.write_byte(manager + i as u32, *value);
            }
            q.set_port(m, old)?;
            return Ok(());
        }
        if w.proc_id == 1 {
            q.pen_size(m, 2, 2)?;
            q.frame_rect(m, structure)?;
            q.pen_size(m, 1, 1)?;
            q.frame_rect(
                m,
                Rect {
                    top: b.top - 1,
                    left: b.left - 1,
                    bottom: b.bottom + 1,
                    right: b.right + 1,
                },
            )?;
            for (i, value) in saved.iter().enumerate() {
                m.write_byte(manager + i as u32, *value);
            }
            q.set_port(m, old)?;
            return Ok(());
        }
        // Paint only the frame strips, preserving application content.
        for r in [
            Rect {
                bottom: b.top,
                ..structure
            },
            Rect {
                top: b.bottom,
                ..structure
            },
            Rect {
                top: b.top,
                bottom: b.bottom,
                right: b.left,
                ..structure
            },
            Rect {
                top: b.top,
                bottom: b.bottom,
                left: b.right,
                ..structure
            },
        ] {
            q.paint_rect(m, r, [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55], 8)?;
        }
        q.frame_rect(m, structure)?;
        q.frame_rect(
            m,
            Rect {
                top: b.top - 1,
                left: b.left - 1,
                bottom: b.bottom + 1,
                right: b.right + 1,
            },
        )?;
        q.set_word(m, 68, 0)?;
        q.set_word(m, 70, 0)?;
        q.set_word(m, 72, 1)?;
        q.set_word(m, 74, 12)?;
        let width = fonts.text_width(0, 12, 0, &w.title)?;
        let title_x = b.left + (b.right - b.left - width) / 2;
        q.paint_rect(
            m,
            Rect {
                top: b.top - 18,
                left: title_x - 3,
                bottom: b.top - 3,
                right: title_x + width + 3,
            },
            [0; 8],
            8,
        )?;
        q.move_to(m, b.left + (b.right - b.left - width) / 2, b.top - 6)?;
        q.text(m, fonts, &w.title)?;
        if m.read_byte(address + 112) != 0 {
            q.paint_rect(
                m,
                Rect {
                    top: b.top - 17,
                    left: b.left - 1,
                    bottom: b.top - 6,
                    right: b.left + 10,
                },
                [0; 8],
                8,
            )?;
            q.frame_rect(
                m,
                Rect {
                    top: b.top - 17,
                    left: b.left - 1,
                    bottom: b.top - 6,
                    right: b.left + 10,
                },
            )?;
        }
        for (i, byte) in saved.iter().enumerate() {
            m.write_byte(manager + i as u32, *byte);
        }
        q.set_port(m, old)
    }
    pub fn dispose<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        events: &mut Events,
        address: u32,
    ) -> Result<(), String> {
        self.remove(m, heap, q, events, address, true)
    }
    fn remove<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        events: &mut Events,
        address: u32,
        release_record: bool,
    ) -> Result<(), String> {
        let index = self
            .windows
            .iter()
            .position(|w| w.address == address)
            .ok_or("unknown window")?;
        let w = self.windows.remove(index);
        if let Some(i) = self
            .auxiliary
            .iter()
            .position(|&(owner, _)| owner == address)
        {
            let (_, h) = self.auxiliary.remove(i);
            heap.dispose(m, h).map_err(|e| e.to_string())?;
            self.link_auxiliary(m);
        }
        events.remove_window(address);
        q.close_port(m, heap, address)?;
        for offset in [114, 118, 122, 134] {
            let h = m.read_long(address + offset);
            heap.dispose(m, h).map_err(|e| e.to_string())?;
        }
        if w.owned && release_record {
            heap.dispose_ptr(address).map_err(|e| e.to_string())?;
        }
        self.links(m);
        Ok(())
    }
}

impl Windows {
    fn default_colors<B: AddressBus>(&mut self, m: &mut B, heap: &mut Heap) -> Result<u32, String> {
        if let Some(h) = self.default_table {
            return Ok(h);
        }
        // Explicit standard-window API profile measured on the reference. Frame
        // artwork remains the runtime's owned rendering; these are RGB facts.
        let colors = [
            [65535; 3],
            [0; 3],
            [0; 3],
            [0; 3],
            [65535; 3],
            [65535; 3],
            [0; 3],
            [65535; 3],
            [0; 3],
            [0xcccc, 0xcccc, 65535],
            [0; 3],
            [0xcccc, 0xcccc, 65535],
            [0x3333, 0x3333, 0x6666],
        ];
        let h = heap
            .new_handle(m, 8 + 13 * 8, true)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        m.write_word(p + 6, 12);
        for (i, color) in colors.iter().enumerate() {
            let entry = p + 8 + i as u32 * 8;
            m.write_word(entry, i as u16);
            for (j, v) in color.iter().enumerate() {
                m.write_word(entry + 2 + j as u32 * 2, *v);
            }
        }
        self.default_table = Some(h);
        Ok(h)
    }
    fn default_auxiliary<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<u32, String> {
        if let Some(h) = self.default_auxiliary {
            return Ok(h);
        }
        let table = self.default_colors(m, heap)?;
        let h = heap.new_handle(m, 28, true).map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        m.write_long(p + 8, table);
        self.default_auxiliary = Some(h);
        self.link_auxiliary(m);
        Ok(h)
    }
    fn link_auxiliary<B: AddressBus>(&self, m: &mut B) {
        for (i, &(_, h)) in self.auxiliary.iter().enumerate() {
            let next = self
                .auxiliary
                .get(i + 1)
                .map(|&(_, h)| h)
                .or(self.default_auxiliary)
                .unwrap_or(0);
            let p = m.read_long(h);
            m.write_long(p, next);
        }
        m.write_long(
            0xcd0,
            self.auxiliary
                .first()
                .map(|&(_, h)| h)
                .or(self.default_auxiliary)
                .unwrap_or(0),
        );
    }
    fn ensure_auxiliary<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        w: u32,
    ) -> Result<u32, String> {
        if let Some(&(_, h)) = self.auxiliary.iter().find(|&&(owner, _)| owner == w) {
            return Ok(h);
        }
        let variant = self
            .windows
            .iter()
            .find(|window| window.address == w)
            .ok_or("color service on unknown window")?
            .proc_id
            & 15;
        let default = self.default_colors(m, heap)?;
        self.default_auxiliary(m, heap)?;
        let h = heap.new_handle(m, 28, true).map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        m.write_long(p + 4, w);
        m.write_long(p + 8, default);
        m.write_long(p + 16, (variant as u32) << 24);
        self.auxiliary.insert(0, (w, h));
        self.link_auxiliary(m);
        Ok(h)
    }
    fn get_auxiliary<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        w: u32,
    ) -> Result<(bool, u32), String> {
        if let Some(&(_, h)) = self.auxiliary.iter().find(|&&(owner, _)| owner == w) {
            return Ok((true, h));
        }
        if w != 0 && !self.windows.iter().any(|window| window.address == w) {
            return Err("GetAuxWin on unknown window".into());
        }
        Ok((w == 0, self.default_auxiliary(m, heap)?))
    }
    pub fn colorize<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &mut QuickDraw,
        fonts: &Fonts,
        w: u32,
        table: Option<u32>,
    ) -> Result<(), String> {
        q.make_color(m, heap, w)?;
        self.ensure_auxiliary(m, heap, w)?;
        let table = if let Some(h) = table {
            h
        } else {
            self.default_colors(m, heap)?
        };
        self.set_window_color(m, heap, q, fonts, w, table)
    }
    fn set_window_color<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        w: u32,
        table: u32,
    ) -> Result<(), String> {
        if w == 0 || table == 0 {
            return Err("SetWinColor default mutation/nil table outside measured profile".into());
        }
        if !self.windows.iter().any(|window| window.address == w)
            || m.read_word(w + 6) & 0xc000 != 0xc000
        {
            return Err("SetWinColor requires an existing color window".into());
        }
        let size = heap
            .size(table)
            .map_err(|e| format!("window color table: {e}"))?;
        let p = m.read_long(table);
        if size < 8 {
            return Err("truncated window color table".into());
        }
        let n = m.read_word(p + 6) as u32 + 1;
        if n > 256 || 8 + n * 8 > size {
            return Err("truncated/oversized window color entries".into());
        }
        let entry = (0..n)
            .map(|i| p + 8 + i * 8)
            .find(|&at| m.read_word(at) == 0)
            .ok_or("window color table has no content color")?;
        let color = [
            m.read_word(entry + 2),
            m.read_word(entry + 4),
            m.read_word(entry + 6),
        ];
        let default = self.default_colors(m, heap)?;
        let dp = m.read_long(default);
        let same = m.read_word(dp + 6) as u32 + 1 == n
            && (0..4 + n * 8).all(|i| m.read_byte(dp + 4 + i) == m.read_byte(p + 4 + i));
        let h = self.ensure_auxiliary(m, heap, w)?;
        let ap = m.read_long(h);
        m.write_long(ap + 8, if same { default } else { table });
        for (i, v) in color.iter().enumerate() {
            m.write_word(w + 42 + i as u32 * 2, *v);
        }
        let index = q.color_index(m, color)?;
        m.write_long(w + 84, index as u32);
        // With standard windows under the reference Appearance Manager, frame
        // and text parts are ignored; content RGB changes, foreground does not.
        self.draw(m, q, fonts, w)?;
        if self
            .windows
            .iter()
            .any(|window| window.address == w && window.visible)
        {
            self.paint_content(m, q, w)?;
        }
        Ok(())
    }
    fn paint_content<B: AddressBus>(&self, m: &mut B, q: &QuickDraw, w: u32) -> Result<(), String> {
        let mut color = [65535; 3];
        if let Some(&(_, h)) = self.auxiliary.iter().find(|&&(owner, _)| owner == w) {
            let ap = m.read_long(h);
            let table = m.read_long(ap + 8);
            let p = m.read_long(table);
            let n = m.read_word(p + 6) as u32 + 1;
            if n > 256 {
                return Err("oversized window color table".into());
            }
            if let Some(entry) = (0..n)
                .map(|i| p + 8 + i * 8)
                .find(|&at| m.read_word(at) == 0)
            {
                color = [
                    m.read_word(entry + 2),
                    m.read_word(entry + 4),
                    m.read_word(entry + 6),
                ];
            }
        }
        let surface = q.port_surface(m, w)?;
        let index = if surface.depth == 1 {
            classic_core::pixels::nearest(color, &[[65535; 3], [0; 3]]).unwrap()
        } else {
            q.color_index(m, color)?
        };
        let vis = m.read_long(w + 24);
        // The Window Manager uses wContentColor, independently of the app's
        // current RGB background and clip. Do not mutate the app's port state.
        for band in Region::read(m, vis)?.bands {
            for y in band.top..band.bottom {
                for &(left, right) in &band.spans {
                    for x in left..right {
                        surface.write(m, x, y, index, 0);
                    }
                }
            }
        }
        Ok(())
    }
}
