//! Bounded PICT2 bitmap decoding from Apple's public picture opcode layout.
//! Decode completely before a renderer commits any guest pixels.
use crate::Rect;
use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
#[derive(Debug)]
pub struct Picture {
    pub frame: Rect,
    pub bitmaps: Vec<Bitmap>,
}
#[derive(Debug)]
pub struct Bitmap {
    pub bounds: Rect,
    pub source: Rect,
    pub destination: Rect,
    pub clip: Rect,
    pub row_bytes: usize,
    pub depth: u8,
    pub palette: Vec<[u16; 3]>,
    pub pixels: Vec<u8>,
}
struct Reader<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Reader<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], String> {
        let end = self
            .position
            .checked_add(count)
            .ok_or("PICT offset overflow")?;
        let result = self
            .bytes
            .get(self.position..end)
            .ok_or_else(|| format!("truncated PICT at{} for{count}bytes", self.position))?;
        self.position = end;
        Ok(result)
    }
    fn byte(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u16, String> {
        Ok(u16::from_be_bytes(self.take(2)?.try_into().unwrap()))
    }
    fn rect(&mut self) -> Result<Rect, String> {
        Ok(Rect {
            top: self.word()? as i16,
            left: self.word()? as i16,
            bottom: self.word()? as i16,
            right: self.word()? as i16,
        })
    }
}
fn dimensions(rect: Rect) -> Result<(usize, usize), String> {
    let w = i32::from(rect.right) - i32::from(rect.left);
    let h = i32::from(rect.bottom) - i32::from(rect.top);
    if w <= 0 || h <= 0 {
        return Err("empty PICT bitmap/frame".into());
    }
    Ok((w as usize, h as usize))
}
fn unpack(bytes: &[u8], expected: usize) -> Result<Vec<u8>, String> {
    let mut r = Reader { bytes, position: 0 };
    let mut result = Vec::with_capacity(expected);
    while r.position < bytes.len() {
        let code = r.byte()?;
        match code {
            128 => return Err("noncanonical PICT PackBits no-op outside measured profile".into()),
            0..=127 => {
                let n = usize::from(code) + 1;
                if result.len() + n > expected {
                    return Err("PICT literal exceeds decoded row".into());
                }
                result.extend_from_slice(r.take(n)?);
            }
            _ => {
                let n = 257 - usize::from(code);
                if result.len() + n > expected {
                    return Err("PICT repeat exceeds decoded row".into());
                }
                let value = r.byte()?;
                result.resize(result.len() + n, value);
            }
        }
    }
    if result.len() != expected {
        return Err("short decoded PICT row".into());
    }
    Ok(result)
}
pub fn decode(bytes: &[u8]) -> Result<Picture, String> {
    let mut r = Reader { bytes, position: 0 };
    // picSize is only the low word for large pictures; the owning handle bounds
    // the readable data. Do not truncate a valid >64KiB image to this word.
    let _size = r.word()?;
    let frame = r.rect()?;
    dimensions(frame)?;
    if r.word()? != 0x0011 || r.word()? != 0x02ff {
        return Err("PICT version outside indexed PICT2 profile".into());
    }
    let mut clip = frame;
    let mut bitmaps = Vec::new();
    loop {
        if r.position & 1 != 0 {
            r.take(1)?;
        }
        let offset = r.position;
        let op = r.word()?;
        match op {
            0x00ff => break,
            0x0000 | 0x001e => (),
            0x0c00 => {
                r.take(24)?;
            }
            0x0001 => {
                if r.word()? != 10 {
                    return Err("nonrectangular PICT clipping region outside profile".into());
                }
                clip = r.rect()?;
            }
            0x0090 | 0x0098 => {
                let row = r.word()?;
                let indexed = row & 0x8000 != 0;
                let row_bytes = usize::from(row & 0x3fff);
                let bounds = r.rect()?;
                if op == 0x0090 && row_bytes >= 8 {
                    return Err("PICT BitsRect requires rowBytes<8".into());
                }
                let (width, height) = dimensions(bounds)?;
                let size = row_bytes
                    .checked_mul(height)
                    .filter(|n| *n <= 2_000_000)
                    .ok_or("PICT bitmap exceeds bounded surface")?;
                let depth;
                if row_bytes == 0 {
                    return Err("zero PICT row stride".into());
                }
                let palette = if indexed {
                    let header = r.take(36)?;
                    let word = |i| u16::from_be_bytes([header[i], header[i + 1]]);
                    if word(0) != 0
                        || word(2) != 0
                        || word(16) != 0
                        || word(18) != 8
                        || word(20) != 1
                        || word(22) != 8
                    {
                        return Err("PICT PixMap format outside indexed8 profile".into());
                    }
                    depth = 8;
                    r.take(4)?;
                    let flags = r.word()?;
                    let count = usize::from(r.word()?) + 1;
                    if count > 256 || flags & !0x8000 != 0 {
                        return Err("PICT color table outside indexed profile".into());
                    }
                    let mut colors = Vec::with_capacity(count);
                    for index in 0..count {
                        if r.word()? as usize != index {
                            return Err("nonpositional PICT color table outside profile".into());
                        }
                        colors.push([r.word()?, r.word()?, r.word()?]);
                    }
                    colors
                } else {
                    depth = 1;
                    vec![[65535; 3], [0; 3]]
                };
                if width > row_bytes * (8 / usize::from(depth)) {
                    return Err("PICT width exceeds row stride".into());
                }
                let source = r.rect()?;
                let destination = r.rect()?;
                let mode = r.word()?;
                if mode != 0 {
                    return Err(format!("PICT transfer mode{mode}outside srcCopy profile"));
                }
                let mut pixels = Vec::with_capacity(size);
                for _ in 0..height {
                    if op == 0x0090 || row_bytes < 8 {
                        pixels.extend_from_slice(r.take(row_bytes)?);
                    } else {
                        let n = if row_bytes > 250 {
                            usize::from(r.word()?)
                        } else {
                            usize::from(r.byte()?)
                        };
                        pixels.extend_from_slice(&unpack(r.take(n)?, row_bytes)?);
                    }
                }
                if depth == 8
                    && pixels.chunks_exact(row_bytes).any(|row| {
                        row[..width]
                            .iter()
                            .any(|&p| usize::from(p) >= palette.len())
                    })
                {
                    return Err("PICT pixel index outside its color table".into());
                }
                bitmaps.push(Bitmap {
                    bounds,
                    source,
                    destination,
                    clip,
                    row_bytes,
                    depth,
                    palette,
                    pixels,
                });
                if bitmaps.len() > 256 {
                    return Err("PICT bitmap operation count exceeds profile".into());
                }
            }
            _ => return Err(format!("unsupported PICT opcode{op:#06x}at{offset:#x}")),
        }
    }
    if r.position != bytes.len() {
        return Err("trailing data after PICT end opcode".to_string());
    }
    Ok(Picture { frame, bitmaps })
}
#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &[u8] = include_bytes!("../../probes/pictbitmap/pict-0.bin");
    #[test]
    fn truncated_or_unknown_picture_never_returns_a_partial_decoding() {
        assert!(decode(FIXTURE).is_ok());
        for length in 0..FIXTURE.len() {
            assert!(
                decode(&FIXTURE[..length]).is_err(),
                "accepted truncated prefix{length}"
            );
        }
        let mut unsupported = FIXTURE.to_vec();
        unsupported[40..42].copy_from_slice(&0x0031u16.to_be_bytes());
        assert!(decode(&unsupported)
            .unwrap_err()
            .contains("unsupported PICT opcode"));
        let mut bad_row = FIXTURE.to_vec();
        let length = bad_row.len();
        bad_row[length - 3] = 0x7f;
        assert!(decode(&bad_row).is_err());
    }
    #[test]
    fn large_picture_uses_handle_extent_instead_of_wrapped_pic_size() {
        let mut large = FIXTURE[..FIXTURE.len() - 2].to_vec();
        large.resize(large.len() + 80_000, 0);
        large.extend_from_slice(&[0, 255]);
        let low = large.len() as u16;
        large[..2].copy_from_slice(&low.to_be_bytes());
        let picture = decode(&large).unwrap();
        assert_eq!(picture.bitmaps.len(), 1);
        assert_eq!(
            picture.bitmaps[0].pixels,
            decode(FIXTURE).unwrap().bitmaps[0].pixels
        );
    }
}
