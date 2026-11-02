#![no_std]
#![no_main]
extern crate alloc;
use alloc::{vec, vec::Vec};
use classic_core::m68k::AddressBus;
use core::{
    alloc::{GlobalAlloc, Layout},
    arch::{asm, global_asm},
    fmt::{self, Write},
    panic::PanicInfo,
    ptr,
    sync::atomic::{AtomicUsize, Ordering},
};
global_asm!(include_str!("boot.S"));

const HEAP_SIZE: usize = 16 * 1024 * 1024;
#[repr(align(4096))]
struct Storage([u8; HEAP_SIZE]);
static mut STORAGE: Storage = Storage([0; HEAP_SIZE]);
struct Bump {
    next: AtomicUsize,
}
unsafe impl GlobalAlloc for Bump {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let base = ptr::addr_of_mut!(STORAGE.0) as *mut u8 as usize;
        let result = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |offset| {
                let at = (base + offset).checked_add(layout.align() - 1)? & !(layout.align() - 1);
                let end = at.checked_add(layout.size())?.checked_sub(base)?;
                if end <= HEAP_SIZE {
                    Some(end)
                } else {
                    None
                }
            });
        match result {
            Ok(old) => ((base + old + layout.align() - 1) & !(layout.align() - 1)) as *mut u8,
            Err(_) => ptr::null_mut(),
        }
    }
    unsafe fn dealloc(&self, _: *mut u8, _: Layout) {}
}
#[global_allocator]
static ALLOCATOR: Bump = Bump {
    next: AtomicUsize::new(0),
};

struct Uart;
impl Write for Uart {
    fn write_str(&mut self, text: &str) -> fmt::Result {
        for byte in text.bytes() {
            unsafe {
                while ptr::read_volatile(0x09000018 as *const u32) & (1 << 5) != 0 {
                    core::hint::spin_loop();
                }
                ptr::write_volatile(0x09000000 as *mut u32, byte as u32);
            }
        }
        Ok(())
    }
}
struct Memory {
    bytes: Vec<u8>,
}
impl AddressBus for Memory {
    fn read_byte(&mut self, a: u32) -> u8 {
        self.bytes[a as usize]
    }
    fn write_byte(&mut self, a: u32, v: u8) {
        self.bytes[a as usize] = v;
    }
    fn read_word(&mut self, a: u32) -> u16 {
        u16::from_be_bytes([self.read_byte(a), self.read_byte(a + 1)])
    }
    fn write_word(&mut self, a: u32, v: u16) {
        let bytes = v.to_be_bytes();
        self.write_byte(a, bytes[0]);
        self.write_byte(a + 1, bytes[1]);
    }
    fn read_long(&mut self, a: u32) -> u32 {
        ((self.read_word(a) as u32) << 16) | self.read_word(a + 2) as u32
    }
    fn write_long(&mut self, a: u32, v: u32) {
        self.write_word(a, (v >> 16) as u16);
        self.write_word(a + 2, v as u16);
    }
}
#[no_mangle]
pub extern "C" fn kernel_main(boot_data: usize) -> ! {
    let mut console = Uart;
    let el: u64;
    let sctlr: u64;
    unsafe {
        asm!("mrs {0}, CurrentEL",out(reg)el,options(nomem,nostack));
        asm!("mrs {0}, SCTLR_EL1",out(reg)sctlr,options(nomem,nostack));
    }
    writeln!(console, "CLASSIC-NATIVE-BEGIN").unwrap();
    writeln!(
        console,
        "platform=qemu-virt cpu=aarch64 el={} mmu={}",
        el >> 2,
        sctlr & 1
    )
    .unwrap();
    writeln!(console, "boot-data={boot_data:#x}").unwrap();
    assert_eq!(el >> 2, 1);
    assert_eq!(sctlr & 1, 0);
    let mut memory = Memory {
        bytes: vec![0; 8 * 1024 * 1024],
    };
    let observation = classic_core::core_probe(&mut memory).expect("core experiment failed");
    writeln!(console, "{observation}").unwrap();
    let execution =
        classic_core::execution::owned_probe(&mut memory).expect("owned68K execution failed");
    writeln!(
        console,
        "OWNED68K instructions={} traps={} pc={:#x} sp={:#x} preserved={}",
        execution.instructions,
        execution.traps.len(),
        execution.return_pc,
        execution.return_sp,
        execution.registers_preserved as u8
    )
    .unwrap();
    writeln!(console, "{}", execution.observation()).unwrap();
    writeln!(console, "CLASSIC-NATIVE-PASS").unwrap();
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }
}
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let _ = writeln!(Uart, "CLASSIC-NATIVE-PANIC: {info}");
    loop {
        unsafe {
            asm!("wfe", options(nomem, nostack));
        }
    }
}
// Freestanding memory intrinsics; volatile loops prevent recursive lowering.
#[no_mangle]
pub unsafe extern "C" fn memset(dest: *mut u8, value: i32, len: usize) -> *mut u8 {
    for i in 0..len {
        ptr::write_volatile(dest.add(i), value as u8);
    }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memcpy(dest: *mut u8, source: *const u8, len: usize) -> *mut u8 {
    for i in 0..len {
        ptr::write_volatile(dest.add(i), ptr::read_volatile(source.add(i)));
    }
    dest
}
#[no_mangle]
pub unsafe extern "C" fn memmove(dest: *mut u8, source: *const u8, len: usize) -> *mut u8 {
    if (dest as usize) > (source as usize) {
        for i in (0..len).rev() {
            ptr::write_volatile(dest.add(i), ptr::read_volatile(source.add(i)));
        }
    } else {
        memcpy(dest, source, len);
    }
    dest
}
