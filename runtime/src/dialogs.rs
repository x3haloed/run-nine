//! Dialog items retain public handles and execute application callbacks elsewhere.
use crate::{
    controls::Controls,
    events::Events,
    fonts::Fonts,
    managers::{Heap, Rect},
    quickdraw::{read_rect, write_rect, QuickDraw},
    resource::Resource,
    windows::Windows,
};
use m68k::AddressBus;
use std::collections::{BTreeMap, VecDeque};

pub fn item_text_dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    heap: &mut Heap,
) -> Result<bool, String> {
    if op != 0xa990 { return Ok(false); }
    let sp = cpu.dar[15];
    let out = m.read_long(sp);
    let handle = m.read_long(sp + 4);
    if out == 0 { return Err("GetDialogItemText with null output".into()); }
    let count = heap.size(handle).map_err(|e| format!("dialog text handle: {e}"))?.min(255);
    let ptr = heap.data(handle).map_err(|e| format!("dialog text handle: {e}"))?;
    let bytes: Vec<_> = (0..count).map(|i| m.read_byte(ptr + i)).collect();
    m.write_byte(out, count as u8);
    for (i, byte) in bytes.into_iter().enumerate() { m.write_byte(out + 1 + i as u32, byte); }
    m.write_word(0x220, 0);
    cpu.dar[0] = 0;
    cpu.set_ccr(4);
    cpu.dar[15] += 8;
    cpu.pc += 2;
    Ok(true)
}

