//! Plain and styled TextEdit records and document editing in application memory.
use crate::{
    fonts::Fonts,
    managers::{Heap, Rect},
    quickdraw::{read_rect, write_rect, QuickDraw},
};
use m68k::AddressBus;
use std::collections::BTreeSet;

/// Shared classic word wrapping, including CRs and trailing break whitespace.
fn line_starts(
    text: &[u8],
    width: i32,
    cr_only: bool,
    mut measure: impl FnMut(usize, u8) -> Result<i32, String>,
) -> Result<Vec<usize>, String> {
    let mut starts = vec![0usize];
    let mut first = 0;
    while first < text.len() {
        let mut i = first;
        let mut advance = 0;
        let mut space = None;
        while i < text.len() {
            if text[i] == 13 {
                i += 1;
                break;
            }
            let next = advance + measure(i, text[i])?;
            if !cr_only && next > width && i > first {
                if text[i] == b' ' {
                    while i < text.len() && text[i] == b' ' {
                        i += 1;
                    }
                } else if let Some(end) = space {
                    i = end;
                }
                break;
            }
            advance = next;
            i += 1;
            if text[i - 1] == b' ' {
                space = Some(i);
            }
        }
        // The last start is the end-of-text sentinel, included in TERec.
        starts.push(i);
        first = i;
    }
    Ok(starts)
}

