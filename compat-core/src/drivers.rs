//! Device Manager unit-table lookup for synchronous control/status failures.
//! There are no installed hardware drivers in the current emulated profile.
use crate::{Heap, MemoryBus};
/// Measured reference capacity; capacity is separate from installed drivers.
pub const PROFILE_UNIT_COUNT: u16 = 96;
#[derive(Debug, PartialEq, Eq)]
pub enum DriverError {
    MissingUnitTable,
    InstalledDriver(u16),
}
pub fn initialize<B: MemoryBus>(m: &mut B, heap: &mut Heap, count: u16) -> Result<(), i16> {
    if count > 0x7fff {
        return Err(-50);
    }
    let table = heap.new_ptr(m, u32::from(count) * 4, true)?;
    m.write_long(0x11c, table);
    m.write_word(0x1d2, count);
    Ok(())
}
pub fn unit_error<B: MemoryBus>(m: &mut B, pb: u32) -> Result<i16, DriverError> {
    let index = !m.read_word(pb + 24);
    let count = m.read_word(0x1d2);
    if index >= count {
        return Ok(-21);
    }
    let table = m.read_long(0x11c);
    if table == 0 {
        return Err(DriverError::MissingUnitTable);
    }
    if m.read_long(table + u32::from(index) * 4) == 0 {
        Ok(-22)
    } else {
        Err(DriverError::InstalledDriver(index))
    }
}
#[cfg(feature = "m68k-adapter")]
pub fn dispatch<B: m68k::AddressBus>(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
) -> Result<bool, DriverError> {
    if !matches!(op, 0xa004 | 0xa204 | 0xa005 | 0xa205) {
        return Ok(false);
    }
    let pb = cpu.dar[8];
    let error = unit_error(m, pb)?;
    MemoryBus::write_word(m, pb + 6, op);
    MemoryBus::write_word(m, pb + 16, error as u16);
    cpu.dar[0] = error as i32 as u32;
    cpu.set_ccr(8);
    cpu.pc += 2;
    Ok(true)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "m68k-adapter")]
    #[test]
    fn occupied_unit_is_an_unsupported_dispatch_without_guest_commit() {
        let mut m = m68k::LinearMemoryBus::new(0x10000);
        let mut heap = Heap::new(0x2000, 0xe000);
        initialize(&mut m, &mut heap, 4).unwrap();
        let table = MemoryBus::read_long(&mut m, 0x11c);
        MemoryBus::write_long(&mut m, table + 12, 0x4444);
        let pb = 0x1000;
        MemoryBus::write_word(&mut m, pb + 24, 0xfffc);
        MemoryBus::write_word(&mut m, pb + 6, 0x5a5a);
        MemoryBus::write_word(&mut m, pb + 16, 0x1234);
        let mut cpu = m68k::CpuCore::new();
        cpu.pc = 0x600;
        cpu.dar[8] = pb;
        cpu.dar[15] = 0xf000;
        cpu.dar[0] = 0x12345678;
        cpu.set_ccr(31);
        assert_eq!(
            dispatch(0xa204, &mut cpu, &mut m),
            Err(DriverError::InstalledDriver(3))
        );
        assert_eq!(
            (
                cpu.pc,
                cpu.dar[0],
                cpu.dar[8],
                cpu.dar[15],
                cpu.get_sr() & 31
            ),
            (0x600, 0x12345678, pb, 0xf000, 31)
        );
        assert_eq!(MemoryBus::read_word(&mut m, pb + 6), 0x5a5a);
        assert_eq!(MemoryBus::read_word(&mut m, pb + 16), 0x1234);
    }
    #[test]
    fn lookup_uses_the_public_table_capacity_and_actual_slot() {
        struct Memory(alloc::vec::Vec<u8>);
        impl MemoryBus for Memory {
            fn read_byte(&mut self, a: u32) -> u8 {
                self.0[a as usize]
            }
            fn write_byte(&mut self, a: u32, v: u8) {
                self.0[a as usize] = v;
            }
        }
        let mut m = Memory(alloc::vec![0;0x10000]);
        let mut heap = Heap::new(0x2000, 0xe000);
        initialize(&mut m, &mut heap, 4).unwrap();
        let pb = 0x1000;
        MemoryBus::write_word(&mut m, pb + 24, 0xfffb);
        assert_eq!(unit_error(&mut m, pb), Ok(-21));
        MemoryBus::write_word(&mut m, pb + 24, 0xfffc);
        assert_eq!(unit_error(&mut m, pb), Ok(-22));
        MemoryBus::write_word(&mut m, 0x1d2, 3);
        assert_eq!(unit_error(&mut m, pb), Ok(-21));
    }
}
