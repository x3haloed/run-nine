//! Classic application-local TextEdit scrap and in-memory system Scrap Manager.
//! The two stores are distinct; applications explicitly move TEXT between them.
use crate::managers::Heap;
use m68k::{AddressBus, CpuCore};

pub fn init_te<B: AddressBus>(m: &mut B, heap: &mut Heap) -> Result<u32, String> {
    let mut h = m.read_long(0xab4);
    if h == 0 {
        h = heap.new_handle(m, 0, true).map_err(|e| e.to_string())?;
        m.write_long(0xab4, h);
    }
    Ok(h)
}
pub fn init<B: AddressBus>(m: &mut B, heap: &mut Heap) -> Result<u32, String> {
    let mut h = m.read_long(0x964);
    if h == 0 {
        h = heap.new_handle(m, 0, true).map_err(|e| e.to_string())?;
        m.write_long(0x964, h);
        m.write_word(0x96a, 1);
        let name = b"\x0eClipboard File";
        let p = heap
            .new_ptr(m, name.len() as u32, false)
            .map_err(|e| e.to_string())?;
        for (i, b) in name.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        m.write_long(0x96c, p);
    }
    Ok(h)
}
pub fn zero<B: AddressBus>(m: &mut B, heap: &mut Heap) -> Result<(), String> {
    let h = init(m, heap)?;
    heap.resize(m, h, 0).map_err(|e| e.to_string())?;
    m.write_long(0x960, 0);
    // OS9's observable generation advances in units of eight. PutScrap does
    // not change it. The pinned native probe checks changes, not absolute IDs.
    let generation = m.read_word(0x968).wrapping_add(8);
    m.write_word(0x968, generation);
    Ok(())
}
pub fn put<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    kind: u32,
    bytes: &[u8],
) -> Result<(), String> {
    let h = init(m, heap)?;
    let size = m.read_long(0x960);
    let len = u32::try_from(bytes.len()).map_err(|_| "scrap length overflow")?;
    let end = size
        .checked_add(8)
        .and_then(|v| v.checked_add(len))
        .ok_or("scrap size overflow")?;
    let padded = end.checked_add(end & 1).ok_or("scrap padding overflow")?;
    if padded > 32 * 1024 * 1024 {
        return Err("system scrap exceeds supported capacity".into());
    }
    heap.resize(m, h, padded).map_err(|e| e.to_string())?;
    let p = heap.data(h).map_err(|e| e.to_string())?;
    m.write_long(p + size, kind);
    m.write_long(p + size + 4, len);
    for (i, b) in bytes.iter().enumerate() {
        m.write_byte(p + size + 8 + i as u32, *b);
    }
    if end != padded {
        m.write_byte(p + end, 0);
    }
    m.write_long(0x960, padded);
    Ok(())
}
pub fn get<B: AddressBus>(
    m: &mut B,
    heap: &mut Heap,
    kind: u32,
    destination: u32,
    offset: u32,
) -> Result<i32, String> {
    let h = init(m, heap)?;
    let size = m.read_long(0x960);
    if size > heap.size(h).map_err(|e| e.to_string())? {
        return Err("scrap size exceeds handle".into());
    }
    let p = heap.data(h).map_err(|e| e.to_string())?;
    let mut position = 0;
    while position + 8 <= size {
        let entry = m.read_long(p + position);
        let len = m.read_long(p + position + 4);
        let end = position
            .checked_add(8)
            .and_then(|v| v.checked_add(len))
            .ok_or("scrap entry overflow")?;
        if end > size {
            return Err("truncated scrap entry".into());
        }
        if entry == kind {
            // Save source before resizing a possibly identical destination.
            let data: Vec<u8> = (0..len)
                .map(|i| m.read_byte(p + position + 8 + i))
                .collect();
            if destination != 0 {
                heap.resize(m, destination, len)
                    .map_err(|e| e.to_string())?;
                let d = heap.data(destination).map_err(|e| e.to_string())?;
                for (i, b) in data.iter().enumerate() {
                    m.write_byte(d + i as u32, *b);
                }
            }
            if offset != 0 {
                m.write_long(offset, position + 8);
            }
            return Ok(len as i32);
        }
        position = end + (end & 1);
    }
    if offset != 0 {
        m.write_long(offset, 0x40000000);
    }
    Ok(-102)
}
pub fn dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut CpuCore,
    m: &mut B,
    heap: &mut Heap,
) -> Result<bool, String> {
    let sp = cpu.dar[15];
    let pop = match op {
        0xa9cc => {
            init_te(m, heap)?;
            m.write_word(0xab0, 0);
            0
        }
        0xa9f9 => {
            init(m, heap)?;
            m.write_long(sp, 0x960);
            0
        }
        0xa9fc => {
            zero(m, heap)?;
            m.write_long(sp, 0);
            0
        }
        0xa9fe => {
            let p = m.read_long(sp);
            let kind = m.read_long(sp + 4);
            let len = m.read_long(sp + 8);
            if len > 32 * 1024 * 1024 {
                return Err("unsupported PutScrap source length".into());
            }
            let data: Vec<u8> = (0..len).map(|i| m.read_byte(p + i)).collect();
            put(m, heap, kind, &data)?;
            m.write_long(sp + 12, 0);
            12
        }
        0xa9fd => {
            let offset = m.read_long(sp);
            let kind = m.read_long(sp + 4);
            let destination = m.read_long(sp + 8);
            let result = get(m, heap, kind, destination, offset)?;
            m.write_long(sp + 12, result as u32);
            12
        }
        _ => return Ok(false),
    };
    cpu.dar[15] += pop;
    cpu.pc += 2;
    Ok(true)
}
