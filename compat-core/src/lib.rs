#![no_std]
extern crate alloc;
pub mod audio;
pub mod catalog;
pub mod color_seed;
pub mod cursor;
pub mod mouse;
pub mod drivers;
mod managers;
pub mod pixels;
pub mod pict;
pub mod retrace;
pub mod timers;
#[cfg(feature = "m68k-adapter")]
pub use m68k;
pub use managers::{Heap, Rect};
#[cfg(feature = "m68k-adapter")]
pub mod execution;
#[cfg(feature = "m68k-adapter")]
pub mod traps;

/// Macintosh guest addresses and byte order, independent of a host CPU bus.
pub trait MemoryBus {
    fn read_byte(&mut self, address: u32) -> u8;
    fn write_byte(&mut self, address: u32, value: u8);
    fn read_word(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.read_byte(a), self.read_byte(a + 1)])
    }
    fn write_word(&mut self, a: u32, v: u16) {
        let b = v.to_be_bytes();
        self.write_byte(a, b[0]);
        self.write_byte(a + 1, b[1]);
    }
    fn read_long(&mut self, a: u32) -> u32 {
        ((self.read_word(a) as u32) << 16) | self.read_word(a + 2) as u32
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.write_word(a, (v >> 16) as u16);
        self.write_word(a + 2, v as u16);
    }
}
#[cfg(feature = "m68k-adapter")]
impl<T: m68k::AddressBus> MemoryBus for T {
    fn read_byte(&mut self, a: u32) -> u8 {
        m68k::AddressBus::read_byte(self, a)
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        m68k::AddressBus::write_byte(self, a, v)
    }
    fn read_word(&mut self, a: u32) -> u16 {
        m68k::AddressBus::read_word(self, a)
    }
    fn write_word(&mut self, a: u32, v: u16) {
        m68k::AddressBus::write_word(self, a, v)
    }
    fn read_long(&mut self, a: u32) -> u32 {
        m68k::AddressBus::read_long(self, a)
    }
    fn write_long(&mut self, a: u32, v: u32) {
        m68k::AddressBus::write_long(self, a, v)
    }
}

/// The same core-v1 experiment runs in the hosted runtime and native kernel.
pub fn core_probe<B: MemoryBus>(memory: &mut B) -> Result<alloc::string::String, i16> {
    let mut heap = Heap::new(0x20000, 0xe0000);
    let h = heap.new_handle(memory, 16, false)?;
    let allocation_error = heap.error;
    let initial_size = heap.size(h)?;
    heap.lock(h, true)?;
    let p = heap.data(h)?;
    for (i, b) in b"Macintosh probe\0".iter().enumerate() {
        memory.write_byte(p + i as u32, *b);
    }
    heap.lock(h, false)?;
    let _ = heap.resize(memory, h, 64);
    let resize_error = heap.error;
    let resized_size = heap.size(h)?;
    heap.lock(h, true)?;
    let p = heap.data(h)?;
    let preserved = b"Macintosh probe\0"
        .iter()
        .enumerate()
        .all(|(i, b)| memory.read_byte(p + i as u32) == *b);
    heap.lock(h, false)?;
    heap.dispose(memory, h)?;
    let a = Rect {
        top: 0,
        left: 0,
        bottom: 20,
        right: 30,
    };
    let b = Rect {
        top: 10,
        left: 15,
        bottom: 40,
        right: 50,
    };
    let intersection = a.intersection(b);
    let rect = intersection.unwrap_or(Rect {
        top: 0,
        left: 0,
        bottom: 0,
        right: 0,
    });
    Ok(alloc::format!("{{\"schema\":1,\"probe\":\"core-v1\",\"memory\":{{\"allocation_error\":{allocation_error},\"initial_size\":{initial_size},\"resize_error\":{resize_error},\"resized_size\":{resized_size},\"preserved\":{}}},\"rectangles\":{{\"intersects\":{},\"intersection\":[{},{},{},{}],\"origin_inside\":{},\"right_edge_inside\":{},\"bottom_edge_inside\":{}}}}}",preserved as u8,intersection.is_some() as u8,rect.top,rect.left,rect.bottom,rect.right,a.contains(0,0) as u8,a.contains(0,30) as u8,a.contains(20,0) as u8))
}

pub mod extended;

#[cfg(feature = "m68k-adapter")]
pub mod sane;

pub mod legacy_colors;

pub mod random;

pub mod volume;