pub struct TextEdit {
    records: BTreeSet<u32>,
    styled: BTreeSet<u32>,
    auto_view: BTreeSet<u32>,
    outline: BTreeSet<u32>,
    click: Option<(u32, i32, i32, i32)>,
    click_aborted: bool,
}
impl TextEdit {
    fn text_box<B: AddressBus>(
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        text: &[u8],
        rect: Rect,
        justification: i16,
    ) -> Result<(), String> {
        if !matches!(justification, -1 | 0 | 1) {
            return Err(format!(
                "unsupported text box justification {justification}"
            ));
        }
        let width = i32::from(rect.right) - i32::from(rect.left);
        if width <= 0 {
            return Err("text box has no positive width".into());
        }
        let port = q.get_port(m)?;
        if port == 0 {
            return Err("text box without current port".into());
        }
        if m.read_long(port + 76) != 0 {
            return Err("text box with SpaceExtra".into());
        }
        let font = m.read_word(port + 68) as i16;
        let size = m.read_word(port + 74) as i16;
        let face = m.read_byte(port + 70);
        let metrics = fonts.select(font, size, face)?;
        let ascent = metrics.info[0];
        let height = metrics.info[0] + metrics.info[1] + metrics.info[3];
        if height <= 0 {
            return Err("text box font has no line height".into());
        }
        let x_for = |line_width: i16| -> i16 {
            match justification {
                0 => rect.left.wrapping_add(1),
                -1 => rect.right.wrapping_sub(line_width),
                _ => {
                    (i32::from(rect.left) + (width - i32::from(line_width) + 1).div_euclid(2))
                        as i16
                }
            }
        };
        // TextBox erases the box, but its fitting single-line path draws under
        // the caller's original clip, even when the box has no vertical area.
        let background = q.pattern(m, port, 32)?;
        q.paint_rect(m, rect, background, 8)?;
        let full = fonts.text_width(font, size, face, text)?;
        if !text.contains(&13) && i32::from(full) <= width {
            q.move_to(m, x_for(full), rect.top.wrapping_add(ascent))?;
            return q.text(m, fonts, text);
        }
        let starts = line_starts(text, width, false, |_, byte| {
            Ok(i32::from(metrics.widths[byte as usize]))
        })?;
        let clip = m.read_long(port + 28);
        let original = crate::regions::Region::read(m, clip)?;
        original
            .combine(
                &crate::regions::Region::rectangle(rect),
                crate::regions::Operation::Intersection,
            )
            .store(m, heap, clip)?;
        let draw = (|| {
            let mut top = rect.top;
            for line in 0..starts.len() {
                let first = starts[line];
                let end = starts.get(line + 1).copied().unwrap_or(first);
                let bytes = &text[first..end];
                let mut align_end = bytes.len();
                if justification == -1 {
                    while align_end > 0 && matches!(bytes[align_end - 1], b' ' | 13) {
                        align_end -= 1;
                    }
                }
                let width = fonts.text_width(font, size, face, &bytes[..align_end])?;
                // The next baseline is positioned before testing remaining
                // text or vertical room. This pen movement is observable.
                q.move_to(m, x_for(width), top.wrapping_add(ascent))?;
                if first == text.len() || top >= rect.bottom {
                    break;
                }
                q.text(m, fonts, bytes)?;
                top = top.wrapping_add(height);
            }
            Ok(())
        })();
        let restore = original.store(m, heap, clip);
        draw.and(restore)
    }
    pub fn documents<B: AddressBus>(&self, m: &mut B) -> Result<serde_json::Value, String> {
        let mut result = Vec::new();
        for &h in &self.records {
            let p = self.record(m, h)?;
            let text = self.text(m, h)?;
            let font = if self.is_styled(h) {
                let c = crate::styledtext::Context::read(m, p)?;
                let offset = m.read_word(p + 32) as usize;
                let style = c.caret(m, offset, text.len())?;
                serde_json::json!([style.font, style.face, style.size])
            } else {
                serde_json::json!([
                    m.read_word(p + 74),
                    m.read_byte(p + 76),
                    m.read_word(p + 80)
                ])
            };
            let mut value = serde_json::json!({"handle":h,"port":m.read_long(p+82),"selection":[m.read_word(p+32),m.read_word(p+34)],"text_bytes":text,"font":font,"word_break":m.read_long(p+38),"click_loop":m.read_long(p+42)});
            let dest = read_rect(m, p);
            let view = read_rect(m, p + 8);
            value["dest_rect"] = serde_json::json!([dest.top, dest.left, dest.bottom, dest.right]);
            value["view_rect"] = serde_json::json!([view.top, view.left, view.bottom, view.right]);
            if self.is_styled(h) {
                value["styled"] = serde_json::json!(true);
                let context = crate::styledtext::Context::read(m, p)?;
                value["style_runs"] = serde_json::Value::Array(context.runs.iter().map(|(offset,index)| {
                    let s = &context.styles[*index];
                    serde_json::json!({"offset":offset,"font":s.font,"face":s.face,"size":s.size,"color":s.color,"height":s.height,"ascent":s.ascent})
                }).collect());
            }
            result.push(value);
        }
        Ok(serde_json::json!({"schema":1,"documents":result}))
    }
    pub fn new() -> Self {
        Self {
            records: BTreeSet::new(),
            styled: BTreeSet::new(),
            auto_view: BTreeSet::new(),
            outline: BTreeSet::new(),
            click: None,
            click_aborted: false,
        }
    }
    pub fn click_result(&mut self, result: i16) -> Result<(), String> {
        if self.click.is_none() {
            return Err("TextEdit click-loop return without tracking".into());
        }
        self.click_aborted = result == 0;
        Ok(())
    }
    // Classic Roman word boundaries: single internal apostrophes, hyphens and
    // periods join letters/numbers; other punctuation selects independently.
    fn word_range(text: &[u8], offset: i32) -> (i32, i32) {
        if text.is_empty() {
            return (0, 0);
        }
        let at = (offset.max(0) as usize).min(text.len() - 1);
        let letter = |b: u8| b.is_ascii_alphanumeric() || b >= 0x80;
        let word = |i: usize| {
            letter(text[i])
                || (matches!(text[i], b'-' | b'\'' | b'.')
                    && i > 0
                    && i + 1 < text.len()
                    && letter(text[i - 1])
                    && letter(text[i + 1]))
        };
        let mut start = at;
        let mut end = at + 1;
        if word(at) {
            while start > 0 && word(start - 1) {
                start -= 1;
            }
            while end < text.len() && word(end) {
                end += 1;
            }
        } else if text[at] == b' ' {
            while start > 0 && text[start - 1] == b' ' {
                start -= 1;
            }
            while end < text.len() && text[end] == b' ' {
                end += 1;
            }
        }
        (start as i32, end as i32)
    }
    fn record<B: AddressBus>(&self, m: &mut B, h: u32) -> Result<u32, String> {
        if !self.records.contains(&h) {
            return Err(format!("unknown TEHandle {h:#x}"));
        }
        Ok(m.read_long(h))
    }
    pub fn create<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        dest: Rect,
        view: Rect,
    ) -> Result<u32, String> {
        let port = q.get_port(m)?;
        if port == 0 {
            return Err("TENew without a current port".into());
        }
        let font = m.read_word(port + 68) as i16;
        let face = m.read_byte(port + 70);
        let size = m.read_word(port + 74) as i16;
        let info = fonts.select(font, size, face)?.info;
        let h = heap
            .new_handle(m, 98, true)
            .map_err(|e| format!("TENew record: {e}"))?;
        let text = heap
            .new_handle(m, 0, true)
            .map_err(|e| format!("TENew text: {e}"))?;
        let p = heap.data(h).map_err(|e| e.to_string())?;
        write_rect(m, p, dest);
        write_rect(m, p + 8, view);
        m.write_word(p + 24, (info[0] + info[1] + info[3]) as u16);
        m.write_word(p + 26, info[0] as u16);
        m.write_long(p + 62, text);
        m.write_word(p + 74, font as u16);
        m.write_byte(p + 76, face);
        let mode = m.read_word(port + 72);
        m.write_word(p + 78, mode);
        m.write_word(p + 80, if size == 0 { 12 } else { size } as u16);
        m.write_long(p + 82, port);
        self.records.insert(h);
        Ok(h)
    }
    pub fn create_styled<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        dest: Rect,
        view: Rect,
    ) -> Result<u32, String> {
        let h = self.create(m, heap, q, fonts, dest, view)?;
        let p = self.record(m, h)?;
        let port = m.read_long(p + 82);
        let style = crate::styledtext::Style::port(m, port, fonts)?;
        crate::styledtext::initialize(m, heap, p, style)?;
        self.styled.insert(h);
        Ok(h)
    }
    pub fn is_styled(&self, h: u32) -> bool {
        self.styled.contains(&h)
    }
    fn character_styles<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        h: u32,
        length: usize,
    ) -> Result<Vec<crate::styledtext::Style>, String> {
        let p = self.record(m, h)?;
        if self.is_styled(h) {
            let c = crate::styledtext::Context::read(m, p)?;
            Ok((0..length.max(1))
                .map(|i| c.styles[c.index(i)].clone())
                .collect())
        } else {
            Ok(vec![
                crate::styledtext::Style::unstyled(m, p, fonts)?;
                length.max(1)
            ])
        }
    }
    fn width(
        styles: &[crate::styledtext::Style],
        fonts: &Fonts,
        text: &[u8],
        start: usize,
        end: usize,
    ) -> Result<i16, String> {
        let mut width = 0i32;
        for i in start..end {
            width += styles[i].advance(fonts, text[i])? as i32;
        }
        Ok(width as i16)
    }
    fn line_metrics<B: AddressBus>(&self, m: &mut B, h: u32) -> Result<Vec<(i16, i16)>, String> {
        let p = self.record(m, h)?;
        let lines = m.read_word(p + 94) as usize;
        if self.is_styled(h) {
            let c = crate::styledtext::Context::read(m, p)?;
            let lp = m.read_long(c.heights);
            Ok((0..lines.max(1))
                .map(|i| {
                    (
                        m.read_word(lp + 4 * i as u32) as i16,
                        m.read_word(lp + 4 * i as u32 + 2) as i16,
                    )
                })
                .collect())
        } else {
            Ok(vec![
                (m.read_word(p + 24) as i16, m.read_word(p + 26) as i16);
                lines.max(1)
            ])
        }
    }
    pub fn scroll<B: AddressBus>(
        &self,
        m: &mut B,
        h: u32,
        dh: i16,
        dv: i16,
        pin: bool,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let dest = read_rect(m, p);
        let view = read_rect(m, p + 8);
        let (mut dx, mut dy) = (dh as i32, dv as i32);
        if pin {
            let width = dest.right as i32 - dest.left as i32;
            let text = self.text(m, h)?;
            let metrics = self.line_metrics(m, h)?;
            let count = m.read_word(p + 94) as usize;
            let mut height = metrics
                .iter()
                .take(count)
                .map(|(height, _)| *height as i32)
                .sum::<i32>();
            if text.is_empty() || text.last() == Some(&13) {
                height += metrics.last().unwrap().0 as i32;
            }
            let min_left = (view.left as i32).min(view.right as i32 - width);
            let min_top = (view.top as i32).min(view.bottom as i32 - height);
            dx = (dest.left as i32 + dx).clamp(min_left, view.left as i32) - dest.left as i32;
            dy = (dest.top as i32 + dy).clamp(min_top, view.top as i32) - dest.top as i32;
        }
        let translate = |r: Rect| -> Result<Rect, String> {
            Ok(Rect {
                top: i16::try_from(r.top as i32 + dy).map_err(|_| "TextEdit scroll overflow")?,
                bottom: i16::try_from(r.bottom as i32 + dy)
                    .map_err(|_| "TextEdit scroll overflow")?,
                left: i16::try_from(r.left as i32 + dx).map_err(|_| "TextEdit scroll overflow")?,
                right: i16::try_from(r.right as i32 + dx)
                    .map_err(|_| "TextEdit scroll overflow")?,
            })
        };
        let dest = translate(dest)?;
        let selected = translate(read_rect(m, p + 16))?;
        write_rect(m, p, dest);
        write_rect(m, p + 16, selected);
        Ok(())
    }
    pub fn selection_view<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        h: u32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let offset = m.read_word(p + 34) as i16;
        let (y, x) = self.point(m, fonts, h, offset)?;
        let view = read_rect(m, p + 8);
        let count = m.read_word(p + 94) as usize;
        let metrics = self.line_metrics(m, h)?;
        let mut line = 0;
        while line + 1 < count && m.read_word(p + 98 + line as u32 * 2) as i32 <= offset as i32 {
            line += 1;
        }
        let height = metrics[line].0 as i32;
        let top = y as i32 - height;
        let dy = if top < view.top as i32 {
            view.top as i32 - top
        } else if y > view.bottom {
            view.bottom as i32 - y as i32
        } else {
            0
        };
        let dx = if (x as i32 - 1) < view.left as i32 {
            view.left as i32 - (x as i32 - 1)
        } else if x > view.right {
            view.right as i32 - x as i32
        } else {
            0
        };
        self.scroll(
            m,
            h,
            i16::try_from(dx).map_err(|_| "TextEdit scroll overflow")?,
            i16::try_from(dy).map_err(|_| "TextEdit scroll overflow")?,
            true,
        )
    }
    pub fn height<B: AddressBus>(
        &self,
        m: &mut B,
        h: u32,
        start: i32,
        end: i32,
    ) -> Result<i32, String> {
        let metrics = self.line_metrics(m, h)?;
        // OS 9 orders the unsigned line arguments before bounding the upper
        // endpoint. Out-of-range equal endpoints therefore describe no lines.
        let (first, last) = if (start as u32) <= (end as u32) {
            (start as u32, end as u32)
        } else {
            (end as u32, start as u32)
        };
        let first = first.max(1) as usize;
        let last = (last.max(1) as usize).min(metrics.len());
        if last < first {
            return Ok(0);
        }
        Ok(metrics[first - 1..last]
            .iter()
            .map(|(height, _)| *height as i32)
            .sum())
    }
    pub fn point<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        h: u32,
        offset: i16,
    ) -> Result<(i16, i16), String> {
        let p = self.record(m, h)?;
        let dest = read_rect(m, p);
        let text = self.text(m, h)?;
        let offset = (offset as i32).clamp(0, text.len() as i32) as usize;
        let count = m.read_word(p + 94) as usize;
        let metrics = self.line_metrics(m, h)?;
        let styles = self.character_styles(m, fonts, h, text.len())?;
        let mut line = 0usize;
        while line + 1 < count && m.read_word(p + 98 + 2 * line as u32) as usize <= offset {
            line += 1;
        }
        if offset == text.len() && text.last() == Some(&13) {
            let y = dest
                .top
                .wrapping_add(
                    metrics
                        .iter()
                        .map(|(height, _)| *height as i32)
                        .sum::<i32>() as i16,
                )
                .wrapping_add(metrics.last().unwrap().0);
            return Ok((y, dest.left.wrapping_add(1)));
        }
        let start = if count == 0 {
            0
        } else {
            m.read_word(p + 96 + 2 * line as u32) as usize
        };
        let logical_end = if count == 0 {
            0
        } else {
            m.read_word(p + 98 + 2 * line as u32) as usize
        };
        let mut end = logical_end;
        if end > start && text[end - 1] == 13 {
            end -= 1;
        }
        let width = Self::width(&styles, fonts, &text, start, end)?;
        let spare = (dest.right - dest.left).wrapping_sub(width).wrapping_sub(1);
        let shift = match m.read_word(p + 58) as i16 {
            0 => 0,
            1 => spare / 2,
            -1 => spare,
            _ => return Err("unknown TextEdit justification".into()),
        };
        let x = dest
            .left
            .wrapping_add(1)
            .wrapping_add(shift)
            .wrapping_add(Self::width(&styles, fonts, &text, start, offset.min(end))?);
        let y = dest.top.wrapping_add(
            metrics[..=line]
                .iter()
                .map(|(height, _)| *height as i32)
                .sum::<i32>() as i16,
        );
        Ok((y, x))
    }
    pub fn continuous_style<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        h: u32,
        requested: u16,
    ) -> Result<(u16, crate::styledtext::Style), String> {
        if requested & !15 != 0 {
            return Err("unsupported continuous-style mode".into());
        }
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let length = m.read_word(p + 60) as usize;
        if start > end || end > length {
            return Err("invalid continuous-style selection".into());
        }
        let selected = if self.is_styled(h) {
            let c = crate::styledtext::Context::read(m, p)?;
            if start == end {
                vec![if let Some(pending) = c.pending(m)? {
                    pending
                } else {
                    c.styles[c.index(start.saturating_sub(1))].clone()
                }]
            } else {
                (start..end).map(|i| c.styles[c.index(i)].clone()).collect()
            }
        } else {
            vec![crate::styledtext::Style::unstyled(m, p, fonts)?]
        };
        let mut style = selected[0].clone();
        let mut mode = requested;
        if selected.iter().any(|s| s.font != style.font) {
            mode &= !1;
        }
        if selected.iter().any(|s| s.size != style.size) {
            mode &= !4;
        }
        if selected.iter().any(|s| s.color != style.color) {
            mode &= !8;
        }
        style.face = selected.iter().fold(0x7f, |face, s| face & s.face);
        if style.face == 0 && selected.iter().any(|s| s.face != 0) {
            mode &= !2;
        }
        Ok((mode, style))
    }
    pub fn set_style<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        mode: u16,
        new: &crate::styledtext::Style,
        recalculate: bool,
    ) -> Result<(), String> {
        if !self.is_styled(h) {
            return Err("TESetStyle on unstyled TextEdit is not implemented".into());
        }
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let length = m.read_word(p + 60) as usize;
        if start > end || end > length {
            return Err("invalid styled selection".into());
        }
        let mut c = crate::styledtext::Context::read(m, p)?;
        if start == end {
            let style = c.caret(m, start, length)?.changed(mode, new, fonts)?;
            c.set_pending(m, heap, Some(&style))?;
        } else {
            let mut indices = c.indices(length);
            for i in start..end {
                let style = c.styles[indices[i]].changed(mode, new, fonts)?;
                let index = c.intern(&indices, 0, style);
                indices[i] = index;
            }
            c.write(m, heap, &indices, 0)?;
        }
        if recalculate {
            self.calculate(m, heap, fonts, h)?;
        }
        Ok(())
    }
    pub fn text<B: AddressBus>(&self, m: &mut B, h: u32) -> Result<Vec<u8>, String> {
        let p = self.record(m, h)?;
        let len = m.read_word(p + 60) as u32;
        let t = m.read_long(p + 62);
        let t = m.read_long(t);
        Ok((0..len).map(|i| m.read_byte(t + i)).collect())
    }
    pub fn calculate<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let text_handle = m.read_long(p + 62);
        let length = heap
            .size(text_handle)
            .map_err(|e| format!("TECalText text handle: {e}"))?;
        if length > 32767 {
            return Err("TECalText text exceeds classic length limit".into());
        }
        m.write_word(p + 60, length as u16);
        let text = self.text(m, h)?;
        let rect = read_rect(m, p);
        let width = rect.right as i32 - rect.left as i32;
        if width <= 0 {
            return Err("TextEdit destination has no width".into());
        }
        let styles = self.character_styles(m, fonts, h, text.len())?;
        let cr_only = (m.read_word(p + 72) as i16) < 0;
        let starts = line_starts(&text, width, cr_only, |i, byte| {
            Ok(i32::from(styles[i].advance(fonts, byte)?))
        })?;
        heap.resize(m, h, 96 + starts.len() as u32 * 2)
            .map_err(|e| format!("TECalText record: {e}"))?;
        let p = self.record(m, h)?;
        m.write_word(p + 94, (starts.len() - 1) as u16);
        for (i, start) in starts.iter().enumerate() {
            m.write_word(p + 96 + i as u32 * 2, *start as u16);
        }
        if self.is_styled(h) {
            let c = crate::styledtext::Context::read(m, p)?;
            let count = starts.len().saturating_sub(1).max(1);
            heap.resize(m, c.heights, (count * 4) as u32)
                .map_err(|e| e.to_string())?;
            let lp = m.read_long(c.heights);
            for line in 0..count {
                let first = *starts.get(line).unwrap_or(&0);
                let last = *starts.get(line + 1).unwrap_or(&first);
                let segment = if first < last {
                    &styles[first..last]
                } else {
                    &styles[0..1]
                };
                let height = segment.iter().map(|s| s.height).max().unwrap();
                let ascent = segment.iter().map(|s| s.ascent).max().unwrap();
                m.write_word(lp + 4 * line as u32, height as u16);
                m.write_word(lp + 4 * line as u32 + 2, ascent as u16);
            }
        }
        // Native TECalText also clamps a stale selection after the application
        // directly shrinks hText (TeachText reuses its TEHandle for New).
        let start = m.read_word(p + 32) as i16 as i32;
        let end = m.read_word(p + 34) as i16 as i32;
        self.select(m, h, start, end)
    }
    fn replace<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        text: &[u8],
    ) -> Result<(), String> {
        if text.len() > 32767 {
            return Err("TextEdit text exceeds classic 32767-byte limit".into());
        }
        let p = self.record(m, h)?;
        let t = m.read_long(p + 62);
        heap.resize(m, t, text.len() as u32)
            .map_err(|e| format!("TextEdit text resize: {e}"))?;
        let data = m.read_long(t);
        for (i, b) in text.iter().enumerate() {
            m.write_byte(data + i as u32, *b);
        }
        m.write_word(p + 60, text.len() as u16);
        self.calculate(m, heap, fonts, h)
    }
    pub fn set_text<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        text: &[u8],
    ) -> Result<(), String> {
        if text.len() > 32767 {
            return Err("TextEdit text exceeds classic 32767-byte limit".into());
        }
        if self.is_styled(h) {
            let p = self.record(m, h)?;
            let port = m.read_long(p + 82);
            let mut c = crate::styledtext::Context::read(m, p)?;
            c.styles = vec![crate::styledtext::Style::port(m, port, fonts)?];
            c.write(m, heap, &vec![0; text.len()], 0)?;
        }
        self.replace(m, heap, fonts, h, text)?;
        self.select(m, h, text.len() as i32, text.len() as i32)
    }
    pub fn select<B: AddressBus>(
        &self,
        m: &mut B,
        h: u32,
        start: i32,
        end: i32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let length = m.read_word(p + 60) as i32;
        let start = start.clamp(0, length);
        let end = end.clamp(start, length);
        m.write_word(p + 32, start as u16);
        m.write_word(p + 34, end as u16);
        Ok(())
    }
    pub fn offset<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &Fonts,
        h: u32,
        v: i16,
        x: i16,
    ) -> Result<i16, String> {
        let p = self.record(m, h)?;
        let dest = read_rect(m, p);
        let count = m.read_word(p + 94) as usize;
        let metrics = self.line_metrics(m, h)?;
        if v < dest.top {
            return Ok(0);
        }
        let text = self.text(m, h)?;
        let mut line = 0usize;
        let mut top = dest.top as i32;
        while line < count {
            let height = metrics[line].0 as i32;
            if height <= 0 {
                return Err("invalid TextEdit line height".into());
            }
            if (v as i32) < top + height {
                break;
            }
            top += height;
            line += 1;
        }
        if line >= count {
            return Ok(text.len() as i16);
        }
        let start = m.read_word(p + 96 + line as u32 * 2) as usize;
        let mut end = m.read_word(p + 98 + line as u32 * 2) as usize;
        if start > end || end > text.len() {
            return Err("invalid TextEdit line starts".into());
        }
        if end > start && text[end - 1] == 13 {
            end -= 1;
        }
        let styles = self.character_styles(m, fonts, h, text.len())?;
        let width = Self::width(&styles, fonts, &text, start, end)? as i32;
        // OS 9's point sweep reserves one pixel for the insertion caret and
        // places each hit boundary one pixel past the glyph midpoint.
        let spare = dest.right as i32 - dest.left as i32 - width - 1;
        let mut at = dest.left as i32
            + match m.read_word(p + 58) as i16 {
                0 => 0,
                1 => spare / 2,
                -1 => spare,
                _ => return Err("unknown TextEdit justification".into()),
            };
        for i in start..end {
            let advance = styles[i].advance(fonts, text[i])? as i32;
            if (x as i32 - at) * 2 <= advance + 2 {
                return Ok(i as i16);
            }
            at += advance;
        }
        Ok(end as i16)
    }
    pub fn insert<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        added: &[u8],
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let mut text = self.text(m, h)?;
        if start > text.len() || end > text.len() {
            return Err("invalid TextEdit selection".into());
        }
        if text
            .len()
            .checked_add(added.len())
            .is_none_or(|n| n > 32767)
        {
            return Err("TextEdit text exceeds classic 32767-byte limit".into());
        }
        if self.is_styled(h) && !added.is_empty() {
            let mut c = crate::styledtext::Context::read(m, p)?;
            let mut indices = c.indices(text.len());
            let empty = c.index(0);
            let style = c.caret(m, start, text.len())?;
            let index = c.intern(&indices, empty, style);
            indices.splice(start..start, vec![index; added.len()]);
            c.write(m, heap, &indices, empty)?;
            c.set_pending(m, heap, None)?;
        }
        text.splice(start..start, added.iter().copied());
        self.replace(m, heap, fonts, h, &text)?;
        self.select(
            m,
            h,
            (start + added.len()) as i32,
            (end + added.len()) as i32,
        )
    }
    pub fn copy<B: AddressBus>(&self, m: &mut B, heap: &mut Heap, h: u32) -> Result<(), String> {
        if self.is_styled(h) {
            return Err("styled clipboard copy is not implemented".into());
        }
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let text = self.text(m, h)?;
        let selected = text
            .get(start..end)
            .ok_or("invalid TextEdit copy selection")?;
        let scrap = crate::scrap::init_te(m, heap)?;
        heap.resize(m, scrap, selected.len() as u32)
            .map_err(|e| e.to_string())?;
        let data = heap.data(scrap).map_err(|e| e.to_string())?;
        for (i, b) in selected.iter().enumerate() {
            m.write_byte(data + i as u32, *b);
        }
        m.write_word(0xab0, selected.len() as u16);
        Ok(())
    }
    pub fn paste<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
    ) -> Result<(), String> {
        if self.is_styled(h) {
            return Err("styled clipboard paste is not implemented".into());
        }
        let scrap = crate::scrap::init_te(m, heap)?;
        let n = m.read_word(0xab0) as u32;
        if n > heap.size(scrap).map_err(|e| e.to_string())? {
            return Err("TextEdit scrap length exceeds handle".into());
        }
        let data = heap.data(scrap).map_err(|e| e.to_string())?;
        let added: Vec<u8> = (0..n).map(|i| m.read_byte(data + i)).collect();
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let mut text = self.text(m, h)?;
        if start > end || end > text.len() {
            return Err("invalid TextEdit paste selection".into());
        }
        text.splice(start..end, added.iter().copied());
        self.replace(m, heap, fonts, h, &text)?;
        self.select(
            m,
            h,
            (start + added.len()) as i32,
            (start + added.len()) as i32,
        )
    }
    pub fn delete<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let start = m.read_word(p + 32) as usize;
        let end = m.read_word(p + 34) as usize;
        let mut text = self.text(m, h)?;
        if start > end || end > text.len() {
            return Err(format!("invalid TextEdit deletion range {start}..{end} for {} bytes (handle {h:#x}, record {p:#x})",text.len()));
        }
        let pending = if self.is_styled(h) {
            if start == end {
                return Ok(());
            }
            let c = crate::styledtext::Context::read(m, p)?;
            let style = c.caret(m, start, text.len())?;
            let mut indices = c.indices(text.len());
            indices.drain(start..end);
            c.write(m, heap, &indices, c.index(0))?;
            Some(style)
        } else {
            None
        };
        text.drain(start..end);
        self.replace(m, heap, fonts, h, &text)?;
        self.select(m, h, start as i32, start as i32)?;
        if let Some(style) = pending {
            let p = self.record(m, h)?;
            crate::styledtext::Context::read(m, p)?.set_pending(m, heap, Some(&style))?;
        }
        Ok(())
    }
    pub fn key<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        fonts: &Fonts,
        h: u32,
        key: u8,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        if ![0x1e, 0x1f].contains(&key) {
            m.write_byte(p + 71, 0xff);
        }
        let start = m.read_word(p + 32) as i32;
        let end = m.read_word(p + 34) as i32;
        match key {
            8 => {
                if start == end && start > 0 {
                    self.select(m, h, start - 1, end)?;
                }
                self.delete(m, heap, fonts, h)
            }
            0x1c => self.select(
                m,
                h,
                if start == end { start - 1 } else { start },
                if start == end { start - 1 } else { start },
            ),
            0x1d => self.select(
                m,
                h,
                if start == end { end + 1 } else { end },
                if start == end { end + 1 } else { end },
            ),
            0x1e | 0x1f => {
                let anchor = start;
                let text = self.text(m, h)?;
                let count = m.read_word(p + 94) as usize;
                let mut line = 0usize;
                while line + 1 < count && m.read_word(p + 98 + line as u32 * 2) as i32 <= anchor {
                    line += 1;
                }
                if anchor as usize == text.len() && text.last() == Some(&13) {
                    line = count;
                }
                let destination = if key == 0x1e && line == 0 {
                    0
                } else if key == 0x1f && line + 1 >= count {
                    text.len() as i32
                } else {
                    let target = if key == 0x1e { line - 1 } else { line + 1 };
                    let metrics = self.line_metrics(m, h)?;
                    let (_, x) = self.point(m, fonts, h, anchor as i16)?;
                    let rect = read_rect(m, p);
                    let y = rect
                        .top
                        .wrapping_add(
                            metrics
                                .iter()
                                .take(target)
                                .map(|(height, _)| *height as i32)
                                .sum::<i32>() as i16,
                        )
                        .wrapping_add(metrics[target].0 / 2);
                    self.offset(m, fonts, h, y, x)? as i32
                };
                if destination > 0 {
                    let at_line_start = (0..count)
                        .any(|line| m.read_word(p + 96 + line as u32 * 2) as i32 == destination);
                    m.write_word(p + 70, if at_line_start { 0xffff } else { 0 });
                }
                self.select(m, h, destination, destination)
            }
            _ => {
                self.delete(m, heap, fonts, h)?;
                self.insert(m, heap, fonts, h, &[key])
            }
        }
    }
    pub fn update<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        h: u32,
        dirty: Rect,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        let port = m.read_long(p + 82);
        let old = q.get_port(m)?;
        let saved: Vec<u8> = (0..108).map(|i| m.read_byte(port + i)).collect();
        let clip = m.read_long(port + 28);
        let original_clip = crate::regions::Region::read(m, clip)?;
        let view = crate::regions::Region::rectangle(read_rect(m, p + 8));
        let dirty = crate::regions::Region::rectangle(dirty);
        original_clip
            .combine(&view, crate::regions::Operation::Intersection)
            .combine(&dirty, crate::regions::Operation::Intersection)
            .store(m, heap, clip)?;
        q.set_port(m, port)?;
        let render = (|| {
            let dest = read_rect(m, p);
            let view = read_rect(m, p + 8);
            q.paint_rect(m, view, [0; 8], 8)?;
            let mode = m.read_word(p + 78);
            m.write_word(port + 72, mode);
            let lines = m.read_word(p + 94) as usize;
            let text = self.text(m, h)?;
            let styles = self.character_styles(m, fonts, h, text.len())?;
            let metrics = self.line_metrics(m, h)?;
            let mut line_top = dest.top;
            let active = m.read_word(p + 36) != 0;
            let outlined = !active && self.outline.contains(&h);
            let foreground = m.read_long(port + 80);
            let background = m.read_long(port + 84);
            let mut selection_region = crate::regions::Region::default();
            for line in 0..lines {
                let start = m.read_word(p + 96 + line as u32 * 2) as usize;
                let logical_end = m.read_word(p + 98 + line as u32 * 2) as usize;
                if start > logical_end || logical_end > text.len() {
                    return Err("invalid TextEdit line starts".into());
                }
                let mut end = logical_end;
                if end > start && text[end - 1] == 13 {
                    end -= 1;
                }
                let (line_height, ascent) = metrics[line];
                if line_height <= 0 {
                    return Err("invalid TextEdit line height".into());
                }
                let y = line_top.wrapping_add(ascent);
                let width = Self::width(&styles, fonts, &text, start, end)?;
                let spare = (dest.right - dest.left).wrapping_sub(width).wrapping_sub(1);
                let x = dest.left.wrapping_add(match m.read_word(p + 58) as i16 {
                    0 => 0,
                    1 => spare / 2,
                    -1 => spare,
                    _ => return Err("unknown TextEdit justification".into()),
                });
                q.move_to(m, x, y)?;
                let mut first = start;
                while first < end {
                    let mut last = first + 1;
                    while last < end && styles[last] == styles[first] {
                        last += 1;
                    }
                    m.write_long(port + 84, background);
                    if self.is_styled(h) {
                        styles[first].apply(m, port)?;
                    } else {
                        m.write_long(port + 80, foreground);
                        m.write_word(port + 68, styles[first].font as u16);
                        m.write_byte(port + 70, styles[first].face);
                        m.write_word(port + 74, styles[first].size as u16);
                    }
                    q.text(m, fonts, &text[first..last])?;
                    first = last;
                }
                if active || outlined {
                    let selection_start = m.read_word(p + 32) as usize;
                    let selection_end = m.read_word(p + 34) as usize;
                    if selection_start < selection_end
                        && selection_start < logical_end
                        && selection_end > start
                    {
                        let first = selection_start.max(start).min(end);
                        let last = selection_end.min(end);
                        let left = x
                            .wrapping_add(1)
                            .wrapping_add(Self::width(&styles, fonts, &text, start, first)?);
                        let right = if selection_end > end {
                            dest.right
                        } else {
                            x.wrapping_add(1)
                                .wrapping_add(Self::width(&styles, fonts, &text, start, last)?)
                        };
                        let rectangle = Rect {
                            top: line_top,
                            left,
                            bottom: line_top.wrapping_add(line_height),
                            right,
                        };
                        selection_region = selection_region.combine(
                            &crate::regions::Region::rectangle(rectangle),
                            crate::regions::Operation::Union,
                        );
                        if !outlined {
                            m.write_long(port + 80, 33);
                            m.write_long(port + 84, 30);
                            q.paint_rect(m, rectangle, [255; 8], 10)?;
                        }
                    } else if selection_start == selection_end
                        && (outlined || m.read_word(p + 56) != 0)
                        && selection_start >= start
                        && selection_start <= end
                        && (selection_start < logical_end || line + 1 == lines)
                    {
                        let caret = x.wrapping_add(Self::width(
                            &styles,
                            fonts,
                            &text,
                            start,
                            selection_start,
                        )?);
                        m.write_long(port + 80, 33);
                        m.write_long(port + 84, 30);
                        q.paint_rect(
                            m,
                            Rect {
                                top: line_top,
                                left: caret,
                                bottom: line_top.wrapping_add(line_height),
                                right: caret + 1,
                            },
                            if outlined {
                                [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]
                            } else {
                                [255; 8]
                            },
                            10,
                        )?;
                    }
                }
                line_top = line_top.wrapping_add(line_height);
            }
            if (active || outlined)
                && (outlined || m.read_word(p + 56) != 0)
                && (text.is_empty() || text.last() == Some(&13))
                && m.read_word(p + 32) == m.read_word(p + 34)
                && m.read_word(p + 34) as usize == text.len()
            {
                let height = metrics.last().unwrap().0;
                m.write_long(port + 80, 33);
                m.write_long(port + 84, 30);
                q.paint_rect(
                    m,
                    Rect {
                        top: line_top,
                        left: dest.left,
                        bottom: line_top.wrapping_add(height),
                        right: dest.left + 1,
                    },
                    if outlined {
                        [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]
                    } else {
                        [255; 8]
                    },
                    10,
                )?;
            }
            if !selection_region.bands.is_empty() {
                write_rect(m, p + 16, selection_region.bounds());
            }
            if outlined && !selection_region.bands.is_empty() {
                let mut interior = selection_region.clone();
                for (dh, dv) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                    interior = interior.combine(
                        &selection_region.offset(dh, dv)?,
                        crate::regions::Operation::Intersection,
                    );
                }
                let frame =
                    selection_region.combine(&interior, crate::regions::Operation::Difference);
                m.write_long(port + 80, 33);
                m.write_long(port + 84, 30);
                for band in frame.bands {
                    for (left, right) in band.spans {
                        q.paint_rect(
                            m,
                            Rect {
                                top: band.top,
                                left,
                                bottom: band.bottom,
                                right,
                            },
                            [255; 8],
                            10,
                        )?;
                    }
                }
            }
            Ok(())
        })();
        original_clip.store(m, heap, clip)?;
        for (i, b) in saved.iter().enumerate() {
            m.write_byte(port + i as u32, *b);
        }
        q.set_port(m, old)?;
        render
    }
    pub fn dispose<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<(), String> {
        let p = self.record(m, h)?;
        if self.is_styled(h) {
            crate::styledtext::dispose(m, heap, p)?;
            self.styled.remove(&h);
        }
        let t = m.read_long(p + 62);
        heap.dispose(m, t).map_err(|e| e.to_string())?;
        heap.dispose(m, h).map_err(|e| e.to_string())?;
        self.records.remove(&h);
        self.auto_view.remove(&h);
        self.outline.remove(&h);
        Ok(())
    }
    pub fn activate<B: AddressBus>(&self, m: &mut B, h: u32, active: bool) -> Result<(), String> {
        let p = self.record(m, h)?;
        m.write_word(p + 36, if active { 0xff00 } else { 0 });
        m.write_word(p + 56, if active { 0xff00 } else { 0 });
        let now = m.read_long(0x16a);
        m.write_long(p + 52, now.wrapping_add(30));
        Ok(())
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        opcode: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
        heap: &mut Heap,
        q: &QuickDraw,
        fonts: &Fonts,
        events: &crate::events::Events,
        callbacks: &mut crate::callbacks::Callbacks,
    ) -> Result<bool, String> {
        let sp = cpu.dar[15];
        let pop;
        let mut redraw = None;
        match opcode {
            0xa9ce => {
                let justification = m.read_word(sp) as i16;
                let bounds = m.read_long(sp + 2);
                let rect = read_rect(m, bounds);
                let length = m.read_long(sp + 6) as i32;
                if !(0..=32767).contains(&length) {
                    return Err("text box length outside classic text range".into());
                }
                let ptr = m.read_long(sp + 10);
                let bytes: Vec<_> = (0..length as u32).map(|i| m.read_byte(ptr + i)).collect();
                Self::text_box(m, heap, q, fonts, &bytes, rect, justification)?;
                cpu.dar[0] = 14;
                cpu.set_ccr(0);
                pop = 14;
            }
            0xa813 => {
                let h = m.read_long(sp);
                self.record(m, h)?;
                if m.read_byte(sp + 4) != 0 {
                    self.auto_view.insert(h);
                } else {
                    self.auto_view.remove(&h);
                }
                pop = 6;
            }
            0xa9dd | 0xa812 => {
                let h = m.read_long(sp);
                let dv = m.read_word(sp + 4) as i16;
                let dh = m.read_word(sp + 6) as i16;
                self.scroll(m, h, dh, dv, opcode == 0xa812)?;
                redraw = Some(h);
                pop = 8;
            }
            0xa811 => {
                let h = m.read_long(sp);
                self.selection_view(m, fonts, h)?;
                redraw = Some(h);
                pop = 4;
            }
            0xa83d => {
                let selector = m.read_word(sp);
                let sp = sp + 2;
                match selector {
                    11 => {
                        let h = m.read_long(sp);
                        let paint = m.read_byte(sp + 4) != 0;
                        let scrap = m.read_long(sp + 6);
                        let end = m.read_long(sp + 10) as i32;
                        let start = m.read_long(sp + 14) as i32;
                        let p = self.record(m, h)?;
                        if scrap != 0 && self.is_styled(h) {
                            let length = m.read_word(p + 60) as usize;
                            let start = start.clamp(0, length as i32) as usize;
                            let end = end.clamp(start as i32, length as i32) as usize;
                            let runs = crate::styledtext::read_scrap(m, heap, scrap)?;
                            if !runs.is_empty() && start < end {
                                let mut context = crate::styledtext::Context::read(m, p)?;
                                let mut indices = context.indices(length);
                                for (i, (at, style)) in runs.iter().enumerate() {
                                    let from = (start + at).min(end);
                                    let to = runs
                                        .get(i + 1)
                                        .map_or(end, |(next, _)| (start + next).min(end));
                                    if from < to {
                                        let index = context.intern(&indices, 0, style.clone());
                                        indices[from..to].fill(index);
                                    }
                                }
                                context.write(m, heap, &indices, 0)?;
                                if paint {
                                    self.calculate(m, heap, fonts, h)?;
                                    redraw = Some(h);
                                }
                            }
                        }
                        pop = 20;
                    }
                    10 => {
                        let h = m.read_long(sp);
                        let output = m.read_long(sp + 4);
                        let mode_pointer = m.read_long(sp + 8);
                        let requested = m.read_word(mode_pointer);
                        let (mode, style) = self.continuous_style(m, fonts, h, requested)?;
                        if mode & 1 != 0 {
                            m.write_word(output, style.font as u16);
                        }
                        if mode & 2 != 0 {
                            m.write_byte(output + 2, style.face);
                        }
                        if mode & 4 != 0 {
                            m.write_word(output + 4, style.size as u16);
                        }
                        if mode & 8 != 0 {
                            for (i, c) in style.color.iter().enumerate() {
                                m.write_word(output + 6 + 2 * i as u32, *c);
                            }
                        }
                        m.write_word(mode_pointer, mode);
                        m.write_byte(sp + 12, (mode == requested) as u8);
                        pop = 14;
                    }
                    14 => {
                        let h = m.read_long(sp);
                        let action = m.read_word(sp + 4) as i16;
                        let feature = m.read_word(sp + 6) as i16;
                        self.record(m, h)?;
                        if ![0, 2].contains(&feature) || ![-1, 0, 1].contains(&action) {
                            return Err(format!(
                                "unsupported TextEdit feature/action {feature}/{action}"
                            ));
                        }
                        let flags = if feature == 0 {
                            &mut self.auto_view
                        } else {
                            &mut self.outline
                        };
                        let previous = flags.contains(&h);
                        if action == 1 {
                            flags.insert(h);
                        } else if action == 0 {
                            flags.remove(&h);
                        }
                        if feature == 2 && action != -1 && previous != (action == 1) {
                            redraw = Some(h);
                        }
                        m.write_word(sp + 8, previous as u16);
                        pop = 10;
                    }
                    3 => {
                        let h = m.read_long(sp);
                        let ascent = m.read_long(sp + 4);
                        let height = m.read_long(sp + 8);
                        let output = m.read_long(sp + 12);
                        let offset = m.read_word(sp + 16) as i16;
                        let p = self.record(m, h)?;
                        let length = m.read_word(p + 60) as usize;
                        let offset =
                            (offset as i32).clamp(0, length.saturating_sub(1) as i32) as usize;
                        let styles = self.character_styles(m, fonts, h, length)?;
                        let style = &styles[offset];
                        style.write_attributes(m, output);
                        m.write_word(height, style.height as u16);
                        m.write_word(ascent, style.ascent as u16);
                        pop = 20;
                    }
                    8 => {
                        let h = m.read_long(sp);
                        let offset = m.read_word(sp + 4) as i16;
                        let (y, x) = self.point(m, fonts, h, offset)?;
                        m.write_long(sp + 6, ((y as u16 as u32) << 16) | (x as u16 as u32));
                        pop = 8;
                    }
                    9 => {
                        let h = m.read_long(sp);
                        let start = m.read_long(sp + 4) as i32;
                        let end = m.read_long(sp + 8) as i32;
                        let height = self.height(m, h, start, end)?;
                        m.write_long(sp + 12, height as u32);
                        pop = 14;
                    }
                    4 | 6 => {
                        let h = m.read_long(sp);
                        let p = self.record(m, h)?;
                        if !self.is_styled(h) {
                            return Err(
                                "style handle query on unstyled TextEdit is not implemented".into(),
                            );
                        }
                        let handle = if selector == 4 {
                            m.read_long(p + 74)
                        } else {
                            crate::styledtext::selection_scrap(m, heap, p)?
                        };
                        m.write_long(sp + 4, handle);
                        pop = 6;
                    }
                    1 => {
                        let h = m.read_long(sp);
                        let redraw = m.read_byte(sp + 4) != 0;
                        let attributes = m.read_long(sp + 6);
                        let mode = m.read_word(sp + 10);
                        let style = crate::styledtext::Style::attributes(m, attributes);
                        self.set_style(m, heap, fonts, h, mode, &style, redraw)?;
                        if redraw {
                            let p = self.record(m, h)?;
                            let dirty = read_rect(m, p + 8);
                            self.update(m, heap, q, fonts, h, dirty)?;
                        }
                        pop = 14;
                    }
                    7 => {
                        let h = m.read_long(sp);
                        let style_scrap = m.read_long(sp + 4);
                        let length = m.read_long(sp + 8);
                        let text = m.read_long(sp + 12);
                        if style_scrap != 0 {
                            return Err(
                                "TEStyleInsert with an explicit style scrap is not implemented"
                                    .into(),
                            );
                        }
                        if length > 32767 {
                            return Err("TextEdit source length exceeds classic limit".into());
                        }
                        if !self.is_styled(h) {
                            return Err(
                                "TEStyleInsert on unstyled TextEdit is not implemented".into()
                            );
                        }
                        let bytes: Vec<u8> = (0..length).map(|i| m.read_byte(text + i)).collect();
                        self.insert(m, heap, fonts, h, &bytes)?;
                        redraw = Some(h);
                        pop = 18;
                    }
                    _ => {
                        return Err(format!(
                            "unimplemented styled TextEdit selector {selector:#06x}"
                        ))
                    }
                }
            }
            0xa83c => {
                let h = m.read_long(sp);
                let v = m.read_word(sp + 4) as i16;
                let x = m.read_word(sp + 6) as i16;
                let offset = self.offset(m, fonts, h, v, x)?;
                m.write_word(sp + 8, offset as u16);
                pop = 8;
            }
            0xa9d4 => {
                let h = m.read_long(sp);
                let p = self.record(m, h)?;
                if self.click_aborted {
                    self.click_aborted = false;
                    self.click = None;
                    m.write_long(p + 46, events.ticks);
                    cpu.dar[15] += 10;
                    cpu.pc += 2;
                    return Ok(true);
                }
                let (v, x) = if self.click.is_some() {
                    let port = m.read_long(p + 82);
                    let bitmap = q.port_bitmap(m, port)?;
                    let origin = read_rect(m, bitmap + 6);
                    (
                        events.mouse.0.wrapping_add(origin.top),
                        events.mouse.1.wrapping_add(origin.left),
                    )
                } else {
                    (m.read_word(sp + 6) as i16, m.read_word(sp + 8) as i16)
                };
                let offset = self.offset(m, fonts, h, v, x)? as i32;
                let (anchor, initial_start, initial_end) =
                    if let Some((active, anchor, start, end)) = self.click {
                        if active != h {
                            return Err("reentrant TEClick".into());
                        }
                        (anchor, start, end)
                    } else {
                        let extend = m.read_byte(sp + 4) != 0;
                        let start = m.read_word(p + 32) as i32;
                        let anchor = if extend {
                            if offset <= start {
                                m.read_word(p + 34) as i32
                            } else {
                                start
                            }
                        } else {
                            offset
                        };
                        let double = m.read_word(p + 50) as i16 as i32 == offset
                            && events.ticks.wrapping_sub(m.read_long(p + 46)) <= m.read_long(0x2f0);
                        m.write_word(p + 50, offset as u16);
                        if !double {
                            let length = m.read_word(p + 60) as i32;
                            m.write_word(
                                p + 70,
                                if extend {
                                    0xff00
                                } else if offset < length {
                                    0xffff
                                } else {
                                    0
                                },
                            );
                        }
                        let (start, end) = if double {
                            if m.read_long(p + 38) != 0 {
                                return Err(
                                    "custom TextEdit word-break callback is not implemented".into(),
                                );
                            }
                            let text = self.text(m, h)?;
                            let (left, right) = Self::word_range(
                                &text,
                                if extend {
                                    offset.saturating_sub(1)
                                } else {
                                    offset
                                },
                            );
                            if extend {
                                if offset <= anchor {
                                    (left.min(anchor), left.max(anchor))
                                } else {
                                    (anchor.min(right), anchor.max(right))
                                }
                            } else {
                                (left, right)
                            }
                        } else {
                            (anchor.min(offset), anchor.max(offset))
                        };
                        (anchor, start, end)
                    };
                let (start, end) =
                    if self.click.is_some() && offset != m.read_word(p + 50) as i16 as i32 {
                        (anchor.min(offset), anchor.max(offset))
                    } else {
                        (initial_start, initial_end)
                    };
                self.select(m, h, start, end)?;
                m.write_word(p + 56, 0xff00);
                let dirty = read_rect(m, p + 8);
                self.update(m, heap, q, fonts, h, dirty)?;
                if events.modifiers & 0x80 == 0 {
                    self.click = Some((h, anchor, initial_start, initial_end));
                    let hook = m.read_long(p + 42);
                    if hook != 0 {
                        callbacks.call_register(cpu, m, hook, &[(11, p), (12, h)], 2)?;
                    }
                    return Ok(true);
                }
                self.click = None;
                m.write_long(p + 46, events.ticks);
                pop = 10;
            }
            0xa9da => {
                let h = m.read_long(sp);
                let p = self.record(m, h)?;
                let now = m.read_long(0x16a);
                let deadline = m.read_long(p + 52);
                if m.read_word(p + 36) != 0
                    && m.read_word(p + 32) == m.read_word(p + 34)
                    && now.wrapping_sub(deadline) < 0x80000000
                {
                    let visible = m.read_word(p + 56) == 0;
                    m.write_word(p + 56, if visible { 0xff00 } else { 0 });
                    m.write_long(p + 52, now.wrapping_add(30));
                    redraw = Some(h);
                }
                pop = 4;
            }
            0xa9d2 | 0xa83e => {
                let view = m.read_long(sp);
                let dest = m.read_long(sp + 4);
                let view = read_rect(m, view);
                let dest = read_rect(m, dest);
                let h = if opcode == 0xa83e {
                    self.create_styled(m, heap, q, fonts, dest, view)?
                } else {
                    self.create(m, heap, q, fonts, dest, view)?
                };
                m.write_long(sp + 8, h);
                pop = 8;
            }
            0xa9cd => {
                let h = m.read_long(sp);
                self.dispose(m, heap, h)?;
                pop = 4;
            }
            0xa9cf | 0xa9de => {
                let h = m.read_long(sp);
                let len = m.read_long(sp + 4);
                let p = m.read_long(sp + 8);
                if len > 32767 {
                    return Err("TextEdit source length exceeds classic limit".into());
                }
                let text: Vec<u8> = (0..len).map(|i| m.read_byte(p + i)).collect();
                if opcode == 0xa9cf {
                    self.set_text(m, heap, fonts, h, &text)?;
                } else {
                    self.insert(m, heap, fonts, h, &text)?;
                    redraw = Some(h);
                }
                pop = 12;
            }
            0xa9d1 => {
                let h = m.read_long(sp);
                let end = m.read_long(sp + 4) as i32;
                let start = m.read_long(sp + 8) as i32;
                self.select(m, h, start, end)?;
                if self.is_styled(h) {
                    let p = self.record(m, h)?;
                    crate::styledtext::Context::read(m, p)?.set_pending(m, heap, None)?;
                }
                pop = 12;
                redraw = Some(h);
            }
            0xa9cb => {
                let h = m.read_long(sp);
                let p = self.record(m, h)?;
                let text = m.read_long(p + 62);
                m.write_long(sp + 4, text);
                pop = 4;
            }
            0xa9df => {
                let h = m.read_long(sp);
                let just = m.read_word(sp + 4) as i16;
                if ![-1, 0, 1].contains(&just) {
                    return Err("unknown TextEdit alignment".into());
                }
                let p = self.record(m, h)?;
                m.write_word(p + 58, just as u16);
                pop = 6;
                redraw = Some(h);
            }
            0xa9d5 | 0xa9d6 | 0xa9db => {
                let h = m.read_long(sp);
                if opcode == 0xa9db {
                    self.paste(m, heap, fonts, h)?;
                } else {
                    self.copy(m, heap, h)?;
                    if opcode == 0xa9d6 {
                        self.delete(m, heap, fonts, h)?;
                    }
                }
                pop = 4;
                if opcode != 0xa9d5 {
                    redraw = Some(h);
                }
            }
            0xa9d7 => {
                let h = m.read_long(sp);
                self.delete(m, heap, fonts, h)?;
                pop = 4;
                redraw = Some(h);
            }
            0xa9dc => {
                let h = m.read_long(sp);
                let key = m.read_word(sp + 4) as u8;
                self.key(m, heap, fonts, h, key)?;
                if self.auto_view.contains(&h) {
                    self.selection_view(m, fonts, h)?;
                }
                pop = 6;
                redraw = Some(h);
            }
            0xa9d0 => {
                let h = m.read_long(sp);
                self.calculate(m, heap, fonts, h)?;
                pop = 4;
            }
            0xa9d8 | 0xa9d9 => {
                let h = m.read_long(sp);
                self.activate(m, h, opcode == 0xa9d8)?;
                pop = 4;
                redraw = Some(h);
            }
            0xa9d3 => {
                let h = m.read_long(sp);
                let dirty = m.read_long(sp + 4);
                let dirty = read_rect(m, dirty);
                self.update(m, heap, q, fonts, h, dirty)?;
                pop = 8;
            }
            _ => return Ok(false),
        }
        if let Some(h) = redraw {
            let p = self.record(m, h)?;
            if opcode != 0xa9da && opcode != 0xa9d9 && !(opcode == 0xa83d && m.read_word(sp) == 14)
            {
                m.write_word(p + 56, 0xff00);
                let now = m.read_long(0x16a);
                m.write_long(p + 52, now.wrapping_add(30));
            }
            let dirty = read_rect(m, p + 8);
            self.update(m, heap, q, fonts, h, dirty)?;
        }
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
