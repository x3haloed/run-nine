// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
use crate::{Bytes, Error, Output, Result};
#[path = "table.rs"]
mod table;

pub(super) fn decode<'a>(r: &mut Bytes<'a>, out: &mut Output) -> Result<()> {
    // Borrow memoized literals from the immutable compressed input. They do not
    // consume another copy of the output budget or depend on output reallocations.
    let mut memo: Vec<&'a [u8]> = Vec::new();
    loop {
        let tag = r.byte()?;
        match tag {
            0x00..=0x1f => {
                let words = if tag & 15 == 0 {
                    usize::try_from(r.integer()?).map_err(|_| Error::InvalidInteger)?
                } else {
                    usize::from(tag & 15)
                };
                let n = words.checked_mul(2).ok_or(Error::InvalidInteger)?;
                out.check(n)?;
                let literal = r.take(n)?;
                if tag & 16 != 0 {
                    memo.try_reserve(1).map_err(|_| Error::Allocation)?;
                    memo.push(literal);
                }
                out.bytes(literal)?;
            }
            0x20..=0x4a => {
                let index = match tag {
                    0x20 => 40 + usize::from(r.byte()?),
                    0x21 => 296 + usize::from(r.byte()?),
                    0x22 => 40 + usize::from(r.word()?),
                    _ => usize::from(tag - 0x23),
                };
                out.bytes(memo.get(index).ok_or(Error::InvalidReference(index))?)?;
            }
            0x4b..=0xfd => out.bytes(&table::WORDS[usize::from(tag - 0x4b)].to_be_bytes())?,
            0xfe => extension(r, out)?,
            0xff => {
                if r.pos != r.data.len() {
                    return Err(Error::TrailingData);
                }
                return Ok(());
            }
        }
    }
}
fn extension(r: &mut Bytes<'_>, out: &mut Output) -> Result<()> {
    match r.byte()? {
        0 => {
            let segment = r.integer()? as u16;
            let n = r.count16()?;
            out.check(
                n.checked_mul(8)
                    .and_then(|v| v.checked_add(6))
                    .ok_or(Error::InvalidInteger)?,
            )?;
            let tail = [0x3f, 0x3c, (segment >> 8) as u8, segment as u8, 0xa9, 0xf0];
            let mut address = 6u16;
            for _ in 0..n {
                address = address.wrapping_add((r.integer()? as u16).wrapping_sub(6));
                out.bytes(&tail)?;
                out.bytes(&address.to_be_bytes())?;
            }
            out.bytes(&tail)
        }
        1 => {
            let mut target = r.integer()? as u16;
            let delta = r.integer()? as u16;
            let n = r.count16()? + 1;
            let mut a5 = r.integer()? as u16;
            out.check(n * 8)?;
            for i in 0..n {
                if i != 0 {
                    target = target.wrapping_sub(8);
                    a5 = if delta == 0 {
                        r.integer()? as u16
                    } else {
                        a5.wrapping_add(delta)
                    };
                }
                out.bytes(&[0x61, 0])?;
                out.bytes(&target.to_be_bytes())?;
                out.bytes(&[0x4e, 0xed])?;
                out.bytes(&a5.to_be_bytes())?;
            }
            Ok(())
        }
        kind @ (2 | 3) => {
            let value = r.integer()? as u16;
            let n = r.count16()? + 1;
            let bytes = value.to_be_bytes();
            out.repeat(if kind == 2 { &bytes[1..] } else { &bytes }, n)
        }
        kind @ (4..=6) => {
            let mut value = r.integer()? as u32;
            let n = r.count16()? + 1;
            let width = if kind == 6 { 4 } else { 2 };
            out.check(n * width)?;
            for i in 0..n {
                if i != 0 {
                    let diff = if kind == 4 {
                        i32::from(r.byte()? as i8)
                    } else {
                        r.integer()?
                    };
                    value = value.wrapping_add(diff as u32);
                }
                let bytes = value.to_be_bytes();
                out.bytes(&bytes[4 - width..])?;
            }
            Ok(())
        }
        kind => Err(Error::InvalidToken(kind)),
    }
}
