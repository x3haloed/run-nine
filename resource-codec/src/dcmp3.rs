// Copyright (c) 2026 RUN9 contributors. SPDX-License-Identifier: MIT OR Apache-2.0
// Threshold facts derived from pinned MIT ResourceDASM; see NOTICES.txt.
use crate::{Error, Output, Result};
struct Bits<'a> {
    data: &'a [u8],
    byte: usize,
    bit: u8,
}
impl Bits<'_> {
    fn read(&mut self, count: u8) -> Result<usize> {
        let mut value = 0;
        for _ in 0..count {
            let b = *self.data.get(self.byte).ok_or(Error::Truncated)?;
            value = (value << 1) | usize::from((b >> (7 - self.bit)) & 1);
            self.bit += 1;
            if self.bit == 8 {
                self.bit = 0;
                self.byte += 1;
            }
        }
        Ok(value)
    }
    fn finish(&self) -> Result<()> {
        if self.bit == 0 {
            if self.byte != self.data.len() {
                return Err(Error::TrailingData);
            }
        } else {
            if self.byte + 1 != self.data.len() {
                return Err(Error::TrailingData);
            }
            if self.data[self.byte] & ((1 << (8 - self.bit)) - 1) != 0 {
                return Err(Error::TrailingData);
            }
        }
        Ok(())
    }
}
fn literal_length(b: &mut Bits<'_>) -> Result<usize> {
    if b.read(1)? == 0 {
        return Ok(1);
    }
    Ok(match b.read(2)? {
        0 => 2,
        1 => 3,
        2 => 4 + b.read(2)?,
        _ => {
            let n = b.read(4)?;
            match n {
                0..=7 => n + 8,
                8..=11 => 16 + (n - 8) * 4 + b.read(2)?,
                _ => 32 + (n - 12) * 8 + b.read(3)?,
            }
        }
    })
}
fn run_code(b: &mut Bits<'_>) -> Result<usize> {
    let mut prefix = 0;
    while prefix < 10 && b.read(1)? != 0 {
        prefix += 1;
    }
    Ok(match prefix {
        0 => b.read(1)?,
        1 => {
            if b.read(1)? == 0 {
                2
            } else {
                3 + b.read(1)?
            }
        }
        2 => {
            if b.read(1)? == 0 {
                5 + b.read(1)?
            } else {
                7 + b.read(2)?
            }
        }
        3 => 11 + b.read(3)?,
        4 => 19 + b.read(3)?,
        n => (1usize << n) - 5 + b.read(n)?,
    })
}
fn offset(b: &mut Bits<'_>, produced: usize) -> Result<usize> {
    // Select the coding family using format-specific (nonuniform) thresholds.
    let first_bits = match produced {
        0..=10 => 0,
        11..=20 => 1,
        21..=40 => 2,
        41..=80 => 3,
        81..=160 => 4,
        161..=0x2a0 => 5,
        0x2a1..=1000 => 6,
        1001..=0xa80 => 7,
        0xa81..=0x1500 => 8,
        0x1501..=0x2a00 => 9,
        0x2a01..=0x5400 => 10,
        0x5401..=0xa800 => 11,
        0xa801..=0x11170 => 12,
        0x11171..=0x2a000 => 13,
        _ => 14,
    };
    if b.read(1)? == 0 {
        return Ok(1 + b.read(first_bits)?);
    }
    if b.read(1)? == 0 {
        return Ok((1 << first_bits) + 1 + b.read(first_bits + 2)?);
    }
    let base = 5 * (1usize << first_bits) + 1;
    let width = if first_bits == 7 {
        // Both anomalies are retained exactly from the permissive reference.
        match produced {
            0..=0x282 => 1,
            0x283..=0x284 => 2,
            0x285..=0x290 => 4,
            0x291..=0x2a0 => 5,
            0x2a1..=0x2c0 => 6,
            0x2c1..=0x300 => 7,
            0x301..=0x380 => 8,
            0x381..=0x480 => 9,
            0x481..=0x66c => 10,
            _ => 11,
        }
    } else {
        let max_width = first_bits + 4;
        let mut w = 1;
        while produced > base - 1 + (1usize << w) {
            w += 1;
            if w > max_width {
                return Err(Error::InvalidOffset(produced));
            }
        }
        if first_bits == 14 && w == 7 {
            8
        } else {
            w
        }
    };
    Ok(base + b.read(width)?)
}
pub(super) fn decode(input: &[u8], out: &mut Output) -> Result<()> {
    let mut b = Bits {
        data: input,
        byte: 0,
        bit: 0,
    };
    let mut allow_literal = true;
    while out.data.len() < out.declared {
        let code = run_code(&mut b)?;
        if code == 0 && allow_literal {
            let n = literal_length(&mut b)?;
            out.check(n)?;
            for _ in 0..n {
                let v = b.read(8)? as u8;
                out.bytes(&[v])?;
            }
            allow_literal = n == 63;
        } else {
            let n = code + if allow_literal { 2 } else { 3 };
            let distance = offset(&mut b, out.data.len())?;
            out.copy(distance, n)?;
            allow_literal = true;
        }
    }
    b.finish()
}
