// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
//! Classic resource decompression. See docs/resource-codec-format.md for grammar,
//! reference disagreements and policies. Third-party notices are in NOTICES.txt.
mod dcmp0;
mod dcmp3;

/// RUN9 policy, not a format or native Resource Manager limit.
pub const DEFAULT_MAX_OUTPUT: usize = 64 * 1024 * 1024;

/// All errors own their data and never borrow the input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    Truncated,
    InvalidHeader(&'static str),
    Unsupported { version: u8, decompressor: i16 },
    OutputLimit { declared: usize, limit: usize },
    Allocation,
    InvalidInteger,
    InvalidToken(u8),
    InvalidReference(usize),
    InvalidOffset(usize),
    LengthMismatch,
    TrailingData,
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}
impl std::error::Error for Error {}
type Result<T> = std::result::Result<T, Error>;

/// Returns `None` when the resource attribute bit is clear or signature absent.
pub fn decompress_if_needed(attrs: u8, data: &[u8]) -> Result<Option<Vec<u8>>> {
    decompress_if_needed_with_limit(attrs, data, DEFAULT_MAX_OUTPUT)
}

/// Like [`decompress_if_needed`], with a caller-selected maximum decoded length.
/// Allocation is fallible; inputs are never modified. The limit applies before
/// allocating and to every output operation (plus one dcmp0 alignment byte).
pub fn decompress_if_needed_with_limit(
    attrs: u8,
    data: &[u8],
    max_output: usize,
) -> Result<Option<Vec<u8>>> {
    if attrs & 1 == 0 || !data.starts_with(&[0xa8, 0x9f, 0x65, 0x72]) {
        return Ok(None);
    }
    let mut r = Bytes::new(data);
    r.take(4)?;
    let header_size = r.word()?;
    if header_size != 0 && header_size != 18 {
        return Err(Error::InvalidHeader("size"));
    }
    let version = r.byte()?;
    if r.byte()? != 1 {
        return Err(Error::InvalidHeader("attributes"));
    }
    let declared = usize::try_from(r.long()?).map_err(|_| Error::InvalidInteger)?;
    let params = r.take(6)?;
    let id = match version {
        8 => {
            if params[4..] != [0, 0] {
                return Err(Error::InvalidHeader("reserved"));
            }
            i16::from_be_bytes([params[2], params[3]])
        }
        9 => i16::from_be_bytes([params[0], params[1]]),
        _ => {
            return Err(Error::Unsupported {
                version,
                decompressor: -1,
            })
        }
    };
    if !matches!((version, id), (8, 0) | (9, 3)) {
        return Err(Error::Unsupported {
            version,
            decompressor: id,
        });
    }
    if declared > max_output {
        return Err(Error::OutputLimit {
            declared,
            limit: max_output,
        });
    }
    let mut out = Output::new(declared, version == 8)?;
    match version {
        8 => dcmp0::decode(&mut r, &mut out)?,
        9 => dcmp3::decode(&data[r.pos..], &mut out)?,
        _ => unreachable!(),
    }
    if out.data.len() < declared {
        return Err(Error::LengthMismatch);
    }
    out.data.truncate(declared);
    Ok(Some(out.data))
}

struct Bytes<'a> {
    data: &'a [u8],
    pos: usize,
}
impl<'a> Bytes<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }
    fn take(&mut self, len: usize) -> Result<&'a [u8]> {
        let end = self.pos.checked_add(len).ok_or(Error::Truncated)?;
        let v = self.data.get(self.pos..end).ok_or(Error::Truncated)?;
        self.pos = end;
        Ok(v)
    }
    fn byte(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn word(&mut self) -> Result<u16> {
        let v = self.take(2)?;
        Ok(u16::from_be_bytes([v[0], v[1]]))
    }
    fn long(&mut self) -> Result<u32> {
        let v = self.take(4)?;
        Ok(u32::from_be_bytes([v[0], v[1], v[2], v[3]]))
    }
    fn integer(&mut self) -> Result<i32> {
        let h = self.byte()?;
        match h {
            0..=127 => Ok(i32::from(h)),
            255 => Ok(self.long()? as i32),
            _ => Ok((i32::from(h) << 8 | i32::from(self.byte()?)) - 0xc000),
        }
    }
    fn count16(&mut self) -> Result<usize> {
        Ok((self.integer()? as u32 & 0xffff) as usize)
    }
}
struct Output {
    data: Vec<u8>,
    declared: usize,
    ceiling: usize,
}
impl Output {
    fn new(declared: usize, odd_padding: bool) -> Result<Self> {
        let ceiling = declared
            .checked_add(usize::from(odd_padding && declared & 1 != 0))
            .ok_or(Error::InvalidInteger)?;
        let mut data = Vec::new();
        data.try_reserve_exact(ceiling)
            .map_err(|_| Error::Allocation)?;
        Ok(Self {
            data,
            declared,
            ceiling,
        })
    }
    fn check(&self, count: usize) -> Result<()> {
        if count > self.ceiling - self.data.len() {
            Err(Error::LengthMismatch)
        } else {
            Ok(())
        }
    }
    fn bytes(&mut self, v: &[u8]) -> Result<()> {
        self.check(v.len())?;
        self.data.extend_from_slice(v);
        Ok(())
    }
    fn repeat(&mut self, v: &[u8], count: usize) -> Result<()> {
        self.check(v.len().checked_mul(count).ok_or(Error::InvalidInteger)?)?;
        for _ in 0..count {
            self.data.extend_from_slice(v);
        }
        Ok(())
    }
    fn copy(&mut self, offset: usize, count: usize) -> Result<()> {
        if offset == 0 || offset > self.data.len() {
            return Err(Error::InvalidOffset(offset));
        }
        self.check(count)?;
        for _ in 0..count {
            let v = self.data[self.data.len() - offset];
            self.data.push(v);
        }
        Ok(())
    }
}
