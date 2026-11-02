//! QuickDraw regions: canonical horizontal bands and native XOR edge encoding.
use crate::{
    managers::{Heap, Rect},
    quickdraw::read_rect,
};
use m68k::AddressBus;
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq)]
pub struct Band {
    pub top: i16,
    pub bottom: i16,
    pub spans: Vec<(i16, i16)>,
}
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    pub bands: Vec<Band>,
}
/// Offset a region in its memory representation. Classic rectangular region
/// headers retain the translated bounding box even when their area is empty.
pub fn offset_handle<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    h: u32,
    dh: i16,
    dv: i16,
) -> Result<(), String> {
    let p = heap.data(h).map_err(|e| e.to_string())?;
    if heap.size(h).map_err(|e| e.to_string())? < 10 {
        return Err("short region handle".into());
    }
    if m.read_word(p) == 10 {
        let r = read_rect(m, p + 2);
        let r = Rect {
            top: r.top.checked_add(dv).ok_or("region vertical overflow")?,
            bottom: r.bottom.checked_add(dv).ok_or("region vertical overflow")?,
            left: r.left.checked_add(dh).ok_or("region horizontal overflow")?,
            right: r
                .right
                .checked_add(dh)
                .ok_or("region horizontal overflow")?,
        };
        crate::quickdraw::write_rect(m, p + 2, r);
        Ok(())
    } else {
        Region::read(m, h)?.offset(dh, dv)?.store(m, heap, h)
    }
}
#[derive(Clone, Copy)]
pub enum Operation {
    Intersection,
    Union,
    Difference,
    Xor,
}
impl Region {
    pub fn rectangle(r: Rect) -> Self {
        if r.top >= r.bottom || r.left >= r.right {
            return Self::default();
        }
        Self {
            bands: vec![Band {
                top: r.top,
                bottom: r.bottom,
                spans: vec![(r.left, r.right)],
            }],
        }
    }
    pub fn bounds(&self) -> Rect {
        if self.bands.is_empty() {
            return Rect {
                top: 0,
                left: 0,
                bottom: 0,
                right: 0,
            };
        }
        Rect {
            top: self.bands[0].top,
            bottom: self.bands.last().unwrap().bottom,
            left: self
                .bands
                .iter()
                .flat_map(|b| &b.spans)
                .map(|s| s.0)
                .min()
                .unwrap(),
            right: self
                .bands
                .iter()
                .flat_map(|b| &b.spans)
                .map(|s| s.1)
                .max()
                .unwrap(),
        }
    }
    pub fn contains(&self, v: i16, h: i16) -> bool {
        self.bands
            .iter()
            .any(|b| v >= b.top && v < b.bottom && b.spans.iter().any(|s| h >= s.0 && h < s.1))
    }
    fn append(&mut self, top: i16, bottom: i16, spans: Vec<(i16, i16)>) {
        if spans.is_empty() || top >= bottom {
            return;
        }
        if let Some(last) = self.bands.last_mut() {
            if last.bottom == top && last.spans == spans {
                last.bottom = bottom;
                return;
            }
        }
        self.bands.push(Band { top, bottom, spans });
    }
    fn spans(&self, y: i16) -> &[(i16, i16)] {
        self.bands
            .iter()
            .find(|b| y >= b.top && y < b.bottom)
            .map(|b| b.spans.as_slice())
            .unwrap_or(&[])
    }
    pub fn combine(&self, other: &Self, operation: Operation) -> Self {
        let ys: BTreeSet<i16> = self
            .bands
            .iter()
            .chain(&other.bands)
            .flat_map(|b| [b.top, b.bottom])
            .collect();
        let ys: Vec<i16> = ys.into_iter().collect();
        let mut out = Self::default();
        for y in ys.windows(2) {
            let a = self.spans(y[0]);
            let b = other.spans(y[0]);
            let xs: BTreeSet<i16> = a.iter().chain(b).flat_map(|s| [s.0, s.1]).collect();
            let xs: Vec<i16> = xs.into_iter().collect();
            let mut spans: Vec<(i16, i16)> = Vec::new();
            for x in xs.windows(2) {
                let ina = a.iter().any(|s| x[0] >= s.0 && x[0] < s.1);
                let inb = b.iter().any(|s| x[0] >= s.0 && x[0] < s.1);
                let include = match operation {
                    Operation::Intersection => ina && inb,
                    Operation::Union => ina || inb,
                    Operation::Difference => ina && !inb,
                    Operation::Xor => ina ^ inb,
                };
                if include {
                    if let Some(last) = spans.last_mut() {
                        if last.1 == x[0] {
                            last.1 = x[1];
                            continue;
                        }
                    }
                    spans.push((x[0], x[1]));
                }
            }
            out.append(y[0], y[1], spans);
        }
        out
    }
    pub fn offset(&self, dh: i16, dv: i16) -> Result<Self, String> {
        let mut result = self.clone();
        for b in &mut result.bands {
            b.top = b.top.checked_add(dv).ok_or("region vertical overflow")?;
            b.bottom = b.bottom.checked_add(dv).ok_or("region vertical overflow")?;
            for s in &mut b.spans {
                s.0 = s.0.checked_add(dh).ok_or("region horizontal overflow")?;
                s.1 = s.1.checked_add(dh).ok_or("region horizontal overflow")?;
            }
        }
        Ok(result)
    }
    pub fn words(&self) -> Result<Vec<u16>, String> {
        let b = self.bounds();
        let mut words = vec![
            10,
            b.top as u16,
            b.left as u16,
            b.bottom as u16,
            b.right as u16,
        ];
        if self.bands.is_empty() || (self.bands.len() == 1 && self.bands[0].spans.len() == 1) {
            return Ok(words);
        }
        let mut changes: BTreeMap<i16, BTreeSet<i16>> = BTreeMap::new();
        for band in &self.bands {
            for y in [band.top, band.bottom] {
                for span in &band.spans {
                    for x in [span.0, span.1] {
                        let set = changes.entry(y).or_default();
                        if !set.insert(x) {
                            set.remove(&x);
                        }
                    }
                }
            }
        }
        for (y, edges) in changes {
            if edges.is_empty() {
                continue;
            }
            if y == 32767 || edges.contains(&32767) {
                return Err("complex region reaches sentinel coordinate".into());
            }
            words.push(y as u16);
            words.extend(edges.into_iter().map(|x| x as u16));
            words.push(0x7fff);
        }
        words.push(0x7fff);
        if words.len() * 2 > 32766 {
            return Err("region exceeds classic size limit".into());
        }
        words[0] = (words.len() * 2) as u16;
        Ok(words)
    }
    pub fn read<B: AddressBus>(m: &mut B, handle: u32) -> Result<Self, String> {
        if handle == 0 {
            return Err("nil region handle".into());
        }
        let p = m.read_long(handle);
        let size = m.read_word(p) as u32;
        if size < 10 || size & 1 != 0 || size > 32766 {
            return Err("invalid region size".into());
        }
        if size == 10 {
            return Ok(Self::rectangle(read_rect(m, p + 2)));
        }
        let mut pos = 10;
        let mut edges: BTreeSet<i16> = BTreeSet::new();
        let mut last_y = None;
        let mut out = Self::default();
        let mut terminated = false;
        while pos + 2 <= size {
            let y = m.read_word(p + pos) as i16;
            pos += 2;
            if y == 32767 {
                terminated = true;
                break;
            }
            if let Some(previous) = last_y {
                if y <= previous {
                    return Err("region rows out of order".into());
                }
                if edges.len() & 1 != 0 {
                    return Err("unbalanced region edges".into());
                }
                let xs: Vec<i16> = edges.iter().copied().collect();
                out.append(
                    previous,
                    y,
                    xs.chunks_exact(2).map(|x| (x[0], x[1])).collect(),
                );
            }
            let mut end = false;
            let mut last_x = None;
            while pos + 2 <= size {
                let x = m.read_word(p + pos) as i16;
                pos += 2;
                if x == 32767 {
                    end = true;
                    break;
                }
                if last_x.is_some_and(|old| x <= old) {
                    return Err("region edges out of order".into());
                }
                last_x = Some(x);
                if !edges.insert(x) {
                    edges.remove(&x);
                }
            }
            if !end {
                return Err("unterminated region row".into());
            }
            last_y = Some(y);
        }
        if !terminated || !edges.is_empty() || pos != size {
            return Err("unterminated complex region".into());
        }
        Ok(out)
    }
    pub fn store<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        handle: u32,
    ) -> Result<(), String> {
        let words = self.words()?;
        heap.resize(m, handle, (words.len() * 2) as u32)
            .map_err(|e| e.to_string())?;
        let p = heap.data(handle).map_err(|e| e.to_string())?;
        for (i, word) in words.iter().enumerate() {
            m.write_word(p + 2 * i as u32, *word);
        }
        Ok(())
    }
    pub fn new_handle<B: AddressBus>(m: &mut B, heap: &mut Heap) -> Result<u32, String> {
        let h = heap.new_handle(m, 10, true).map_err(|e| e.to_string())?;
        Self::default().store(m, heap, h)?;
        Ok(h)
    }
}
/// Region membership for drawing, without allocating or expanding scanlines.
pub fn contains<B: AddressBus>(m: &mut B, h: u32, y: i16, x: i16) -> Result<bool, String> {
    if h == 0 {
        return Err("nil clipping region".into());
    }
    let p = m.read_long(h);
    let size = m.read_word(p) as u32;
    if size < 10 || size & 1 != 0 || size > 32766 {
        return Err("invalid clipping region size".into());
    }
    if !read_rect(m, p + 2).contains(y, x) {
        return Ok(false);
    }
    if size == 10 {
        return Ok(true);
    }
    let mut pos = 10;
    let mut inside = false;
    while pos + 2 <= size {
        let row = m.read_word(p + pos) as i16;
        pos += 2;
        if row == 32767 || row > y {
            return Ok(inside);
        }
        let mut end = false;
        while pos + 2 <= size {
            let edge = m.read_word(p + pos) as i16;
            pos += 2;
            if edge == 32767 {
                end = true;
                break;
            }
            if edge <= x {
                inside = !inside;
            }
        }
        if !end {
            return Err("unterminated clipping row".into());
        }
    }
    Err("unterminated clipping region".into())
}

