//! Bounded MC68000 SANE package services, shared with no host OS assumptions.
use crate::extended::{Extended80, Rounding};
use m68k::{AddressBus, CpuCore};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Address(u32, u32),
    HaltUnsupported(u16),
    PrecisionUnsupported(u16),
    IntegerMagnitudeUnsupported,
}
impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::IntegerMagnitudeUnsupported => write!(
                f,
                "SANE integer conversion status for finite magnitude >= 2^63 is not implemented"
            ),
            Self::PrecisionUnsupported(mode) => {
                write!(f, "SANE reserved precision {mode:#x} is not implemented")
            }
            Self::Address(p, n) => {
                write!(f, "SANE operand outside guest memory: {p:#x}, {n} bytes")
            }
            Self::HaltUnsupported(flags) => write!(
                f,
                "SANE halt callback for exceptions {flags:#x} is not implemented"
            ),
        }
    }
}
pub struct Sane {
    environment: u16,
    limit: u32,
}
impl Sane {
    pub fn new(limit: u32) -> Self {
        Self {
            environment: 0,
            limit,
        }
    }
    fn check(&self, p: u32, n: u32) -> Result<(), Error> {
        if p < 0x100 || p.checked_add(n).is_none_or(|end| end > self.limit) {
            Err(Error::Address(p, n))
        } else {
            Ok(())
        }
    }
    fn read<B: AddressBus>(&self, m: &mut B, p: u32) -> Result<Extended80, Error> {
        self.check(p, 10)?;
        let mut b = [0; 10];
        for (i, v) in b.iter_mut().enumerate() {
            *v = m.read_byte(p + i as u32);
        }
        Ok(Extended80::from_bytes(b))
    }
    fn raise(&mut self, flags: u16) -> Result<(), Error> {
        if self.environment & (flags >> 8) & 0x1f != 0 {
            return Err(Error::HaltUnsupported(flags));
        }
        self.environment |= flags;
        Ok(())
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
    ) -> Result<bool, Error> {
        if op != 0xa9eb && op != 0xa9ec {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        self.check(sp, 6)?;
        let selector = m.read_word(sp);
        let pointer = m.read_long(sp + 2);
        let x = if cpu.flag_x() { 16 } else { 0 };
        let pop;
        match (op, selector) {
            (0xa9eb, 0x2010) | (0xa9eb, 0x2810) => {
                self.check(sp, 10)?;
                let width = if selector == 0x2010 { 2 } else { 4 };
                self.check(pointer, width)?;
                let source = m.read_long(sp + 6);
                let value = self.read(m, source)?;
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition, override_x) = value
                    .integer_sane(rounding, width * 8)
                    .ok_or(Error::IntegerMagnitudeUnsupported)?;
                let condition = if self.environment == 0 {
                    condition
                } else {
                    condition & !4
                };
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                if width == 2 {
                    m.write_word(pointer, result as u16);
                } else {
                    m.write_long(pointer, result);
                }
                let x = override_x.map_or(x, |bit| if bit { 16 } else { 0 });
                cpu.set_ccr(x | condition);
                pop = 10;
            }
            (0xa9eb, 0x14) | (0xa9eb, 0x16) => {
                let value = self.read(m, pointer)?;
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition) =
                    value.integral_sane(rounding, precision, selector == 0x16);
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | condition);
                pop = 6;
            }
            (0xa9eb, 0) | (0xa9eb, 2) | (0xa9eb, 0x0800) | (0xa9eb, 0x0802) | (0xa9eb, 0x12) => {
                let left = self.read(m, pointer)?;
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition) = if selector == 0x12 {
                    pop = 6;
                    left.sqrt_sane(rounding, precision)
                } else {
                    self.check(sp, 10)?;
                    let source = m.read_long(sp + 6);
                    let right = if selector & 0x0800 != 0 {
                        self.check(source, 8)?;
                        let bits = u64::from(m.read_long(source)) << 32
                            | u64::from(m.read_long(source + 4));
                        Extended80::from_double_bits(bits)
                    } else {
                        self.read(m, source)?
                    };
                    pop = 10;
                    left.add_sane(right, selector & 0x07ff == 2, rounding, precision)
                };
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | condition);
            }
            (0xa9eb, 0x0008) | (0xa9eb, 0x0808) | (0xa9eb, 0x000a) | (0xa9eb, 0x080a) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                let left = self.read(m, pointer)?;
                let right = if selector & 0x0800 != 0 {
                    self.check(source, 8)?;
                    let bits =
                        u64::from(m.read_long(source)) << 32 | u64::from(m.read_long(source + 4));
                    Extended80::from_double_bits(bits)
                } else {
                    self.read(m, source)?
                };
                let (condition, flags) = if selector & 0x07ff == 0x000a {
                    left.compare_signaling_sane(right)
                } else {
                    left.compare_sane(right)
                };
                self.raise(flags)?;
                cpu.set_ccr(condition);
                pop = 10;
            }
            (0xa9eb, 0x0017) => {
                self.check(pointer, 2)?;
                m.write_word(pointer, self.environment);
                self.environment = 0;
                cpu.set_ccr(x | 4);
                pop = 6;
            }
            (0xa9eb, 0x0019) => {
                self.check(pointer, 2)?;
                let saved = m.read_word(pointer);
                let pending = self.environment & 0x1f00;
                if saved & (pending >> 8) & 0x1f != 0 {
                    return Err(Error::HaltUnsupported(pending));
                }
                self.environment = saved | pending;
                cpu.set_ccr(x);
                pop = 6;
            }
            (0xa9eb, 0x001c) => {
                self.check(sp, 10)?;
                self.check(pointer, 2)?;
                let source = m.read_long(sp + 6);
                let input = self.read(m, source)?;
                let result = input.classify_sane();
                m.write_word(pointer, result as u16);
                cpu.set_ccr(16 | if result < 0 { 8 } else { 0 });
                pop = 10;
            }
            (0xa9eb, 0x000e) | (0xa9eb, 0x0010) => {
                self.check(sp, 10)?;
                self.check(pointer, 10)?;
                let source = m.read_long(sp + 6);
                let input = self.read(m, source)?;
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition) = input.convert_sane(rounding, precision);
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | condition);
                pop = 10;
            }
            (0xa9ec, 0x8012) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                let base = self.read(m, pointer)?;
                let exponent = self.read(m, source)?;
                let Some((result, flags, x_mode)) = base.small_integer_power_sane(exponent) else {
                    return Ok(false);
                };
                self.raise(flags)?;
                for (i, value) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, value);
                }
                cpu.set_ccr(match x_mode { 2 => x, 1 => 16, _ => 0 });
                pop = 10;
            }
            (0xa9ec, 0x0018) | (0xa9ec, 0x001a) | (0xa9ec, 0x001c) => {
                let input = self.read(m, pointer)?;
                let (result, flags, x_mode) = match selector {
                    0x18 => input.sine_sane(),
                    0x1a => input.cosine_sane(),
                    _ => input.tangent_sane(),
                };
                self.raise(flags)?;
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(match x_mode {
                    2 => x,
                    1 => 16,
                    _ => 0,
                });
                pop = 6;
            }
            (0xa9eb, 0x200e) => {
                self.check(sp, 10)?;
                self.check(pointer, 10)?;
                let source = m.read_long(sp + 6);
                self.check(source, 2)?;
                if self.environment & 0x60 == 0x60 {
                    return Err(Error::PrecisionUnsupported(0x60));
                }
                let input = m.read_word(source) as i16;
                let result = Extended80::from_integer(input);
                self.environment &= !0x80;
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | if input == 0 { 4 } else { 0 });
                pop = 10;
            }
            (0xa9eb, 0x080e | 0x100e | 0x280e) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                self.check(pointer, 10)?;
                let double = selector == 0x080e;
                self.check(source, if double { 8 } else { 4 })?;
                let bits = if !double {
                    u64::from(m.read_long(source))
                } else {
                    u64::from(m.read_long(source)) << 32 | u64::from(m.read_long(source + 4))
                };
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition) = match selector {
                    0x100e => Extended80::from_single_sane(bits as u32, rounding, precision),
                    0x280e => Extended80::from_long_sane(bits as i32, rounding, precision),
                    _ => Extended80::from_double_sane(bits, rounding, precision),
                };
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | condition);
                pop = 10;
            }
            (0xa9eb, 0x0810) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                self.check(pointer, 8)?;
                let input = self.read(m, source)?;
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (bits, flags, condition) = input.double_sane(rounding, precision);
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                m.write_long(pointer, (bits >> 32) as u32);
                m.write_long(pointer + 4, bits as u32);
                cpu.set_ccr(x | condition);
                pop = 10;
            }
            (0xa9eb, 4) | (0xa9eb, 6) | (0xa9eb, 0x0804) | (0xa9eb, 0x0806) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                let left = self.read(m, pointer)?;
                let right = if selector & 0x0800 != 0 {
                    self.check(source, 8)?;
                    let bits =
                        u64::from(m.read_long(source)) << 32 | u64::from(m.read_long(source + 4));
                    Extended80::from_double_bits(bits)
                } else {
                    self.read(m, source)?
                };
                let precision = match self.environment & 0x60 {
                    0 => 64,
                    0x20 => 53,
                    0x40 => 24,
                    mode => return Err(Error::PrecisionUnsupported(mode)),
                };
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (result, flags, condition) = if selector & 0x07ff == 4 {
                    left.multiply_sane(right, rounding, precision)
                } else {
                    left.divide_sane(right, rounding, precision)
                };
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(x | condition);
                pop = 10;
            }
            (0xa9eb, 1) | (0xa9eb, 3) => {
                self.check(pointer, 2)?;
                let word = if selector == 1 {
                    let word = m.read_word(pointer);
                    self.environment = word;
                    word
                } else {
                    m.write_word(pointer, self.environment);
                    self.environment
                };
                cpu.set_ccr(
                    x | if word == 0 {
                        4
                    } else if word & 0x8000 != 0 {
                        8
                    } else {
                        0
                    },
                );
                pop = 6;
            }
            (0xa9ec, 0x001e) => {
                let input = self.read(m, pointer)?;
                let (result, flags) = input.atan(Rounding::Nearest);
                self.raise(flags)?;
                for (i, v) in result.to_bytes().into_iter().enumerate() {
                    m.write_byte(pointer + i as u32, v);
                }
                cpu.set_ccr(if input.finite_at_least_one() { 0 } else { x });
                pop = 6;
            }
            (0xa9eb, 0x1010) => {
                self.check(sp, 10)?;
                let source = m.read_long(sp + 6);
                self.check(pointer, 4)?;
                let input = self.read(m, source)?;
                let rounding = match self.environment >> 13 & 3 {
                    0 => Rounding::Nearest,
                    1 => Rounding::Up,
                    2 => Rounding::Down,
                    _ => Rounding::TowardZero,
                };
                let (bits, flags) = input.single(rounding);
                self.raise(flags)?;
                self.environment = (self.environment & !0x80) | (flags & 0x80);
                m.write_long(pointer, bits);
                cpu.set_ccr(x);
                pop = 10;
            }
            _ => return Ok(false),
        }
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
