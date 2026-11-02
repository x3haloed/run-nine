//! Standard MPW near jump-table segment transitions.
use crate::managers::Heap;
use m68k::AddressBus;
use std::collections::BTreeMap;
pub struct Segments {
    table: u32,
    length: u32,
}
impl Segments {
    pub fn new(table: u32, length: u32) -> Self {
        Self { table, length }
    }
    pub fn load<B: AddressBus>(
        &self,
        id: i16,
        handle: u32,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<(), String> {
        let data = heap.data(handle).map_err(|e| e.to_string())?;
        let size = heap.size(handle).map_err(|e| e.to_string())?;
        if size < 4 {
            return Err("short MPW code resource".into());
        }
        let first = m.read_word(data) as u32;
        let count = m.read_word(data + 2) as u32;
        if first % 8 != 0 || first + count * 8 > self.length || count == 0 {
            return Err(format!("unsupported MPW CODE {id} jump-table header"));
        }
        let mut entries = Vec::new();
        for i in 0..count {
            let e = self.table + first + i * 8;
            if m.read_word(e + 2) != 0x3f3c
                || m.read_word(e + 4) != id as u16
                || m.read_word(e + 6) != 0xa9f0
            {
                return Err(format!(
                    "CODE {id} has an unsupported jump-table entry at {e:#x}"
                ));
            }
            let offset = m.read_word(e) as u32;
            if offset >= size - 4 {
                return Err("code entry outside its resource".into());
            }
            entries.push((e, offset));
        }
        heap.lock(handle, true).map_err(|e| e.to_string())?;
        for (e, offset) in entries {
            m.write_word(e, id as u16);
            m.write_word(e + 2, 0x4ef9);
            m.write_long(e + 4, data + 4 + offset);
        }
        Ok(())
    }
    pub fn unload<B: AddressBus>(
        &self,
        pointer: u32,
        m: &mut B,
        heap: &mut Heap,
        resources: &BTreeMap<([u8; 4], i16), u32>,
    ) -> Result<(), String> {
        let e = pointer.checked_sub(2).ok_or("invalid UnloadSeg pointer")?;
        if e < self.table || e >= self.table + self.length || (e - self.table) % 8 != 0 {
            return Err(format!(
                "UnloadSeg pointer {pointer:#x} is outside the MPW jump table"
            ));
        }
        if m.read_word(e + 2) == 0x3f3c && m.read_word(e + 6) == 0xa9f0 {
            return Ok(());
        }
        if m.read_word(e + 2) != 0x4ef9 {
            return Err("UnloadSeg unsupported jump entry".into());
        }
        let id = m.read_word(e) as i16;
        let h = *resources
            .get(&(*b"CODE", id))
            .ok_or("UnloadSeg has no code resource handle")?;
        let data = heap.data(h).map_err(|e| e.to_string())?;
        let size = heap.size(h).map_err(|e| e.to_string())?;
        if size < 4 {
            return Err("UnloadSeg has a truncated code resource".into());
        }
        let first = m.read_word(data) as u32;
        let count = m.read_word(data + 2) as u32;
        if first % 8 != 0 || first + count * 8 > self.length || count == 0 {
            return Err("UnloadSeg unsupported segment header".into());
        }
        if e < self.table + first || e >= self.table + first + count * 8 {
            return Err("UnloadSeg pointer is outside its segment's jump entries".into());
        }
        let mut entries = Vec::new();
        for i in 0..count {
            let entry = self.table + first + i * 8;
            if m.read_word(entry) != id as u16 || m.read_word(entry + 2) != 0x4ef9 {
                return Err("UnloadSeg inconsistent segment jump entries".into());
            }
            let target = m.read_long(entry + 4);
            let offset = target
                .checked_sub(data + 4)
                .ok_or("UnloadSeg target below code")?;
            if offset >= size - 4 || offset > u16::MAX as u32 {
                return Err("UnloadSeg target outside code".into());
            }
            entries.push((entry, offset));
        }
        heap.lock(h, false).map_err(|e| e.to_string())?;
        for (entry, offset) in entries {
            m.write_word(entry, offset as u16);
            m.write_word(entry + 2, 0x3f3c);
            m.write_word(entry + 4, id as u16);
            m.write_word(entry + 6, 0xa9f0);
        }
        Ok(())
    }
}
