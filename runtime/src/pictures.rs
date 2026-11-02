//! PICT bitmap playback through shared decoding and the current QuickDraw port.
use crate::{machine::Memory, managers::Rect, quickdraw::QuickDraw, regions::Region};
use m68k::AddressBus;
fn size(r: Rect) -> (i32, i32) {
    (
        i32::from(r.right) - i32::from(r.left),
        i32::from(r.bottom) - i32::from(r.top),
    )
}
fn map_rect(r: Rect, from: Rect, to: Rect) -> Result<Rect, String> {
    let (fw, fh) = size(from);
    let (tw, th) = size(to);
    let map =
        |value: i16, origin: i16, dest: i16, scale: i32, denominator: i32| -> Result<i16, String> {
            let v = i64::from(dest)
                + (i64::from(value) - i64::from(origin)) * i64::from(scale)
                    / i64::from(denominator);
            i16::try_from(v)
                .map_err(|_| "PICT mapped coordinate outside QuickDraw word range".into())
        };
    Ok(Rect {
        top: map(r.top, from.top, to.top, th, fh)?,
        bottom: map(r.bottom, from.top, to.top, th, fh)?,
        left: map(r.left, from.left, to.left, tw, fw)?,
        right: map(r.right, from.left, to.left, tw, fw)?,
    })
}
pub fn render(
    m: &mut Memory,
    q: &QuickDraw,
    bytes: &[u8],
    destination: Rect,
) -> Result<(), String> {
    let picture = classic_core::pict::decode(bytes)?;
    let (width, height) = size(destination);
    if width <= 0 || height <= 0 {
        return Ok(());
    }
    let port = q.get_port(m)?;
    let bitmap = q.port_bitmap(m, port)?;
    let target = q.port_surface(m, port)?;
    let palette = if target.depth == 8 {
        q.palette(m, bitmap)?
    } else {
        vec![[65535; 3], [0; 3]]
    };

    if target.depth != 8 {
        return Err("PICT bitmap playback requires the measured eight-bit destination".into());
    }
    if m.read_long(port + 104) != 0 {
        return Err("custom grafProcs in PICT playback outside profile".into());
    }
    let vis = m.read_long(port + 24);
    let clip = m.read_long(port + 28);
    let visible = Region::read(m, vis)?;
    let clipped = Region::read(m, clip)?;
    let mut plan = Vec::new();
    for image in picture.bitmaps {
        let dest = map_rect(image.destination, picture.frame, destination)?;
        let clip = map_rect(image.clip, picture.frame, destination)?;
        let (dw, dh) = size(dest);
        let (sw, sh) = size(image.source);
        if dw <= 0 || dh <= 0 || sw <= 0 || sh <= 0 {
            continue;
        }
        let Some(area) = dest
            .intersection(clip)
            .and_then(|r| r.intersection(target.bounds))
            .and_then(|r| r.intersection(visible.bounds()))
            .and_then(|r| r.intersection(clipped.bounds()))
        else {
            continue;
        };
        let mapping = image
            .palette
            .iter()
            .map(|rgb| {
                let color = *rgb;
                classic_core::pixels::nearest(color, &palette)
                    .ok_or("empty PICT destination color table")
            })
            .collect::<Result<Vec<_>, _>>()?;
        for y in area.top..area.bottom {
            for x in area.left..area.right {
                if !visible.contains(y, x) || !clipped.contains(y, x) {
                    continue;
                }
                let ox = i32::from(x) - i32::from(dest.left);
                let oy = i32::from(y) - i32::from(dest.top);
                let span = |origin: i16, offset: i32, source: i32, dest: i32| {
                    let source = i64::from(source);
                    let dest = i64::from(dest);
                    let offset = i64::from(offset);
                    let (first, last) = if source > dest {
                        (
                            (offset * source + dest / 2) / dest,
                            ((offset + 1) * source + dest / 2) / dest,
                        )
                    } else {
                        let first = offset * source / dest;
                        (first, first + 1)
                    };
                    (
                        i32::from(origin) + first as i32,
                        i32::from(origin) + last as i32,
                    )
                };
                let (mut left, right) = span(image.source.left, ox, sw, dw);
                let (mut top, bottom) = span(image.source.top, oy, sh, dh);
                // A clipped native span starts from the truncated source edge;
                // interior reduction boxes use the rounded boundaries above.
                if x == area.left && sw > dw {
                    left = i32::from(image.source.left)
                        + (i64::from(ox) * i64::from(sw) / i64::from(dw)) as i32;
                }
                if y == area.top && sh > dh {
                    top = i32::from(image.source.top)
                        + (i64::from(oy) * i64::from(sh) / i64::from(dh)) as i32;
                }
                // Native shrinking retains the maximum source index in each
                // covered source box, before mapping through its color table.
                let mut sample = None::<u8>;
                for sy in
                    top.max(i32::from(image.bounds.top))..bottom.min(i32::from(image.bounds.bottom))
                {
                    for sx in left.max(i32::from(image.bounds.left))
                        ..right.min(i32::from(image.bounds.right))
                    {
                        let col = (sx - i32::from(image.bounds.left)) as usize;
                        let row = (sy - i32::from(image.bounds.top)) as usize;
                        let pixel = if image.depth == 8 {
                            image.pixels[row * image.row_bytes + col]
                        } else {
                            u8::from(
                                image.pixels[row * image.row_bytes + col / 8] & (0x80 >> (col & 7))
                                    != 0,
                            )
                        };
                        sample = Some(sample.map_or(pixel, |previous| previous.max(pixel)));
                    }
                }
                let Some(index) = sample else {
                    continue;
                };
                let value = *mapping
                    .get(usize::from(index))
                    .ok_or("PICT source index outside color table")?;
                plan.push((x, y, value));
                if plan.len() > 2_000_000 {
                    return Err("PICT playback exceeds bounded destination surface".into());
                }
            }
        }
    }
    for (x, y, value) in plan {
        target.write(m, x, y, value, 0);
    }
    Ok(())
}

pub fn dispatch(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut Memory,
    heap: &mut crate::managers::Heap,
    q: &QuickDraw,
) -> Result<bool, String> {
    if op != 0xa8f6 {
        return Ok(false);
    }
    let sp = cpu.dar[15];
    let rect = m.read_long(sp);
    let handle = m.read_long(sp + 4);
    let data = heap.data(handle).map_err(|e| e.to_string())?;
    let length = heap.size(handle).map_err(|e| e.to_string())?;
    let destination = crate::quickdraw::read_rect(m, rect);
    let (w, h) = size(destination);
    if w <= 0 || h <= 0 {
        cpu.set_ccr(0);
    } else {
        let bytes = (0..length)
            .map(|i| m.read_byte(data + i))
            .collect::<Vec<_>>();
        render(m, q, &bytes, destination)?;
        m.write_word(0x220, 0);
        cpu.dar[0] = 0;
        cpu.set_ccr(4);
    }
    cpu.dar[15] += 8;
    cpu.pc += 2;
    Ok(true)
}
