//! Cursor-device coordinates and the classic low-memory cursor update request.
use crate::MemoryBus;

pub const TEMP: u32 = 0x828;
pub const RAW: u32 = 0x82c;
pub const LOCATION: u32 = 0x830;
pub const PIN: u32 = 0x834;
pub const NEW: u32 = 0x8ce;
pub const COUPLE: u32 = 0x8cf;
pub type Point = (i16, i16);

pub fn read<B: MemoryBus>(m: &mut B, p: u32) -> Point {
    (m.read_word(p) as i16, m.read_word(p + 2) as i16)
}
pub fn write<B: MemoryBus>(m: &mut B, p: u32, point: Point) {
    m.write_word(p, point.0 as u16);
    m.write_word(p + 2, point.1 as u16);
}
/// An absolute device snapshot, also used by headless input providers.
pub fn publish<B: MemoryBus>(m: &mut B, point: Point) {
    for p in [TEMP, RAW, LOCATION] {
        write(m, p, point);
    }
}
pub fn initialize<B: MemoryBus>(m: &mut B, width: i16, height: i16) {
    publish(m, (0, 0));
    write(m, PIN, (0, 0));
    write(m, PIN + 4, (height, width));
    m.write_byte(NEW, 0);
    m.write_byte(COUPLE, 255);
}

#[derive(Default)]
pub struct Driver {
    device: Point,
}
impl Driver {
    /// Set the initial position when attaching an absolute host pointer.
    pub fn absolute<B: MemoryBus>(&mut self, m: &mut B, point: Point) {
        self.device = point;
        publish(m, point);
    }
    /// Service a guest cursor request independently of host motion.
    pub fn service<B: MemoryBus>(&mut self, m: &mut B) {
        let coupled = m.read_byte(COUPLE) != 0;
        if m.read_byte(NEW) == 0 {
            if coupled && read(m, LOCATION) != self.device {
                publish(m, self.device);
            }
            return;
        }
        let temp = read(m, TEMP);
        let raw = read(m, RAW);
        if temp != raw {
            let adjusted = (
                self.device.0.wrapping_add(temp.0.wrapping_sub(raw.0)),
                self.device.1.wrapping_add(temp.1.wrapping_sub(raw.1)),
            );
            write(m, TEMP, adjusted);
        }
        if coupled {
            let point = read(m, TEMP);
            let top_left = read(m, PIN);
            let bottom_right = read(m, PIN + 4);
            let clamp = |v: i16, lo: i16, hi: i16| {
                if hi > lo {
                    v.clamp(lo, hi - 1)
                } else {
                    lo
                }
            };
            self.device = (
                clamp(point.0, top_left.0, bottom_right.0),
                clamp(point.1, top_left.1, bottom_right.1),
            );
            publish(m, self.device);
        }
        m.write_byte(NEW, 0);
    }
    /// A relative device packet adds motion without overwriting guest recentering.
    pub fn motion<B: MemoryBus>(&mut self, m: &mut B, delta: Point) {
        self.service(m);
        if delta != (0, 0) {
            let temp = read(m, TEMP);
            write(
                m,
                TEMP,
                (temp.0.wrapping_add(delta.0), temp.1.wrapping_add(delta.1)),
            );
            m.write_byte(NEW, 255);
            self.service(m);
            crate::cursor::moved(m);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Memory([u8; 4096]);
    impl MemoryBus for Memory {
        fn read_byte(&mut self, address: u32) -> u8 {
            self.0[address as usize]
        }
        fn write_byte(&mut self, address: u32, value: u8) {
            self.0[address as usize] = value;
        }
    }
    #[test]
    fn device_packets_continue_after_repeated_guest_recentering() {
        let mut m = Memory([0; 4096]);
        initialize(&mut m, 640, 480);
        let mut driver = Driver::default();
        driver.absolute(&mut m, (240, 320));
        for _ in 0..128 {
            write(&mut m, TEMP, (240, 320));
            write(&mut m, RAW, (240, 320));
            m.write_word(NEW, 65535);
            driver.motion(&mut m, (0, 80));
            assert_eq!(read(&mut m, RAW), (240, 400));
            assert_eq!(read(&mut m, LOCATION), (240, 400));
        }
    }
}
