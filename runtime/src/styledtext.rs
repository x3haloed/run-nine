//! Styled TextEdit's app-visible run, style, line-height and insertion records.
//! Memory is authoritative; no native output files or per-application exceptions.
use crate::{fonts::Fonts, managers::Heap};
use m68k::AddressBus;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Style {
    pub font: i16,
    pub face: u8,
    pub size: i16,
    pub color: [u16; 3],
    pub height: i16,
    pub ascent: i16,
}
impl Style {
    pub fn attributes<B: AddressBus>(m: &mut B, p: u32) -> Self {
        Self {
            font: m.read_word(p) as i16,
            face: m.read_byte(p + 2),
            size: m.read_word(p + 4) as i16,
            color: [m.read_word(p + 6), m.read_word(p + 8), m.read_word(p + 10)],
            height: 0,
            ascent: 0,
        }
    }
    pub fn new(
        font: i16,
        face: u8,
        size: i16,
        color: [u16; 3],
        fonts: &Fonts,
    ) -> Result<Self, String> {
        let info = fonts.select(font, size, face)?.info;
        Ok(Self {
            font,
            face,
            size: if size == 0 { 12 } else { size },
            color,
            height: info[0] + info[1] + info[3],
            ascent: info[0],
        })
    }
    pub fn port<B: AddressBus>(m: &mut B, p: u32, fonts: &Fonts) -> Result<Self, String> {
        let color = if m.read_word(p + 6) & 0xc000 == 0xc000 {
            [
                m.read_word(p + 36),
                m.read_word(p + 38),
                m.read_word(p + 40),
            ]
        } else {
            match m.read_long(p + 80) {
                33 => [0; 3],
                30 => [65535; 3],
                _ => {
                    return Err(
                        "styled TextEdit indexed foreground color is not implemented".into(),
                    )
                }
            }
        };
        Self::new(
            m.read_word(p + 68) as i16,
            m.read_byte(p + 70),
            m.read_word(p + 74) as i16,
            color,
            fonts,
        )
    }
    pub fn unstyled<B: AddressBus>(m: &mut B, p: u32, fonts: &Fonts) -> Result<Self, String> {
        Self::new(
            m.read_word(p + 74) as i16,
            m.read_byte(p + 76),
            m.read_word(p + 80) as i16,
            [0; 3],
            fonts,
        )
    }
    fn read_st<B: AddressBus>(m: &mut B, p: u32) -> Self {
        Self {
            height: m.read_word(p + 2) as i16,
            ascent: m.read_word(p + 4) as i16,
            font: m.read_word(p + 6) as i16,
            face: m.read_byte(p + 8),
            size: m.read_word(p + 10) as i16,
            color: [
                m.read_word(p + 12),
                m.read_word(p + 14),
                m.read_word(p + 16),
            ],
        }
    }
    fn write_st<B: AddressBus>(&self, m: &mut B, p: u32, count: u16) {
        m.write_word(p, count);
        m.write_word(p + 2, self.height as u16);
        m.write_word(p + 4, self.ascent as u16);
        m.write_word(p + 6, self.font as u16);
        m.write_byte(p + 8, self.face);
        m.write_byte(p + 9, 0);
        m.write_word(p + 10, self.size as u16);
        for (i, c) in self.color.iter().enumerate() {
            m.write_word(p + 12 + 2 * i as u32, *c);
        }
    }
    pub fn write_attributes<B: AddressBus>(&self, m: &mut B, p: u32) {
        m.write_word(p, self.font as u16);
        m.write_byte(p + 2, self.face);
        m.write_byte(p + 3, 0);
        m.write_word(p + 4, self.size as u16);
        for (i, c) in self.color.iter().enumerate() {
            m.write_word(p + 6 + 2 * i as u32, *c);
        }
    }
    pub fn advance(&self, fonts: &Fonts, byte: u8) -> Result<i16, String> {
        Ok(fonts.select(self.font, self.size, self.face)?.widths[byte as usize])
    }
    pub fn apply<B: AddressBus>(&self, m: &mut B, port: u32) -> Result<(), String> {
        m.write_word(port + 68, self.font as u16);
        m.write_byte(port + 70, self.face);
        m.write_word(port + 74, self.size as u16);
        let foreground = match self.color {
            [0, 0, 0] => 33,
            [65535, 65535, 65535] => 30,
            _ => return Err(
                "nonblack/nonwhite styled glyph colors are not implemented by the one-bit renderer"
                    .into(),
            ),
        };
        m.write_long(port + 80, foreground);
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            for (i, c) in self.color.iter().enumerate() {
                m.write_word(port + 36 + 2 * i as u32, *c);
            }
        }
        Ok(())
    }
    pub fn changed(&self, mode: u16, new: &Self, fonts: &Fonts) -> Result<Self, String> {
        if mode & !15 != 0 {
            return Err(format!("unsupported TextEdit style mode {mode:#x}"));
        }
        Self::new(
            if mode & 1 != 0 { new.font } else { self.font },
            if mode & 2 != 0 {
                if new.face == 0 {
                    0
                } else {
                    self.face | new.face
                }
            } else {
                self.face
            },
            if mode & 4 != 0 { new.size } else { self.size },
            if mode & 8 != 0 { new.color } else { self.color },
            fonts,
        )
    }
}
pub struct Context {
    pub handle: u32,
    pub table: u32,
    pub heights: u32,
    pub null: u32,
    pub scrap: u32,
    pub styles: Vec<Style>,
    pub runs: Vec<(usize, usize)>,
}
impl Context {
    pub fn read<B: AddressBus>(m: &mut B, te: u32) -> Result<Self, String> {
        let h = m.read_long(te + 74);
        if h == 0 {
            return Err("nil TextEdit style handle".into());
        }
        let p = m.read_long(h);
        let n = m.read_word(p) as usize;
        let count = m.read_word(p + 2) as usize;
        if n == 0 || n > 32767 || count == 0 || count > 32767 {
            return Err("invalid TextEdit style/run count".into());
        }
        let table = m.read_long(p + 4);
        let heights = m.read_long(p + 8);
        let null = m.read_long(p + 16);
        if table == 0 || heights == 0 || null == 0 {
            return Err("nil styled TextEdit subhandle".into());
        }
        let t = m.read_long(table);
        let styles = (0..count)
            .map(|i| Style::read_st(m, t + 18 * i as u32))
            .collect();
        let mut runs = Vec::new();
        for i in 0..n {
            let at = m.read_word(p + 20 + 4 * i as u32) as usize;
            let index = m.read_word(p + 22 + 4 * i as u32) as usize;
            if index >= count
                || (i == 0 && at != 0)
                || runs.last().is_some_and(|(prev, _)| *prev >= at)
            {
                return Err("invalid styled TextEdit runs".into());
            }
            runs.push((at, index));
        }
        let np = m.read_long(null);
        let scrap = m.read_long(np + 4);
        if scrap == 0 {
            return Err("nil TextEdit null-style scrap".into());
        }
        Ok(Self {
            handle: h,
            table,
            heights,
            null,
            scrap,
            styles,
            runs,
        })
    }
    pub fn index(&self, offset: usize) -> usize {
        self.runs
            .iter()
            .rev()
            .find(|(at, _)| *at <= offset)
            .unwrap()
            .1
    }
    pub fn indices(&self, length: usize) -> Vec<usize> {
        (0..length).map(|i| self.index(i)).collect()
    }
    pub fn pending<B: AddressBus>(&self, m: &mut B) -> Result<Option<Style>, String> {
        let p = m.read_long(self.scrap);
        let count = m.read_word(p);
        if count == 0 {
            return Ok(None);
        }
        if count != 1 {
            return Err("multiple null insertion styles are not implemented".into());
        }
        Ok(Some(read_scrap_style(m, p + 2)))
    }
    pub fn caret<B: AddressBus>(
        &self,
        m: &mut B,
        offset: usize,
        length: usize,
    ) -> Result<Style, String> {
        if let Some(style) = self.pending(m)? {
            return Ok(style);
        }
        Ok(self.styles[self.index(offset.min(length.saturating_sub(1)))].clone())
    }
    pub fn intern(&mut self, indices: &[usize], empty: usize, style: Style) -> usize {
        if let Some(index) = self.styles.iter().position(|s| *s == style) {
            return index;
        }
        if let Some(index) = (0..self.styles.len())
            .find(|i| !indices.contains(i) && (!indices.is_empty() || *i != empty))
        {
            self.styles[index] = style;
            index
        } else {
            self.styles.push(style);
            self.styles.len() - 1
        }
    }
    pub fn write<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        indices: &[usize],
        empty: usize,
    ) -> Result<(), String> {
        let mut runs = Vec::new();
        for (i, index) in indices.iter().enumerate() {
            if runs.last().is_none_or(|(_, last)| last != index) {
                runs.push((i, *index));
            }
        }
        if runs.is_empty() {
            runs.push((0, empty));
        }
        let mut counts = vec![0u16; self.styles.len()];
        for (_, index) in &runs {
            counts[*index] += 1;
        }
        heap.resize(m, self.table, (18 * self.styles.len()) as u32)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(self.table);
        for (i, style) in self.styles.iter().enumerate() {
            style.write_st(m, p + 18 * i as u32, counts[i]);
        }
        heap.resize(m, self.handle, (20 + 4 * (runs.len() + 1)) as u32)
            .map_err(|e| e.to_string())?;
        let p = m.read_long(self.handle);
        m.write_word(p, runs.len() as u16);
        m.write_word(p + 2, self.styles.len() as u16);
        for (i, (at, index)) in runs.iter().enumerate() {
            m.write_word(p + 20 + 4 * i as u32, *at as u16);
            m.write_word(p + 22 + 4 * i as u32, *index as u16);
        }
        m.write_word(p + 20 + 4 * runs.len() as u32, (indices.len() + 1) as u16);
        m.write_word(p + 22 + 4 * runs.len() as u32, 0xffff);
        Ok(())
    }
    pub fn set_pending<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        style: Option<&Style>,
    ) -> Result<(), String> {
        write_scrap(
            m,
            heap,
            self.scrap,
            &style.map(|s| vec![(0, s.clone())]).unwrap_or_default(),
        )
    }
}
fn alloc<B: AddressBus>(m: &mut B, heap: &mut Heap, size: u32) -> Result<u32, String> {
    heap.new_handle(m, size, true).map_err(|e| e.to_string())
}
pub fn initialize<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    te: u32,
    style: Style,
) -> Result<(), String> {
    let handle = alloc(m, heap, 28)?;
    let table = alloc(m, heap, 18)?;
    let heights = alloc(m, heap, 4)?;
    let null = alloc(m, heap, 8)?;
    let scrap = alloc(m, heap, 2)?;
    let p = m.read_long(handle);
    m.write_long(p + 4, table);
    m.write_long(p + 8, heights);
    m.write_long(p + 16, null);
    let np = m.read_long(null);
    m.write_long(np + 4, scrap);
    let hp = m.read_long(heights);
    m.write_word(hp, style.height as u16);
    m.write_word(hp + 2, style.ascent as u16);
    let c = Context {
        handle,
        table,
        heights,
        null,
        scrap,
        styles: vec![style],
        runs: vec![(0, 0)],
    };
    c.write(m, heap, &[], 0)?;
    m.write_word(te + 24, 0xffff);
    m.write_word(te + 26, 0xffff);
    m.write_word(te + 80, 0xffff);
    m.write_long(te + 74, handle);
    Ok(())
}
pub fn read_scrap<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    h: u32,
) -> Result<Vec<(usize, Style)>, String> {
    let size = heap.size(h).map_err(|e| e.to_string())?;
    if size < 2 {
        return Err("truncated TextEdit style scrap".into());
    }
    let p = m.read_long(h);
    let count = m.read_word(p) as u32;
    if count > 32767 || 2 + 20 * count > size {
        return Err("invalid TextEdit style scrap count".into());
    }
    let mut runs = Vec::new();
    for i in 0..count {
        let p = p + 2 + 20 * i;
        let offset = m.read_long(p) as usize;
        if (i == 0 && offset != 0) || runs.last().is_some_and(|(last, _)| *last >= offset) {
            return Err("invalid TextEdit style scrap offsets".into());
        }
        runs.push((offset, read_scrap_style(m, p)));
    }
    Ok(runs)
}
fn read_scrap_style<B: AddressBus>(m: &mut B, p: u32) -> Style {
    Style {
        height: m.read_word(p + 4) as i16,
        ascent: m.read_word(p + 6) as i16,
        font: m.read_word(p + 8) as i16,
        face: m.read_byte(p + 10),
        size: m.read_word(p + 12) as i16,
        color: [
            m.read_word(p + 14),
            m.read_word(p + 16),
            m.read_word(p + 18),
        ],
    }
}
fn write_scrap<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    h: u32,
    runs: &[(usize, Style)],
) -> Result<(), String> {
    heap.resize(m, h, (2 + 20 * runs.len()) as u32)
        .map_err(|e| e.to_string())?;
    let p = m.read_long(h);
    m.write_word(p, runs.len() as u16);
    for (i, (at, s)) in runs.iter().enumerate() {
        let p = p + 2 + 20 * i as u32;
        m.write_long(p, *at as u32);
        m.write_word(p + 4, s.height as u16);
        m.write_word(p + 6, s.ascent as u16);
        s.write_attributes(m, p + 8);
    }
    Ok(())
}
pub fn selection_scrap<B: AddressBus>(m: &mut B, heap: &mut Heap, te: u32) -> Result<u32, String> {
    let c = Context::read(m, te)?;
    let start = m.read_word(te + 32) as usize;
    let requested_end = m.read_word(te + 34) as usize;
    let length = m.read_word(te + 60) as usize;
    let end = requested_end.min(length);
    if start > end {
        return Err("invalid styled selection".into());
    }
    m.write_word(te + 34, end as u16);
    let mut runs = Vec::new();
    if start == end {
        runs.push((0, c.caret(m, start, length)?));
    } else {
        runs.push((0, c.styles[c.index(start)].clone()));
        for (at, index) in &c.runs {
            if *at > start && *at < end {
                runs.push((*at - start, c.styles[*index].clone()));
            }
        }
    }
    let h = alloc(m, heap, 0)?;
    write_scrap(m, heap, h, &runs)?;
    Ok(h)
}
pub fn dispose<B: AddressBus>(m: &mut B, heap: &mut Heap, te: u32) -> Result<(), String> {
    let c = Context::read(m, te)?;
    for h in [c.table, c.heights, c.scrap, c.null, c.handle] {
        heap.dispose(m, h).map_err(|e| e.to_string())?;
    }
    Ok(())
}
