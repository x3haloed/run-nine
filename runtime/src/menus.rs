//! MenuInfo-backed menu state and application-local menu bars.
use crate::{
    fonts::Fonts,
    managers::{Heap, Rect},
    quickdraw::QuickDraw,
};
use m68k::AddressBus;
#[derive(Clone)]
pub struct Item {
    pub text: Vec<u8>,
    pub key: u8,
    // Retained for menu item accessors and dropdown rendering.
    #[allow(dead_code)]
    pub mark: u8,
    pub style: u8,
    #[allow(dead_code)]
    pub icon: u8,
}
pub struct Menus {
    pub installed: Vec<u32>,
    hidden: Vec<u32>,
    bar: Option<u32>,
    pub highlighted: i16,
    tracking: Option<Tracking>,
}
struct Tracking {
    background: Vec<u8>,
    handle: u32,
    left: i16,
    selected: usize,
    last_down: bool,
    drawn: bool,
}
fn pstring<B: AddressBus>(m: &mut B, p: u32) -> Vec<u8> {
    let n = m.read_byte(p) as u32;
    (0..n).map(|i| m.read_byte(p + 1 + i)).collect()
}
impl Menus {
    pub fn new() -> Self {
        Self {
            installed: Vec::new(),
            hidden: Vec::new(),
            bar: None,
            highlighted: 0,
            tracking: None,
        }
    }
    pub fn items<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<Vec<Item>, String> {
        let size = heap.size(h).map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        if size < 16 {
            return Err("short MenuInfo".into());
        }
        let mut offset = 15 + m.read_byte(p + 14) as u32;
        let mut items = Vec::new();
        loop {
            if offset >= size {
                return Err("unterminated MenuInfo".into());
            }
            let len = m.read_byte(p + offset) as u32;
            if len == 0 {
                break;
            }
            if offset + 1 + len + 4 > size {
                return Err("truncated menu item".into());
            }
            let text = pstring(m, p + offset);
            offset += 1 + len;
            items.push(Item {
                text,
                icon: m.read_byte(p + offset),
                key: m.read_byte(p + offset + 1),
                mark: m.read_byte(p + offset + 2),
                style: m.read_byte(p + offset + 3),
            });
            offset += 4;
        }
        Ok(items)
    }
    fn metadata_offset<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
        index: usize,
    ) -> Result<Option<u32>, String> {
        let items = self.items(m, heap, h)?;
        if index == 0 || index > items.len() {
            return Ok(None);
        }
        let p = m.read_long(h);
        let offset = 15
            + m.read_byte(p + 14) as u32
            + items
                .iter()
                .take(index - 1)
                .map(|i| 5 + i.text.len() as u32)
                .sum::<u32>()
            + 1
            + items[index - 1].text.len() as u32;
        Ok(Some(offset))
    }
    pub fn title<B: AddressBus>(&self, m: &mut B, h: u32) -> Vec<u8> {
        let p = m.read_long(h);
        pstring(m, p + 14)
    }
    pub fn set_item_text<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        _fonts: &Fonts,
        h: u32,
        index: usize,
        text: &[u8],
    ) -> Result<(), String> {
        let mut items = self.items(m, heap, h)?;
        if index == 0 || index > items.len() {
            return Ok(());
        }
        if text.is_empty() || text.len() > 255 {
            return Err("unsupported empty or overlong menu item label".into());
        }
        items[index - 1].text = text.to_vec();
        let p = m.read_long(h);
        let header_len = 15 + m.read_byte(p + 14) as u32;
        let mut bytes: Vec<u8> = (0..header_len).map(|i| m.read_byte(p + i)).collect();
        for item in items {
            bytes.push(item.text.len() as u8);
            bytes.extend(item.text);
            bytes.extend([item.icon, item.key, item.mark, item.style]);
        }
        bytes.push(0);
        heap.resize(m, h, bytes.len() as u32)
            .map_err(|e| format!("SetItem: {e}"))?;
        let p = m.read_long(h);
        for (i, b) in bytes.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        m.write_word(p + 2, 0xffff);
        m.write_word(p + 4, 0xffff);
        Ok(())
    }
    pub fn calculate<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
    ) -> Result<(), String> {
        let items = self.items(m, heap, h)?;
        let width = items
            .iter()
            .map(|i| fonts.text_width(0, 0, i.style, &i.text))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .max()
            .unwrap_or(0)
            + 32;
        let height: usize = items
            .iter()
            .map(|i| if i.text == b"-" { 6 } else { 18 })
            .sum();
        let p = m.read_long(h);
        m.write_word(p + 2, width as u16);
        m.write_word(p + 4, height as u16);
        Ok(())
    }
    pub fn append_labels<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        names: &[&[u8]],
    ) -> Result<(), String> {
        if names.is_empty() {
            return Ok(());
        }
        let items = self.items(m, heap, h)?;
        let old = m.read_long(h);
        let end = 15
            + m.read_byte(old + 14) as usize
            + items.iter().map(|i| 1 + i.text.len() + 4).sum::<usize>();
        let extra: usize = names.iter().map(|n| 1 + n.len().min(255) + 4).sum();
        heap.resize(m, h, (end + extra + 1) as u32)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        let mut offset = end as u32;
        let mut flags = m.read_long(p + 10);
        for (index, name) in names.iter().enumerate() {
            let n = name.len().min(255);
            m.write_byte(p + offset, n as u8);
            offset += 1;
            for b in name.iter().take(n) {
                m.write_byte(p + offset, *b);
                offset += 1;
            }
            for _ in 0..4 {
                m.write_byte(p + offset, 0);
                offset += 1;
            }
            let item = items.len() + index + 1;
            if item < 32 {
                flags |= 1 << item;
            }
        }
        m.write_byte(p + offset, 0);
        m.write_long(p + 10, flags);
        self.calculate(m, heap, fonts, h)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<B: AddressBus, F>(
        &mut self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        resources: &[crate::resource::Resource<'_>],
        mut load: F,
    ) -> Result<bool, String>
    where
        F: FnMut(&crate::resource::Resource<'_>, &mut B, &mut Heap) -> Result<u32, String>,
    {
        let sp = cpu.dar[15];
        match op {
            0xa935 => {
                let before = m.read_word(sp) as i16;
                let h = m.read_long(sp + 2);
                self.insert(m, heap, h, before)?;
                cpu.dar[15] += 6;
            }
            0xa936 => {
                let id = m.read_word(sp) as i16;
                self.remove(m, heap, id)?;
                cpu.dar[15] += 2;
            }
            0xa939 | 0xa93a => {
                let item = m.read_word(sp);
                let h = m.read_long(sp + 2);
                self.enable(m, h, item, op == 0xa939)?;
                cpu.dar[15] += 6;
            }
            0xa93e => {
                let key = m.read_word(sp) as u8;
                let selection = self.key(m, heap, key)?;
                m.write_long(sp + 2, selection);
                cpu.dar[15] += 2;
            }
            0xa84e | 0xa943 | 0xa93f | 0xa941 => {
                let output = m.read_long(sp);
                let index = m.read_word(sp + 4) as usize;
                let h = m.read_long(sp + 6);
                let field = match op {
                    0xa93f => 0,
                    0xa84e => 1,
                    0xa943 => 2,
                    _ => 3,
                };
                let value = if let Some(offset) = self.metadata_offset(m, heap, h, index)? {
                    let p = m.read_long(h);
                    m.read_byte(p + offset + field) as u16
                } else {
                    0
                };
                m.write_word(output, value);
                cpu.dar[15] += 10;
            }
            0xa84f | 0xa944 | 0xa940 | 0xa942 | 0xa945 => {
                let index = m.read_word(sp + 2) as usize;
                let h = m.read_long(sp + 4);
                let value = match op {
                    0xa940 => m.read_byte(sp),
                    0xa945 => {
                        if m.read_byte(sp) != 0 {
                            18
                        } else {
                            0
                        }
                    }
                    _ => m.read_word(sp) as u8,
                };
                let field = match op {
                    0xa940 => 0,
                    0xa84f => 1,
                    0xa944 | 0xa945 => 2,
                    _ => 3,
                };
                if let Some(offset) = self.metadata_offset(m, heap, h, index)? {
                    let p = m.read_long(h);
                    m.write_byte(p + offset + field, value);
                    m.write_word(p + 2, 0xffff);
                    m.write_word(p + 4, 0xffff);
                }
                cpu.dar[15] += 8;
            }
            0xa947 => {
                let text = m.read_long(sp);
                let index = m.read_word(sp + 4) as usize;
                let h = m.read_long(sp + 6);
                let bytes = pstring(m, text);
                self.set_item_text(m, heap, fonts, h, index, &bytes)?;
                cpu.dar[15] += 10;
            }
            0xa946 => {
                let output = m.read_long(sp);
                let index = m.read_word(sp + 4) as usize;
                let h = m.read_long(sp + 6);
                let items = self.items(m, heap, h)?;
                let text = index
                    .checked_sub(1)
                    .and_then(|i| items.get(i))
                    .map(|i| i.text.as_slice())
                    .unwrap_or(&[]);
                m.write_byte(output, text.len() as u8);
                for (i, b) in text.iter().enumerate() {
                    m.write_byte(output + 1 + i as u32, *b);
                }
                cpu.dar[15] += 10;
            }
            0xa950 => {
                let h = m.read_long(sp);
                let count = self.items(m, heap, h)?.len();
                m.write_word(sp + 4, count as u16);
                cpu.dar[15] += 4;
            }
            0xa94d => {
                let kind = m.read_long(sp).to_be_bytes();
                let h = m.read_long(sp + 4);
                let names = if kind == *b"FONT" || kind == *b"FOND" {
                    fonts.family_names().to_vec()
                } else {
                    resources
                        .iter()
                        .filter(|r| r.kind == kind)
                        .filter_map(|r| r.name)
                        .collect()
                };
                self.append_labels(m, heap, fonts, h, &names)?;
                cpu.dar[15] += 8;
            }
            0xa9c0 => {
                let id = m.read_word(sp) as i16;
                let h = self.resource_bar(m, heap, fonts, resources, id, load)?;
                m.write_word(0xa60, 0);
                m.write_long(sp + 2, h);
                cpu.dar[15] += 2;
            }
            0xa93b => {
                let h = self.get_bar(m, heap)?;
                m.write_long(sp, h);
            }
            0xa93c => {
                let h = m.read_long(sp);
                self.set_bar(m, heap, h)?;
                cpu.dar[15] += 4;
            }
            0xa949 => {
                let id = m.read_word(sp) as i16;
                let h = self.handle(m, id);
                m.write_long(sp + 2, h);
                cpu.dar[15] += 2;
            }
            0xa9bf => {
                let id = m.read_word(sp) as i16;
                let h = if let Some(r) = resources.iter().find(|r| r.kind == *b"MENU" && r.id == id)
                {
                    load(r, m, heap)?
                } else {
                    0
                };
                if h != 0 {
                    self.calculate(m, heap, fonts, h)?;
                }
                m.write_long(sp + 2, h);
                cpu.dar[15] += 2;
            }
            _ => return Ok(false),
        }
        cpu.pc += 2;
        Ok(true)
    }
    fn snapshot_bytes(items: &[u32], hidden: &[u32]) -> Vec<u8> {
        let mut bytes = b"MBAR".to_vec();
        bytes.extend_from_slice(&(items.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&(hidden.len() as u16).to_be_bytes());
        for h in items.iter().chain(hidden) {
            bytes.extend_from_slice(&h.to_be_bytes());
        }
        bytes
    }
    fn snapshot<B: AddressBus>(
        m: &mut B,
        heap: &mut Heap,
        items: &[u32],
        hidden: &[u32],
    ) -> Result<u32, String> {
        let bytes = Self::snapshot_bytes(items, hidden);
        let h = heap
            .new_handle(m, bytes.len() as u32, true)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(h);
        for (i, b) in bytes.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        Ok(h)
    }
    pub fn get_bar<B: AddressBus>(&self, m: &mut B, heap: &mut Heap) -> Result<u32, String> {
        Self::snapshot(m, heap, &self.installed, &self.hidden)
    }
    pub fn set_bar<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<(), String> {
        let size = heap.size(h).map_err(|e| e.to_string())?;
        let p = heap.data(h).map_err(|e| e.to_string())?;
        if size < 8 || m.read_long(p) != u32::from_be_bytes(*b"MBAR") {
            return Err("SetMenuBar requires an owned menu-list snapshot".into());
        }
        let count = m.read_word(p + 4) as u32;
        let hidden_count = m.read_word(p + 6) as u32;
        if size != 8 + (count + hidden_count) * 4 {
            return Err("invalid owned menu-list snapshot length".into());
        }
        let mut items = Vec::new();
        for i in 0..count + hidden_count {
            let h = m.read_long(p + 8 + i * 4);
            heap.data(h).map_err(|e| e.to_string())?;
            if items.contains(&h) {
                return Err("duplicate owned menu-list entries".into());
            }
            items.push(h);
        }
        self.hidden = items.split_off(count as usize);
        self.installed = items;
        self.sync(m, heap)
    }
    pub fn handle<B: AddressBus>(&self, m: &mut B, id: i16) -> u32 {
        self.installed
            .iter()
            .chain(&self.hidden)
            .copied()
            .find(|h| {
                let p = m.read_long(*h);
                m.read_word(p) as i16 == id
            })
            .unwrap_or(0)
    }
    pub fn resource_bar<B: AddressBus, F>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        resources: &[crate::resource::Resource<'_>],
        id: i16,
        mut load: F,
    ) -> Result<u32, String>
    where
        F: FnMut(&crate::resource::Resource<'_>, &mut B, &mut Heap) -> Result<u32, String>,
    {
        let Some(resource) = resources.iter().find(|r| r.kind == *b"MBAR" && r.id == id) else {
            return Ok(0);
        };
        let decoded =
            crate::vendor::compressed::decompress_if_needed(resource.attributes, resource.data)
                .map_err(|e| format!("MBAR decompression: {e:?}"))?;
        let bytes = decoded.as_deref().unwrap_or(resource.data);
        if bytes.len() < 2 {
            return Err("truncated MBAR resource".into());
        }
        let count = u16::from_be_bytes(bytes[..2].try_into().unwrap()) as usize;
        if 2 + count * 2 > bytes.len() {
            return Err("truncated MBAR menu IDs".into());
        }
        let mut items = Vec::new();
        for i in 0..count {
            let menu_id = i16::from_be_bytes(bytes[2 + i * 2..4 + i * 2].try_into().unwrap());
            if let Some(resource) = resources
                .iter()
                .find(|r| r.kind == *b"MENU" && r.id == menu_id)
            {
                let h = load(resource, m, heap)?;
                self.calculate(m, heap, fonts, h)?;
                if items.contains(&h) {
                    return Err("duplicate MBAR menu entries are not implemented".into());
                }
                items.push(h);
            }
        }
        Self::snapshot(m, heap, &items, &[])
    }
    fn sync<B: AddressBus>(&mut self, m: &mut B, heap: &mut Heap) -> Result<(), String> {
        // MenuBarHandle is opaque; copy both visible and hierarchical lists.
        let bytes = Self::snapshot_bytes(&self.installed, &self.hidden);
        let h = if let Some(h) = self.bar {
            heap.resize(m, h, bytes.len() as u32)
                .map_err(|e| e.to_string())?;
            h
        } else {
            let h = heap
                .new_handle(m, bytes.len() as u32, true)
                .map_err(|e| e.to_string())?;
            self.bar = Some(h);
            h
        };
        let p = m.read_long(h);
        for (i, b) in bytes.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        m.write_long(0xa1c, h);
        Ok(())
    }
    pub fn init<B: AddressBus>(&mut self, m: &mut B, heap: &mut Heap) -> Result<(), String> {
        self.installed.clear();
        self.hidden.clear();
        self.highlighted = 0;
        self.tracking = None;
        m.write_word(0xa26, 0);
        self.sync(m, heap)
    }
    pub fn insert<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
        before: i16,
    ) -> Result<(), String> {
        if self.installed.contains(&h) || self.hidden.contains(&h) {
            return Ok(());
        }
        if before == -1 {
            self.hidden.push(h);
            return self.sync(m, heap);
        }
        let index = if before == 0 {
            self.installed.len()
        } else {
            self.installed
                .iter()
                .position(|v| {
                    let p = m.read_long(*v);
                    m.read_word(p) as i16 == before
                })
                .unwrap_or(self.installed.len())
        };
        self.installed.insert(index, h);
        self.sync(m, heap)
    }
    pub fn remove<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        id: i16,
    ) -> Result<(), String> {
        for list in [&mut self.installed, &mut self.hidden] {
            list.retain(|h| {
                let p = m.read_long(*h);
                m.read_word(p) as i16 != id
            });
        }
        self.sync(m, heap)
    }
    pub fn enable<B: AddressBus>(
        &self,
        m: &mut B,
        h: u32,
        item: u16,
        enabled: bool,
    ) -> Result<(), String> {
        if item > 31 {
            return Err("extended menu enable flags not implemented".into());
        }
        let p = m.read_long(h);
        let flags = m.read_long(p + 10);
        m.write_long(
            p + 10,
            if enabled {
                flags | (1u32 << item)
            } else {
                flags & !(1u32 << item)
            },
        );
        Ok(())
    }
    pub fn key<B: AddressBus>(&self, m: &mut B, heap: &mut Heap, key: u8) -> Result<u32, String> {
        for h in self.installed.iter().chain(&self.hidden) {
            let p = m.read_long(*h);
            let flags = m.read_long(p + 10);
            if flags & 1 == 0 {
                continue;
            }
            let id = m.read_word(p);
            for (i, item) in self.items(m, heap, *h)?.iter().enumerate() {
                if i < 31
                    && flags & (1u32 << (i + 1)) != 0
                    && item.key != 0
                    && item.key.eq_ignore_ascii_case(&key)
                {
                    return Ok(((id as u32) << 16) | (i + 1) as u32);
                }
            }
        }
        Ok(0)
    }
    fn at_title<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        v: i16,
        h: i16,
    ) -> Result<Option<(u32, i16)>, String> {
        if !(0..20).contains(&v) {
            return Ok(None);
        }
        let mut x = 12;
        for &handle in &self.installed {
            let title = self.title(m, handle);
            let width = fonts.text_width(0, 12, 0, &title)?;
            if h >= x - 6 && h < x + width + 10 {
                let p = m.read_long(handle);
                return Ok(if m.read_long(p + 10) & 1 != 0 {
                    Some((handle, x - 6))
                } else {
                    None
                });
            }
            x += width + 16;
        }
        Ok(None)
    }
    fn restore<B: AddressBus>(m: &mut B, q: &QuickDraw, background: &[u8]) {
        for (i, &b) in background.iter().enumerate() {
            m.write_byte(q.screen_base + i as u32, b);
        }
    }
    fn dropdown<B: AddressBus>(
        &self,
        m: &mut B,
        q: &QuickDraw,
        fonts: &Fonts,
        manager: u32,
        t: &Tracking,
        items: &[Item],
    ) -> Result<(), String> {
        let old = q.get_port(m)?;
        q.set_port(m, manager)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(manager + i)).collect();
        let result = (|| {
            q.pen_size(m, 1, 1)?;
            q.set_word(m, 56, 8)?;
            q.set_word(m, 68, 0)?;
            q.set_word(m, 72, 1)?;
            q.set_word(m, 74, 12)?;
            m.write_long(manager + 80, 33);
            m.write_long(manager + 84, 30);
            let p = m.read_long(t.handle);
            let right = (t.left + m.read_word(p + 2) as i16).min(q.width - 1);
            let bottom = (22 + m.read_word(p + 4) as i16).min(q.height - 1);
            q.paint_rect(
                m,
                Rect {
                    top: 20,
                    left: t.left,
                    bottom,
                    right,
                },
                [0; 8],
                8,
            )?;
            q.frame_rect(
                m,
                Rect {
                    top: 20,
                    left: t.left,
                    bottom,
                    right,
                },
            )?;
            let flags = m.read_long(p + 10);
            let mut y = 21;
            for (i, item) in items.iter().enumerate() {
                let height = if item.text == b"-" { 6 } else { 18 };
                if item.text == b"-" {
                    q.paint_rect(
                        m,
                        Rect {
                            top: y + 2,
                            left: t.left + 3,
                            bottom: y + 3,
                            right: right - 3,
                        },
                        [0xaa; 8],
                        8,
                    )?;
                } else {
                    m.write_byte(manager + 70, item.style);
                    q.move_to(m, t.left + 18, y + 13)?;
                    q.text(m, fonts, &item.text)?;
                    m.write_byte(manager + 70, 0);
                    if item.key != 0 {
                        q.move_to(m, right - 14, y + 13)?;
                        q.text(m, fonts, &[item.key.to_ascii_uppercase()])?;
                    }
                    if item.mark != 0 {
                        q.move_to(m, t.left + 3, y + 13)?;
                        q.text(m, fonts, &[item.mark])?;
                    }
                    if i < 31 && flags & (1 << (i + 1)) == 0 {
                        q.paint_rect(
                            m,
                            Rect {
                                top: y,
                                left: t.left + 1,
                                bottom: y + height,
                                right: right - 1,
                            },
                            [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55],
                            11,
                        )?;
                    }
                    if t.selected == i + 1 {
                        q.paint_rect(
                            m,
                            Rect {
                                top: y,
                                left: t.left + 1,
                                bottom: y + height,
                                right: right - 1,
                            },
                            [255; 8],
                            10,
                        )?;
                    }
                }
                y += height;
            }
            Ok(())
        })();
        for (i, &b) in saved.iter().enumerate() {
            m.write_byte(manager + i as u32, b);
        }
        q.set_port(m, old)?;
        result
    }
    /// Suspend the Toolbox call while the host updates pointer/button state.
    pub fn track<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        manager: u32,
        start: (i16, i16),
        events: &crate::events::Events,
        cancel: bool,
    ) -> Result<Option<u32>, String> {
        for &h in &self.installed {
            let p = m.read_long(h);
            if (m.read_word(p + 2) as i16) < 0 || (m.read_word(p + 4) as i16) < 0 {
                self.calculate(m, heap, fonts, h)?;
            }
        }
        let mut t = if let Some(t) = self.tracking.take() {
            t
        } else {
            let Some((handle, left)) = self.at_title(m, fonts, start.0, start.1)? else {
                return Ok(Some(0));
            };
            Tracking {
                background: (0..q.row_bytes() as u32 * q.height as u32)
                    .map(|i| m.read_byte(q.screen_base + i))
                    .collect(),
                handle,
                left,
                selected: 0,
                last_down: true,
                drawn: false,
            }
        };
        if let Some((handle, left)) = self.at_title(m, fonts, events.mouse.0, events.mouse.1)? {
            if t.handle != handle {
                t.handle = handle;
                t.left = left;
                t.drawn = false;
            }
        }
        let items = self.items(m, heap, t.handle)?;
        if items.iter().any(|i| i.icon != 0 || i.key == 27) {
            return Err("menu icons and hierarchical menu tracking are not implemented".into());
        }
        let p = m.read_long(t.handle);
        if m.read_long(p + 6) != 0 {
            return Err("custom MDEF menu tracking is not implemented".into());
        }
        let flags = m.read_long(p + 10);
        let id = m.read_word(p) as i16;
        let mut selected = 0;
        let mut y = 21;
        for (i, item) in items.iter().enumerate() {
            let height = if item.text == b"-" { 6 } else { 18 };
            if i < 31
                && flags & (1 << (i + 1)) != 0
                && item.text != b"-"
                && events.mouse.0 >= y
                && events.mouse.0 < y + height
                && events.mouse.1 > t.left
                && events.mouse.1 < t.left + m.read_word(p + 2) as i16
            {
                selected = i + 1;
            }
            y += height;
        }
        let down = events.modifiers & 0x80 == 0;
        let released = t.last_down && !down;
        let finish = if cancel {
            Some(0)
        } else if released {
            if selected != 0 {
                Some(((id as u16 as u32) << 16) | selected as u32)
            } else if (0..20).contains(&events.mouse.0) {
                None
            } else {
                Some(0)
            }
        } else {
            None
        };
        if let Some(result) = finish {
            Self::restore(m, q, &t.background);
            self.highlighted = if result != 0 { id } else { 0 };
            m.write_word(0xa26, self.highlighted as u16);
            self.draw(m, q, fonts, manager)?;
            return Ok(Some(result));
        }
        if !t.drawn || t.selected != selected {
            t.selected = selected;
            Self::restore(m, q, &t.background);
            self.highlighted = id;
            m.write_word(0xa26, id as u16);
            self.draw(m, q, fonts, manager)?;
            self.dropdown(m, q, fonts, manager, &t, &items)?;
            t.drawn = true;
        }
        t.last_down = down;
        self.tracking = Some(t);
        Ok(None)
    }
    pub fn draw<B: AddressBus>(
        &self,
        m: &mut B,
        q: &QuickDraw,
        fonts: &Fonts,
        manager_port: u32,
    ) -> Result<(), String> {
        let old = q.get_port(m)?;
        q.set_port(m, manager_port)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(manager_port + i)).collect();
        q.paint_rect(
            m,
            Rect {
                top: 0,
                left: 0,
                bottom: 20,
                right: q.width,
            },
            [0; 8],
            8,
        )?;
        q.paint_rect(
            m,
            Rect {
                top: 19,
                left: 0,
                bottom: 20,
                right: q.width,
            },
            [255; 8],
            8,
        )?;
        q.set_word(m, 68, 0)?;
        q.set_word(m, 70, 0)?;
        q.set_word(m, 72, 1)?;
        q.set_word(m, 74, 12)?;
        let mut x = 12;
        for h in &self.installed {
            let title = self.title(m, *h);
            let width = fonts.text_width(0, 12, 0, &title)?;
            if title == [0x14] {
                q.frame_rect(
                    m,
                    Rect {
                        top: 5,
                        left: x,
                        bottom: 14,
                        right: x + 8,
                    },
                )?;
            } else {
                q.move_to(m, x, 15)?;
                q.text(m, fonts, &title)?;
            }
            let p = m.read_long(*h);
            if m.read_word(p) as i16 == self.highlighted {
                q.paint_rect(
                    m,
                    Rect {
                        top: 0,
                        left: x - 6,
                        bottom: 19,
                        right: x + width + 6,
                    },
                    [255; 8],
                    10,
                )?;
            }
            x += width + 16;
        }
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(manager_port + i as u32, *b);
        }
        q.set_port(m, old)
    }
}