pub fn dispatch<B: AddressBus>(
    opcode: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    heap: &mut Heap,
    q: &crate::quickdraw::QuickDraw,
) -> Result<bool, String> {
    let sp = cpu.dar[15];
    let pop;
    match opcode {
        0xa8d8 => {
            let h = Region::new_handle(m, heap)?;
            m.write_long(sp, h);
            pop = 0;
        }
        0xa8d9 => {
            let h = m.read_long(sp);
            heap.dispose(m, h).map_err(|e| e.to_string())?;
            pop = 4;
        }
        0xa8dd => {
            let h = m.read_long(sp);
            Region::default().store(m, heap, h)?;
            pop = 4;
        }
        0xa8dc => {
            let dest = m.read_long(sp);
            let source = m.read_long(sp + 4);
            Region::read(m, source)?.store(m, heap, dest)?;
            pop = 8;
        }
        0xa8de => {
            let bottom = m.read_word(sp) as i16;
            let right = m.read_word(sp + 2) as i16;
            let top = m.read_word(sp + 4) as i16;
            let left = m.read_word(sp + 6) as i16;
            let h = m.read_long(sp + 8);
            Region::rectangle(Rect {
                top,
                left,
                bottom,
                right,
            })
            .store(m, heap, h)?;
            pop = 12;
        }
        0xa8df => {
            let rect = m.read_long(sp);
            let h = m.read_long(sp + 4);
            Region::rectangle(read_rect(m, rect)).store(m, heap, h)?;
            pop = 8;
        }
        0xa8e0 => {
            let dv = m.read_word(sp) as i16;
            let dh = m.read_word(sp + 2) as i16;
            let h = m.read_long(sp + 4);
            Region::read(m, h)?.offset(dh, dv)?.store(m, heap, h)?;
            pop = 8;
        }
        0xa8e1 => {
            let dv = m.read_word(sp) as i16;
            let dh = m.read_word(sp + 2) as i16;
            let h = m.read_long(sp + 4);
            let r = Region::read(m, h)?;
            if r.bands.len() > 1 || r.bands.first().is_some_and(|b| b.spans.len() > 1) {
                return Err("complex InsetRgn is not implemented".into());
            }
            if !r.bands.is_empty() {
                let b = r.bounds();
                Region::rectangle(Rect {
                    top: b.top.checked_add(dv).ok_or("region inset overflow")?,
                    left: b.left.checked_add(dh).ok_or("region inset overflow")?,
                    bottom: b.bottom.checked_sub(dv).ok_or("region inset overflow")?,
                    right: b.right.checked_sub(dh).ok_or("region inset overflow")?,
                })
                .store(m, heap, h)?;
            }
            pop = 8;
        }
        0xa8e2 => {
            let h = m.read_long(sp);
            let empty = Region::read(m, h)?.bands.is_empty();
            m.write_byte(sp + 4, empty as u8);
            pop = 4;
        }
        0xa8e3 => {
            let b = m.read_long(sp);
            let a = m.read_long(sp + 4);
            let equal = Region::read(m, a)? == Region::read(m, b)?;
            m.write_byte(sp + 8, equal as u8);
            pop = 8;
        }
        0xa8e4..=0xa8e7 => {
            let dest = m.read_long(sp);
            let b = m.read_long(sp + 4);
            let a = m.read_long(sp + 8);
            let a = Region::read(m, a)?;
            let b = Region::read(m, b)?;
            let operation = match opcode {
                0xa8e4 => Operation::Intersection,
                0xa8e5 => Operation::Union,
                0xa8e6 => Operation::Difference,
                0xa8e7 => Operation::Xor,
                _ => unreachable!(),
            };
            a.combine(&b, operation).store(m, heap, dest)?;
            pop = 12;
        }
        0xa8e8 => {
            let h = m.read_long(sp);
            let v = m.read_word(sp + 4) as i16;
            let x = m.read_word(sp + 6) as i16;
            let found = contains(m, h, v, x)?;
            m.write_byte(sp + 8, found as u8);
            pop = 8;
        }
        0xa8e9 => {
            let h = m.read_long(sp);
            let rect = m.read_long(sp + 4);
            let rect = Region::rectangle(read_rect(m, rect));
            let r = Region::read(m, h)?;
            m.write_byte(
                sp + 8,
                (!r.combine(&rect, Operation::Intersection).bands.is_empty()) as u8,
            );
            pop = 8;
        }
        0xa879 | 0xa87a | 0xa87b => {
            let port = q.get_port(m)?;
            if port == 0 {
                return Err("clipping operation without current port".into());
            }
            let clip = m.read_long(port + 28);
            let arg = m.read_long(sp);
            match opcode {
                0xa879 => Region::read(m, arg)?.store(m, heap, clip)?,
                0xa87a => Region::read(m, clip)?.store(m, heap, arg)?,
                0xa87b => Region::rectangle(read_rect(m, arg)).store(m, heap, clip)?,
                _ => unreachable!(),
            };
            pop = 4;
        }
        _ => return Ok(false),
    }
    cpu.dar[15] += pop;
    cpu.pc += 2;
    Ok(true)
}