pub struct Item {
    pub kind: u8,
    pub handle: u32,
    pub rect: Rect,
}
pub struct Dialog {
    pub items: Vec<Item>,
    list: u32,
    cancel_item: usize,
    color_table: Option<u32>,
}
pub struct Dialogs {
    pub records: BTreeMap<u32, Dialog>,
    draw_items: VecDeque<(u32, u16, u32)>,
    drawing_port: Option<u32>,
}
fn word(bytes: &[u8], p: usize) -> Result<i16, String> {
    Ok(i16::from_be_bytes(
        bytes
            .get(p..p + 2)
            .ok_or("truncated dialog resource")?
            .try_into()
            .unwrap(),
    ))
}
fn resource(resources: &[Resource<'_>], kind: [u8; 4], id: i16) -> Result<Vec<u8>, String> {
    let r = resources
        .iter()
        .find(|r| r.kind == kind && r.id == id)
        .ok_or_else(|| format!("missing {:?} {id}", String::from_utf8_lossy(&kind)))?;
    Ok(
        crate::vendor::compressed::decompress_if_needed(r.attributes, r.data)
            .map_err(|e| format!("dialog decompression: {e:?}"))?
            .unwrap_or_else(|| r.data.to_vec()),
    )
}
impl Dialogs {
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_lifecycle<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &mut QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        te: &mut crate::textedit::TextEdit,
        resources: &[Resource<'_>],
    ) -> Result<bool, String> {
        let sp = cpu.dar[15];
        if op == 0xa97c {
            let behind = m.read_long(sp);
            let storage = m.read_long(sp + 4);
            let id = m.read_word(sp + 8) as i16;
            let address = self.from_resource(
                m, heap, q, fonts, windows, controls, events, te, resources, id, storage, behind,
            )?;
            m.write_long(sp + 10, address);
            m.write_word(0xa60, 0);
            cpu.dar[0] = address;
            cpu.set_ccr(if address == 0 { 4 } else { 0 });
            cpu.dar[15] += 10;
            cpu.pc += 2;
            return Ok(true);
        }
        if matches!(op, 0xa982 | 0xa983) {
            let address = m.read_long(sp);
            if !self.records.contains_key(&address) {
                return Err("closing an unknown dialog".into());
            }
            let text = m.read_long(address + 160);
            if text != 0 {
                te.dispose(m, heap, text)?;
                m.write_long(address + 160, 0);
            }
            self.close_record(
                m,
                heap,
                q,
                fonts,
                windows,
                controls,
                events,
                address,
                op == 0xa983,
            )?;
            cpu.dar[15] += 4;
            cpu.pc += 2;
            return Ok(true);
        }
        Ok(false)
    }
    #[allow(clippy::too_many_arguments)]
    fn from_resource<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &mut QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        te: &mut crate::textedit::TextEdit,
        resources: &[Resource<'_>],
        id: i16,
        storage: u32,
        behind: u32,
    ) -> Result<u32, String> {
        if !resources.iter().any(|r| r.kind == *b"DLOG" && r.id == id) {
            return Ok(0);
        }
        let dlog = resource(resources, *b"DLOG", id)?;
        let list = word(&dlog, 18)?;
        // A successful resource lookup clears MemErr even when its DITL is absent.
        m.write_word(0x220, 0);
        if !resources.iter().any(|r| r.kind == *b"DITL" && r.id == list) {
            return Ok(0);
        }
        let data = resource(resources, *b"DITL", list)?;
        let mut bounds = Rect {
            top: word(&dlog, 0)?,
            left: word(&dlog, 2)?,
            bottom: word(&dlog, 4)?,
            right: word(&dlog, 6)?,
        };
        let proc_id = word(&dlog, 8)?;
        let visible = word(&dlog, 10)? != 0;
        let go_away = word(&dlog, 12)? != 0 && !matches!(proc_id, 1 | 2);
        let refcon = u32::from_be_bytes(
            dlog.get(14..18)
                .ok_or("truncated dialog refcon")?
                .try_into()
                .unwrap(),
        );
        let length = usize::from(*dlog.get(20).ok_or("missing dialog title")?);
        let title = dlog.get(21..21 + length).ok_or("truncated dialog title")?;
        let position_at = (21 + length + 1) & !1;
        let position = if dlog.len() >= position_at + 2 {
            word(&dlog, position_at)? as u16
        } else {
            0
        };
        match position {
            0 => {}
            0x280a | 0x680a => {
                // The owned desktop has one active screen. Parent-screen and
                // main-screen placement therefore use the same device bounds.
                let height = bounds.bottom - bounds.top;
                let width = bounds.right - bounds.left;
                let structure = crate::windows::structure_bounds(bounds, proc_id);
                let outer_width = structure.right - structure.left;
                let outer_height = structure.bottom - structure.top;
                let left = (q.width - outer_width) / 2 + bounds.left - structure.left;
                let top = windows.menu_height
                    + (q.height - windows.menu_height - outer_height) / 2
                    + bounds.top - structure.top;
                bounds = Rect {
                    top,
                    left,
                    bottom: top + height,
                    right: left + width,
                };
            }
            _ => {
                return Err(format!(
                    "GetNewDialog position {position:#x} is not implemented"
                ))
            }
        }
        let old = q.get_port(m)?;
        let address = self.create_items_config(
            m, heap, q, fonts, windows, controls, events, resources, data, bounds, proc_id, refcon,
            storage, title, visible, behind, go_away,
        )?;
        if resources.iter().any(|r| r.kind == *b"dctb" && r.id == id) {
            let bytes = resource(resources, *b"dctb", id)?;
            let table = if bytes.len() == 8 && bytes[6..8] == [255, 255] {
                None
            } else {
                let h = heap
                    .new_handle(m, bytes.len() as u32, false)
                    .map_err(|e| e.to_string())?;
                let p = heap.data(h).map_err(|e| e.to_string())?;
                for (i, b) in bytes.iter().enumerate() {
                    m.write_byte(p + i as u32, *b);
                }
                Some(h)
            };
            windows.colorize(m, heap, q, fonts, address, table)?;
            self.records.get_mut(&address).unwrap().color_table = table;
        }
        // Dialog editing inherits the system font and its default-size sentinel.
        m.write_word(address + 68, 0);
        m.write_word(address + 74, 0);
        m.write_word(address + 166, 0);
        q.set_port(m, address)?;
        let rect = read_rect(m, address + 16);
        let result = te.create(m, heap, q, fonts, rect, rect);
        q.set_port(m, old)?;
        let text = result?;
        let record = heap.data(text).map_err(|e| e.to_string())?;
        m.write_word(record + 80, 0);
        m.write_long(address + 160, text);
        if visible {
            self.draw(m, heap, q, fonts, controls, address)?;
        }
        Ok(address)
    }
    pub fn new() -> Self {
        Self {
            records: BTreeMap::new(),
            draw_items: VecDeque::new(),
            drawing_port: None,
        }
    }
    fn expanded<B: AddressBus>(&self, m: &mut B, text: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        let mut i = 0;
        while i < text.len() {
            if text[i] == b'^' && i + 1 < text.len() && (b'0'..=b'3').contains(&text[i + 1]) {
                let h = m.read_long(0xaa0 + (text[i + 1] - b'0') as u32 * 4);
                if h != 0 {
                    let p = m.read_long(h);
                    if p != 0 {
                        let n = m.read_byte(p) as u32;
                        for j in 0..n {
                            result.push(m.read_byte(p + 1 + j));
                        }
                    }
                }
                i += 2;
            } else {
                result.push(text[i]);
                i += 1;
            }
        }
        result
    }
    /// Describe an alert request without displaying it or supplying a result.
    pub fn alert_description<B: AddressBus>(
        &self,
        m: &mut B,
        resources: &[Resource<'_>],
        id: i16,
    ) -> Result<Vec<String>, String> {
        let alert = resource(resources, *b"ALRT", id)?;
        let list = resource(resources, *b"DITL", word(&alert, 8)?)?;
        let count = word(&list, 0)? as i32 + 1;
        let mut at = 2;
        let mut text = Vec::new();
        for _ in 0..count {
            let header = list.get(at..at + 14).ok_or("truncated alert item")?;
            let length = header[13] as usize;
            let bytes = list
                .get(at + 14..at + 14 + length)
                .ok_or("truncated alert item text")?;
            if header[12] & 0x7f == 8 {
                text.push(
                    self.expanded(m, bytes)
                        .iter()
                        .map(|b| crate::mac_roman::MAC_ROMAN[*b as usize])
                        .collect(),
                );
            }
            at += 14 + length + (length & 1);
        }
        Ok(text)
    }
    pub fn parameter_text<B: AddressBus>(
        m: &mut B,
        heap: &mut Heap,
        parameters: [Option<Vec<u8>>; 4],
    ) -> Result<(), String> {
        for (i, text) in parameters.iter().enumerate() {
            let Some(text) = text else {
                continue;
            };
            if text.len() > 255 {
                return Err("dialog parameter exceeds Pascal string limit".into());
            }
            let slot = 0xaa0 + i as u32 * 4;
            let mut h = m.read_long(slot);
            if h == 0 {
                h = heap
                    .new_handle(m, text.len() as u32 + 1, false)
                    .map_err(|e| e.to_string())?;
                m.write_long(slot, h);
            } else {
                heap.resize(m, h, text.len() as u32 + 1)
                    .map_err(|e| e.to_string())?;
            }
            let p = heap.data(h).map_err(|e| e.to_string())?;
            m.write_byte(p, text.len() as u8);
            for (j, b) in text.iter().enumerate() {
                m.write_byte(p + 1 + j as u32, *b);
            }
        }
        Ok(())
    }
    /// Independent standard Open template, using the published classic item
    /// contract and OS9-observed public layout. No system resource binary used.
    #[allow(clippy::too_many_arguments)]
    pub fn standard_open<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
    ) -> Result<u32, String> {
        let mut dlog = vec![0; 20];
        dlog[4..6].copy_from_slice(&166i16.to_be_bytes());
        dlog[6..8].copy_from_slice(&344i16.to_be_bytes());
        dlog[18..20].copy_from_slice(&(-2i16).to_be_bytes());
        let mut ditl = 8u16.to_be_bytes().to_vec();
        for (kind, rect, data) in [
            (4u8, [135i16, 252, 155, 332], b"Open".as_slice()),
            (4, [104, 252, 124, 332], b"Cancel".as_slice()),
            (129, [0, 0, 0, 0], &[]),
            (0, [8, 235, 24, 337], &[]),
            (4, [32, 252, 52, 332], b"Eject".as_slice()),
            (4, [60, 252, 80, 332], b"Desktop".as_slice()),
            (0, [29, 12, 159, 230], &[]),
            (0, [6, 12, 25, 230], &[]),
            (192, [91, 251, 92, 333], &[0, 11]),
        ] {
            ditl.extend_from_slice(&[0; 4]);
            for v in rect {
                ditl.extend_from_slice(&v.to_be_bytes());
            }
            ditl.push(kind);
            ditl.push(data.len() as u8);
            ditl.extend_from_slice(data);
            if data.len() % 2 != 0 {
                ditl.push(0);
            }
        }
        let resources = [
            Resource {
                kind: *b"DLOG",
                id: -2,
                name: None,
                attributes: 0,
                data: &dlog,
                map: &[],
                reference_offset: 0,
            },
            Resource {
                kind: *b"DITL",
                id: -2,
                name: None,
                attributes: 0,
                data: &ditl,
                map: &[],
                reference_offset: 0,
            },
        ];
        self.create(
            m,
            heap,
            q,
            fonts,
            windows,
            controls,
            events,
            &resources,
            -2,
            u32::from_be_bytes(*b"stdf"),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn replacement<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        name: &[u8],
    ) -> Result<u32, String> {
        Self::parameter_text(m, heap, [Some(name.to_vec()), None, None, None])?;
        let mut dlog = vec![0; 20];
        dlog[4..6].copy_from_slice(&118i16.to_be_bytes());
        dlog[6..8].copy_from_slice(&204i16.to_be_bytes());
        dlog[18..20].copy_from_slice(&(-1i16).to_be_bytes());
        let mut ditl = 2u16.to_be_bytes().to_vec();
        for (kind, rect, text) in [
            (4u8, [84i16, 124, 104, 194], b"Replace".as_slice()),
            (4u8, [84i16, 14, 104, 84], b"Cancel".as_slice()),
            (
                136u8,
                [10i16, 10, 76, 194],
                b"Replace existing \xd2^0\xd3 ?".as_slice(),
            ),
        ] {
            ditl.extend_from_slice(&[0; 4]);
            for v in rect {
                ditl.extend_from_slice(&v.to_be_bytes());
            }
            ditl.push(kind);
            ditl.push(text.len() as u8);
            ditl.extend_from_slice(text);
            if text.len() % 2 != 0 {
                ditl.push(0);
            }
        }
        let resources = [
            Resource {
                kind: *b"DLOG",
                id: -1,
                name: None,
                attributes: 0,
                data: &dlog,
                map: &[],
                reference_offset: 0,
            },
            Resource {
                kind: *b"DITL",
                id: -1,
                name: None,
                attributes: 0,
                data: &ditl,
                map: &[],
                reference_offset: 0,
            },
        ];
        self.create(
            m,
            heap,
            q,
            fonts,
            windows,
            controls,
            events,
            &resources,
            -1,
            u32::from_be_bytes(*b"rplc"),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        resources: &[Resource<'_>],
        id: i16,
        refcon: u32,
    ) -> Result<u32, String> {
        let dlog = resource(resources, *b"DLOG", id)?;
        let list_id = word(&dlog, 18)?;
        let data = resource(resources, *b"DITL", list_id)?;
        let height = word(&dlog, 4)? - word(&dlog, 0)?;
        let width = word(&dlog, 6)? - word(&dlog, 2)?;
        if height <= 0 || width <= 0 {
            return Err("empty dialog resource bounds".into());
        }
        let top = (q.height - height) / 3;
        let left = (q.width - width) / 2;
        self.create_items(
            m,
            heap,
            q,
            fonts,
            windows,
            controls,
            events,
            resources,
            data,
            Rect {
                top,
                left,
                bottom: top + height,
                right: left + width,
            },
            0,
            refcon,
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub fn create_alert<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        resources: &[Resource<'_>],
        id: i16,
    ) -> Result<u32, String> {
        let alert = resource(resources, *b"ALRT", id)?;
        let stages = word(&alert, 10)? as u16;
        if stages != 0x5555 {
            return Err(format!("Alert stage table {stages:#x} is not implemented"));
        }
        let mut bounds = Rect {
            top: word(&alert, 0)?,
            left: word(&alert, 2)?,
            bottom: word(&alert, 4)?,
            right: word(&alert, 6)?,
        };
        let height = bounds.bottom - bounds.top;
        let width = bounds.right - bounds.left;
        if height <= 0 || width <= 0 {
            return Err("Empty alert bounds".into());
        }
        let position = if alert.len() >= 14 {
            word(&alert, 12)? as u16
        } else {
            0
        };
        match position {
            0 => {}
            0x280a | 0x300a => {
                let left = (q.width - width - 13) / 2 + 6;
                let available = q.height - windows.menu_height - height - 13;
                let top =
                    windows.menu_height + available / if position == 0x280a { 2 } else { 5 } + 6;
                bounds = Rect {
                    top,
                    left,
                    bottom: top + height,
                    right: left + width,
                };
            }
            _ => return Err(format!("Alert position {position:#x} is not implemented")),
        }
        let data = resource(resources, *b"DITL", word(&alert, 8)?)?;
        self.create_items(
            m, heap, q, fonts, windows, controls, events, resources, data, bounds, 1, 0,
        )
    }
    fn create_items<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        resources: &[Resource<'_>],
        data: Vec<u8>,
        bounds: Rect,
        proc_id: i16,
        refcon: u32,
    ) -> Result<u32, String> {
        self.create_items_config(
            m,
            heap,
            q,
            fonts,
            windows,
            controls,
            events,
            resources,
            data,
            bounds,
            proc_id,
            refcon,
            0,
            b"",
            true,
            u32::MAX,
            false,
        )
    }
    #[allow(clippy::too_many_arguments)]
    fn create_items_config<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        resources: &[Resource<'_>],
        mut data: Vec<u8>,
        bounds: Rect,
        proc_id: i16,
        refcon: u32,
        storage: u32,
        title: &[u8],
        visible: bool,
        behind: u32,
        go_away: bool,
    ) -> Result<u32, String> {
        let address = if storage == 0 {
            heap.new_ptr(m, 170, true)
                .map_err(|e| format!("dialog record: {e}"))?
        } else {
            storage
        };
        windows.create(
            m, heap, q, fonts, events, address, bounds, title, visible, proc_id, behind, go_away,
            refcon,
        )?;
        m.write_word(address + 108, 2);
        m.write_word(address + 164, 0xffff);
        m.write_word(address + 168, 1);
        let count = (word(&data, 0)? as i32 + 1) as usize;
        let mut offset = 2;
        let mut items = Vec::new();
        for _ in 0..count {
            if offset + 14 > data.len() {
                return Err("truncated dialog item".into());
            }
            let rect = Rect {
                top: word(&data, offset + 4)?,
                left: word(&data, offset + 6)?,
                bottom: word(&data, offset + 8)?,
                right: word(&data, offset + 10)?,
            };
            let kind = data[offset + 12];
            let len = data[offset + 13] as usize;
            let text = data
                .get(offset + 14..offset + 14 + len)
                .ok_or("truncated dialog item data")?
                .to_vec();
            let basic = kind & 0x7f;
            let handle = match basic {
                4 | 5 | 6 => controls.create(
                    m,
                    heap,
                    address,
                    rect,
                    &text,
                    true,
                    0,
                    0,
                    if basic == 4 { 0 } else { 1 },
                    (basic - 4) as i16,
                    0,
                )?,
                8 | 16 => {
                    let h = heap
                        .new_handle(m, len as u32, false)
                        .map_err(|e| e.to_string())?;
                    let p = m.read_long(h);
                    for (i, b) in text.iter().enumerate() {
                        m.write_byte(p + i as u32, *b);
                    }
                    h
                }
                32 => {
                    if len != 2 {
                        return Err("invalid icon item resource ID".into());
                    }
                    let id = i16::from_be_bytes(text[..2].try_into().unwrap());
                    let bytes = if resources.iter().any(|r| r.kind == *b"ICON" && r.id == id) {
                        resource(resources, *b"ICON", id)?
                    } else {
                        crate::icons::alert_bitmap(id)
                            .ok_or_else(|| format!("missing ICON {id}"))?
                    };
                    let h = heap
                        .new_handle(m, bytes.len() as u32, false)
                        .map_err(|e| e.to_string())?;
                    let p = m.read_long(h);
                    for (i, b) in bytes.iter().enumerate() {
                        m.write_byte(p + i as u32, *b);
                    }
                    h
                }
                _ => 0,
            };
            data[offset..offset + 4].copy_from_slice(&handle.to_be_bytes());
            items.push(Item { kind, handle, rect });
            offset += 14 + len + (len & 1);
        }
        let list = heap
            .new_handle(m, data.len() as u32, false)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(list);
        for (i, b) in data.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        m.write_long(address + 156, list);
        self.records.insert(
            address,
            Dialog {
                items,
                list,
                cancel_item: 0,
                color_table: None,
            },
        );
        if visible {
            self.draw(m, heap, q, fonts, controls, address)?;
        }
        Ok(address)
    }
    pub fn set_text<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        address: u32,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), String> {
        let dialog = self.records.get(&address).ok_or("unknown dialog")?;
        let item = dialog
            .items
            .get(index.checked_sub(1).ok_or("dialog item zero")?)
            .ok_or("dialog item outside list")?;
        if ![8, 16].contains(&(item.kind & 0x7f)) {
            return Err("text operation on non-text dialog item".into());
        }
        heap.resize(m, item.handle, bytes.len() as u32)
            .map_err(|e| format!("dialog text resize: {e}"))?;
        let p = m.read_long(item.handle);
        for (i, b) in bytes.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        Ok(())
    }
    pub fn draw<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        controls: &Controls,
        address: u32,
    ) -> Result<(), String> {
        let d = self.records.get(&address).ok_or("unknown dialog")?;
        let old = q.get_port(m)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(address + i)).collect();
        q.set_port(m, address)?;
        q.set_word(m, 68, 1)?;
        q.set_word(m, 74, 12)?;
        q.set_word(m, 72, 1)?;
        let render: Result<(), String> = (|| {
            for item in &d.items {
                let basic = item.kind & 0x7f;
                if item.rect.top < 0 || item.rect.left < 0 {
                    continue;
                }
                if [4, 5, 6].contains(&basic) {
                    controls.draw(m, q, item.handle)?;
                    let p = m.read_long(item.handle);
                    let n = m.read_byte(p + 40) as u32;
                    let text: Vec<u8> = (0..n).map(|i| m.read_byte(p + 41 + i)).collect();
                    let width = fonts.text_width(1, 12, 0, &text)?;
                    q.move_to(
                        m,
                        if basic == 4 {
                            item.rect.left + (item.rect.right - item.rect.left - width) / 2
                        } else {
                            item.rect.left + 16
                        },
                        item.rect.top + (item.rect.bottom - item.rect.top - 16) / 2 + 12,
                    )?;
                    q.text(m, fonts, &text)?;
                } else if basic == 8 || basic == 16 {
                    let n = heap.size(item.handle).map_err(|e| e.to_string())?;
                    let p = m.read_long(item.handle);
                    let text: Vec<u8> = (0..n).map(|i| m.read_byte(p + i)).collect();
                    q.paint_rect(m, item.rect, [0; 8], 8)?;
                    if basic == 16 {
                        q.frame_rect(m, item.rect)?;
                    }
                    let text = if basic == 8 {
                        self.expanded(m, &text)
                    } else {
                        text
                    };
                    let mut first = 0;
                    let mut line = 0;
                    while first < text.len() {
                        let mut end = first;
                        let mut space = None;
                        while end < text.len() && text[end] != 13 {
                            if basic == 8
                                && end > first
                                && fonts.text_width(1, 12, 0, &text[first..=end])?
                                    > item.rect.right - item.rect.left - 4
                            {
                                break;
                            }
                            if text[end] == b' ' {
                                space = Some(end);
                            }
                            end += 1;
                        }
                        let newline = end < text.len() && text[end] == 13;
                        if basic == 8 && !newline && end < text.len() {
                            if let Some(at) = space {
                                end = at;
                            }
                        }
                        if end == first && !newline {
                            end += 1;
                        }
                        let y = item.rect.top + 12 + line * 16;
                        if y > item.rect.bottom {
                            break;
                        }
                        q.move_to(m, item.rect.left + 2, y)?;
                        q.text(m, fonts, &text[first..end])?;
                        first = end;
                        if first < text.len() && (text[first] == 13 || text[first] == b' ') {
                            first += 1;
                        }
                        line += 1;
                        if basic == 16 {
                            break;
                        }
                    }
                } else if basic == 32 {
                    let p = m.read_long(item.handle);
                    if heap.size(item.handle).map_err(|e| e.to_string())? < 128 {
                        return Err("short monochrome ICON".into());
                    }
                    for y in 0..32 {
                        for x in 0..32 {
                            let bit = m.read_byte(p + y * 4 + x / 8) & (0x80 >> (x & 7)) != 0;
                            q.transfer(
                                m,
                                address,
                                item.rect.left + x as i16,
                                item.rect.top + y as i16,
                                bit,
                                0,
                            )?;
                        }
                    }
                }
            }
            let default = m.read_word(address + 168) as usize;
            if let Some(item) = default.checked_sub(1).and_then(|i| d.items.get(i)) {
                if item.kind & 0x7f == 4 {
                    let control = m.read_long(item.handle);
                    if m.read_byte(control + 17) != 255 {
                        q.pen_size(m, 2, 2)?;
                        q.set_word(m, 56, 8)?;
                        q.frame_rect(
                            m,
                            Rect {
                                top: item.rect.top - 3,
                                left: item.rect.left - 3,
                                bottom: item.rect.bottom + 3,
                                right: item.rect.right + 3,
                            },
                        )?;
                    }
                }
            }
            Ok(())
        })();
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(address + i as u32, *b);
        }
        q.set_port(m, old)?;
        render?;
        for (i, item) in d.items.iter().enumerate() {
            if item.kind & 0x7f == 0
                && item.handle != 0
                && item.rect.top >= 0
                && item.rect.left >= 0
            {
                self.draw_items
                    .push_back((address, (i + 1) as u16, item.handle));
            }
        }
        Ok(())
    }
    pub fn has_pending_draws(&self) -> bool {
        !self.draw_items.is_empty() || self.drawing_port.is_some()
    }
    /// Run user-item procedures on the application's CPU, before resuming the caller.
    pub fn service_draw<B: AddressBus>(
        &mut self,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        q: &QuickDraw,
        callbacks: &mut crate::callbacks::Callbacks,
    ) -> Result<bool, String> {
        if self.drawing_port.is_some() {
            return Ok(false);
        }
        let Some((dialog, item, target)) = self.draw_items.pop_front() else {
            return Ok(false);
        };
        if !self.records.contains_key(&dialog) {
            return Err("drawing callback for disposed dialog".into());
        }
        let old = q.get_port(m)?;
        q.set_port(m, dialog)?;
        callbacks.call_void(
            cpu,
            m,
            target,
            &[
                crate::callbacks::Arg::Long(dialog),
                crate::callbacks::Arg::Word(item),
            ],
            4,
        )?;
        self.drawing_port = Some(old);
        Ok(true)
    }
    pub fn finish_draw<B: AddressBus>(&mut self, m: &mut B, q: &QuickDraw) -> Result<(), String> {
        let old = self
            .drawing_port
            .take()
            .ok_or("user-item return without a draw continuation")?;
        q.set_port(m, old)
    }
    pub fn dispatch_draw<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        controls: &Controls,
    ) -> Result<bool, String> {
        if op != 0xa981 {
            return Ok(false);
        }
        if self.drawing_port.is_some() {
            return Err(
                "recursive DrawDialog from a user-item procedure is not implemented".into(),
            );
        }
        let dialog = m.read_long(cpu.dar[15]);
        self.draw(m, heap, q, fonts, controls, dialog)?;
        cpu.dar[15] += 4;
        cpu.pc += 2;
        Ok(true)
    }
    pub fn dispose<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        address: u32,
    ) -> Result<(), String> {
        self.close_record(m, heap, q, fonts, windows, controls, events, address, true)
    }
    #[allow(clippy::too_many_arguments)]
    fn close_record<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        windows: &mut Windows,
        controls: &mut Controls,
        events: &mut Events,
        address: u32,
        release_storage: bool,
    ) -> Result<(), String> {
        if self
            .draw_items
            .iter()
            .any(|(dialog, _, _)| *dialog == address)
        {
            return Err("DisposeDialog before its drawing callbacks completed".into());
        }
        let d = self.records.remove(&address).ok_or("unknown dialog")?;
        for item in d.items {
            if item.handle == 0 {
                continue;
            }
            if [4, 5, 6].contains(&(item.kind & 0x7f)) {
                controls.dispose(m, heap, item.handle)?;
            } else if item.kind & 0x7f != 0 {
                heap.dispose(m, item.handle).map_err(|e| e.to_string())?;
            }
        }
        heap.dispose(m, d.list).map_err(|e| e.to_string())?;
        windows.visible(m, heap, q, fonts, events, address, false)?;
        windows.dispose(m, heap, q, events, address)?;
        if let Some(table) = d.color_table {
            heap.dispose(m, table).map_err(|e| e.to_string())?;
        }
        if release_storage {
            heap.dispose_ptr(address).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    /// System-owned Pascal procedure entry returned by GetStdFilterProc.
    pub const STANDARD_FILTER: u32 = 0x18400;
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch_filter<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        controls: &Controls,
    ) -> Result<bool, String> {
        if op != 0xaa68 {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        if cpu.pc == Self::STANDARD_FILTER {
            let item_pointer = m.read_long(sp + 4);
            let event = m.read_long(sp + 8);
            let dialog = m.read_long(sp + 12);
            let d = self
                .records
                .get(&dialog)
                .ok_or("standard filter unknown dialog")?;
            let what = m.read_word(event);
            let key = m.read_long(event + 2) as u8;

            let item = if what == 3 {
                if key == 13 || key == 3 {
                    Some(m.read_word(dialog + 168) as usize)
                } else {
                    None
                }
            } else {
                None
            };
            let handled = if let Some(number) = item {
                if let Some(item) = number.checked_sub(1).and_then(|i| d.items.get(i)) {
                    let control = m.read_long(item.handle);
                    if item.kind & 0x7f == 4
                        && item.kind & 0x80 == 0
                        && m.read_byte(control + 17) != 255
                    {
                        m.write_word(item_pointer, number as u16);
                        true
                    } else {
                        false
                    }
                } else {
                    false
                }
            } else {
                false
            };
            m.write_byte(sp + 16, handled as u8);
            let target = m.read_long(sp);
            cpu.dar[15] += 16;
            cpu.pc = target;
            return Ok(true);
        }
        match cpu.dar[0] as u16 {
            0x0203 => {
                let output = m.read_long(sp);
                m.write_word(Self::STANDARD_FILTER, 0xaa68);
                m.write_long(output, Self::STANDARD_FILTER);
                m.write_word(sp + 4, 0);
                cpu.dar[15] += 4;
            }
            selector @ (0x0304 | 0x0305) => {
                let item = m.read_word(sp) as usize;
                let dialog = m.read_long(sp + 2);
                let d = self
                    .records
                    .get_mut(&dialog)
                    .ok_or("dialog default/cancel unknown dialog")?;
                let target = item
                    .checked_sub(1)
                    .and_then(|i| d.items.get(i))
                    .ok_or("dialog default/cancel outside item list")?;
                if target.kind & 0x7f != 4 {
                    return Err("dialog default/cancel non-button is not implemented".into());
                }
                if selector == 0x0304 {
                    m.write_word(dialog + 168, item as u16);
                    self.draw(m, heap, q, fonts, controls, dialog)?;
                } else {
                    d.cancel_item = item;
                }
                m.write_word(sp + 6, 0);
                cpu.dar[15] += 6;
            }
            _ => return Ok(false),
        }
        cpu.pc += 2;
        Ok(true)
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<bool, String> {
        if op == 0xa98b {
            let sp = cpu.dar[15];
            let mut parameters: [Option<Vec<u8>>; 4] = Default::default();
            for i in 0..4 {
                let p = m.read_long(sp + (3 - i) * 4);
                if p != 0 {
                    let n = m.read_byte(p) as u32;
                    parameters[i as usize] = Some((0..n).map(|j| m.read_byte(p + 1 + j)).collect());
                }
            }
            Self::parameter_text(m, heap, parameters)?;
            cpu.dar[15] += 16;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xa98e {
            let sp = cpu.dar[15];
            let rect_pointer = m.read_long(sp);
            let r = read_rect(m, rect_pointer);
            let handle = m.read_long(sp + 4);
            let kind = m.read_word(sp + 8) as u8;
            let index = m.read_word(sp + 10) as usize;
            let address = m.read_long(sp + 12);
            let d = self
                .records
                .get_mut(&address)
                .ok_or("SetDialogItem unknown dialog")?;
            let index = index.checked_sub(1).ok_or("dialog item zero")?;
            if index >= d.items.len() {
                return Err("SetDialogItem outside item list".into());
            }
            let data = m.read_long(d.list);
            let size = heap.size(d.list).map_err(|e| e.to_string())?;
            let mut offset = 2;
            for _ in 0..index {
                if offset + 14 > size {
                    return Err("truncated dialog item list".into());
                }
                let n = m.read_byte(data + offset + 13) as u32;
                offset += 14 + n + (n & 1);
            }
            if offset + 14 > size {
                return Err("truncated dialog item list".into());
            }
            m.write_long(data + offset, handle);
            write_rect(m, data + offset + 4, r);
            m.write_byte(data + offset + 12, kind);
            d.items[index] = Item {
                kind,
                handle,
                rect: r,
            };
            cpu.dar[15] += 16;
            cpu.pc += 2;
            return Ok(true);
        }
        if op != 0xa98d {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let rect = m.read_long(sp);
        let handle = m.read_long(sp + 4);
        let kind = m.read_long(sp + 8);
        let index = m.read_word(sp + 12) as usize;
        let address = m.read_long(sp + 14);
        let d = self
            .records
            .get(&address)
            .ok_or("GetDialogItem unknown dialog")?;
        let item = d
            .items
            .get(index.checked_sub(1).ok_or("dialog item zero")?)
            .ok_or("GetDialogItem outside item list")?;
        m.write_word(kind, item.kind as u16);
        m.write_long(handle, item.handle);
        write_rect(m, rect, item.rect);
        cpu.dar[15] += 18;
        cpu.pc += 2;
        Ok(true)
    }
}
