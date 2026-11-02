//! Owned bitmap PICT playback through the production DrawPicture entry point.
use crate::{
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{read_rect, write_rect, QuickDraw},
    regions::Region,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const PORT: u32 = 0x1c000;
const RECT: u32 = 0x19000;
const STACK: u32 = 0x700000;
const BOUNDS: Rect = Rect {
    top: 0,
    left: 0,
    bottom: 24,
    right: 64,
};
const INNER: Rect = Rect {
    top: 4,
    left: 6,
    bottom: 13,
    right: 28,
};
const DESTS: [Rect; 5] = [
    Rect {
        top: 2,
        left: 3,
        bottom: 10,
        right: 19,
    },
    Rect {
        top: 1,
        left: 2,
        bottom: 17,
        right: 34,
    },
    Rect {
        top: 3,
        left: 4,
        bottom: 7,
        right: 12,
    },
    Rect {
        top: -2,
        left: -3,
        bottom: 6,
        right: 13,
    },
    Rect {
        top: 4,
        left: 4,
        bottom: 4,
        right: 20,
    },
];
fn hex(m: &mut Memory, p: u32, n: usize) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i as u32)))
        .collect()
}
fn scene() -> Result<(Memory, Heap, QuickDraw, u32, u32), String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.color_capable = true;
    q.init(&mut m, 0x100000)?;
    q.device(&mut m, &mut heap)?;
    q.set_depth(&mut m, &mut heap, 8)
        .map_err(|e| e.to_string())?;
    q.open_color_port(&mut m, &mut heap, PORT)?;
    q.set_port(&mut m, PORT)?;
    let bitmap = q.port_bitmap(&mut m, PORT)?;
    let pixels = heap
        .new_ptr(&mut m, 1536, false)
        .map_err(|e| e.to_string())?;
    m.write_long(bitmap, pixels);
    m.write_word(bitmap + 4, 0x8040);
    write_rect(&mut m, bitmap + 6, BOUNDS);
    write_rect(&mut m, PORT + 16, BOUNDS);
    let vis = m.read_long(PORT + 24);
    let clip = m.read_long(PORT + 28);
    Region::rectangle(BOUNDS).store(&mut m, &mut heap, vis)?;
    Region::rectangle(BOUNDS).store(&mut m, &mut heap, clip)?;
    Ok((m, heap, q, bitmap, pixels))
}
pub fn probe() -> Result<String, String> {
    let (mut m, mut heap, q, bitmap, pixels) = scene()?;
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    let table = m.read_long(bitmap + 42);
    let tabledata = heap.data(table).map_err(|e| e.to_string())?;
    let table_size = heap.size(table).map_err(|e| e.to_string())? as usize;
    let destination_table_hex = hex(&mut m, tabledata, table_size);
    let pictures: [&[u8]; 4] = [
        include_bytes!("../../probes/pictbitmap/pict-0.bin"),
        include_bytes!("../../probes/pictbitmap/pict-1.bin"),
        include_bytes!("../../probes/pictbitmap/pict-2.bin"),
        include_bytes!("../../probes/pictbitmap/pict-3.bin"),
    ];
    for (kind, bytes) in pictures.into_iter().enumerate() {
        let picture = heap
            .new_handle(&mut m, bytes.len() as u32, false)
            .map_err(|e| e.to_string())?;
        let p = heap.data(picture).map_err(|e| e.to_string())?;
        m.put(p as usize, bytes)?;
        heap.lock(picture, true).map_err(|e| e.to_string())?;
        for (which, dest) in DESTS.into_iter().enumerate() {
            for clip in 0..2 {
                for ink in 0..2 {
                    for seed in [0x12345678u32, 0] {
                        for flags in [16u8, 31, 0] {
                            m.put(pixels as usize, &[0x5a; 1536])?;
                            write_rect(&mut m, RECT, dest);
                            let ch = m.read_long(PORT + 28);
                            Region::rectangle(if clip == 0 { BOUNDS } else { INNER })
                                .store(&mut m, &mut heap, ch)?;
                            for (i, rgb) in [
                                if ink == 0 { [0; 3] } else { [65535, 0, 0] },
                                if ink == 0 { [65535; 3] } else { [0, 65535, 0] },
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                for (c, v) in rgb.into_iter().enumerate() {
                                    m.write_word(PORT + 36 + i as u32 * 6 + c as u32 * 2, v);
                                }
                            }
                            let before = hex(&mut m, PORT, 108);
                            let cp = heap.data(ch).map_err(|e| e.to_string())?;
                            let clip_before = hex(&mut m, cp, 10);
                            m.write_long(STACK, 0x55667788);
                            m.write_long(STACK - 4, picture);
                            m.write_long(STACK - 8, RECT);
                            cpu.pc = 0x6000;
                            cpu.dar[15] = STACK - 8;
                            cpu.dar[0] = seed;
                            cpu.set_ccr(flags);
                            m.write_word(0x220, (-123i16) as u16);
                            m.write_word(0xa60, (-192i16) as u16);
                            assert!(crate::pictures::dispatch(
                                0xa8f6, &mut cpu, &mut m, &mut heap, &q
                            )?);
                            assert_eq!(cpu.pc, 0x6002);
                            let mem_error = m.read_word(0x220) as i16;
                            let res_error = m.read_word(0xa60) as i16;
                            let r = read_rect(&mut m, RECT);
                            records.push(json!({"kind":kind,"destination":which,"clip":clip,"ink":ink,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(STACK),"guard":m.read_long(STACK),"mem_error":mem_error,"res_error":res_error,"picture_state":heap.state(picture).map_err(|e|e.to_string())?,"picture_preserved":u8::from((0..bytes.len()).all(|i|m.read_byte(p+i as u32)==bytes[i])),"port_preserved":u8::from(hex(&mut m,PORT,108)==before),"clip_preserved":u8::from(hex(&mut m,cp,10)==clip_before),"dest_rect":[r.top,r.left,r.bottom,r.right],"pixels_hex":hex(&mut m,pixels,1536)}));
                        }
                    }
                }
            }
        }
        heap.dispose(&mut m, picture).map_err(|e| e.to_string())?;
    }
    Ok(json!({"schema":1,"probe":"pict-bitmap-runtime-raw-v1","depth":8,"row_bytes":64,"destination_table_hex":destination_table_hex,"records":records}).to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unsupported_picture_after_valid_bitmap_leaves_guest_state_and_pixels_untouched() {
        let (mut m, mut heap, q, _, pixels) = scene().unwrap();
        m.put(pixels as usize, &[0x5a; 1536]).unwrap();
        let original = include_bytes!("../../probes/pictbitmap/pict-0.bin");
        let mut picture = original[..original.len() - 2].to_vec();
        picture.extend_from_slice(&[0, 0x31, 0, 0, 0, 0, 0, 8, 0, 16, 0, 255]);
        let h = heap
            .new_handle(&mut m, picture.len() as u32, false)
            .unwrap();
        let p = heap.data(h).unwrap();
        m.put(p as usize, &picture).unwrap();
        heap.lock(h, true).unwrap();
        write_rect(&mut m, RECT, DESTS[0]);
        m.write_long(STACK - 4, h);
        m.write_long(STACK - 8, RECT);
        m.write_long(STACK, 0x55667788);
        m.write_word(0x220, (-123i16) as u16);
        m.write_word(0xa60, (-192i16) as u16);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x6000;
        cpu.dar[15] = STACK - 8;
        cpu.dar[0] = 0x12345678;
        cpu.set_ccr(31);
        let registers = cpu.dar;
        assert!(
            crate::pictures::dispatch(0xa8f6, &mut cpu, &mut m, &mut heap, &q)
                .unwrap_err()
                .contains("unsupported PICT opcode")
        );
        assert_eq!(cpu.dar, registers);
        assert_eq!(cpu.pc, 0x6000);
        assert_eq!(cpu.get_sr() & 31, 31);
        assert_eq!(m.read_word(0x220) as i16, -123);
        assert_eq!(m.read_word(0xa60) as i16, -192);
        assert_eq!(m.read_long(STACK), 0x55667788);
        assert!((0..1536).all(|i| m.read_byte(pixels + i) == 0x5a));
        assert_eq!(heap.state(h).unwrap(), 128);
    }
}
