//! Macintosh-facing primitives shared by CPU trap dispatch and probe replay.
use crate::MemoryBus;
use alloc::{collections::BTreeMap, vec::Vec};
const MEM_FULL: i16 = -108;
const NIL_HANDLE: i16 = -109;
#[derive(Clone)]
struct Block {
    ptr: u32,
    size: u32,
    capacity: u32,
    state: u8,
}
pub struct Heap {
    next: u32,
    limit: u32,
    free: BTreeMap<u32, u32>,
    masters: Vec<u32>,
    master_blocks: Vec<(u32, u32)>,
    handles: BTreeMap<u32, Block>,
    pointers: BTreeMap<u32, Block>,
    pub error: i16,
}
impl Heap {
    pub fn free_bytes(&self) -> u32 {
        self.limit - self.next + self.free.values().copied().sum::<u32>()
    }
    pub fn new(first: u32, limit: u32) -> Self {
        Self {
            next: first,
            limit,
            free: BTreeMap::new(),
            masters: Vec::new(),
            master_blocks: Vec::new(),
            handles: BTreeMap::new(),
            pointers: BTreeMap::new(),
            error: 0,
        }
    }
    fn allocate(&mut self, size: u32) -> Result<(u32, u32), i16> {
        if size == 0 {
            return Ok((0, 0));
        }
        let aligned = size.checked_add(3).ok_or(MEM_FULL)? & !3;
        let reusable = self
            .free
            .iter()
            .find(|(_, n)| **n >= aligned)
            .map(|(&p, &n)| (p, n));
        if let Some((p, n)) = reusable {
            self.free.remove(&p);
            if n > aligned {
                self.free.insert(p + aligned, n - aligned);
            }
            return Ok((p, aligned));
        }
        let end = self.next.checked_add(aligned).ok_or(MEM_FULL)?;
        if end > self.limit {
            return Err(MEM_FULL);
        }
        let p = self.next;
        self.next = end;
        Ok((p, aligned))
    }
    fn release(&mut self, mut ptr: u32, mut capacity: u32) {
        if capacity == 0 {
            return;
        }
        if let Some((&p, &n)) = self.free.range(..ptr).next_back() {
            if p + n == ptr {
                self.free.remove(&p);
                ptr = p;
                capacity += n;
            }
        }
        if let Some(n) = self.free.remove(&(ptr + capacity)) {
            capacity += n;
        }
        self.free.insert(ptr, capacity);
    }
    pub fn new_ptr<B: MemoryBus>(
        &mut self,
        memory: &mut B,
        size: u32,
        clear: bool,
    ) -> Result<u32, i16> {
        let result = (|| {
            // A zero-length pointer still has a stable, disposable allocation.
            let (ptr, capacity) = self.allocate(size.max(1))?;
            if clear {
                for i in 0..size {
                    memory.write_byte(ptr + i, 0);
                }
            }
            self.pointers.insert(
                ptr,
                Block {
                    ptr,
                    size,
                    capacity,
                    state: 0x80,
                },
            );
            Ok(ptr)
        })();
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn ptr_size(&mut self, ptr: u32) -> Result<u32, i16> {
        let result = self.pointers.get(&ptr).map(|b| b.size).ok_or(-113);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn dispose_ptr(&mut self, ptr: u32) -> Result<(), i16> {
        let result = self
            .pointers
            .remove(&ptr)
            .map(|b| self.release(b.ptr, b.capacity))
            .ok_or(-113);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn more_masters<B: MemoryBus>(&mut self, memory: &mut B) -> Result<(), i16> {
        let result = self.allocate(64 * 4).map(|(p, n)| {
            self.master_blocks.push((p, n));
            for h in (p..p + n).step_by(4) {
                memory.write_long(h, 0);
                self.masters.push(h);
            }
        });
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn new_handle<B: MemoryBus>(
        &mut self,
        memory: &mut B,
        size: u32,
        clear: bool,
    ) -> Result<u32, i16> {
        let result = (|| {
            if self.masters.is_empty() {
                self.more_masters(memory)?;
            }
            let (ptr, capacity) = self.allocate(size)?;
            let h = self.masters.pop().unwrap();
            if clear {
                for i in 0..size {
                    memory.write_byte(ptr + i, 0);
                }
            }
            memory.write_long(h, ptr);
            self.handles.insert(
                h,
                Block {
                    ptr,
                    size,
                    capacity,
                    state: 0,
                },
            );
            Ok(h)
        })();
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn data(&self, h: u32) -> Result<u32, i16> {
        self.handles.get(&h).map(|b| b.ptr).ok_or(NIL_HANDLE)
    }
    /// Duplicate logical payload into an independent, initially movable handle.
    pub fn duplicate<B: MemoryBus>(&mut self, memory: &mut B, h: u32) -> Result<u32, i16> {
        let result = (|| {
            let size = self.handles.get(&h).ok_or(NIL_HANDLE)?.size;
            let copy = self.new_handle(memory, size, false)?;
            let source = self.data(h)?;
            let dest = self.data(copy)?;
            for i in 0..size {
                let value = memory.read_byte(source + i);
                memory.write_byte(dest + i, value);
            }
            Ok(copy)
        })();
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn size(&mut self, h: u32) -> Result<u32, i16> {
        let result = self.handles.get(&h).map(|b| b.size).ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn lock(&mut self, h: u32, locked: bool) -> Result<(), i16> {
        let result = self
            .handles
            .get_mut(&h)
            .map(|b| b.state = (b.state & !0x80) | if locked { 0x80 } else { 0 })
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn state(&mut self, h: u32) -> Result<u8, i16> {
        let result = self.handles.get(&h).map(|b| b.state).ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn set_state(&mut self, h: u32, state: u8) -> Result<(), i16> {
        let result = self
            .handles
            .get_mut(&h)
            .map(|b| b.state = state & 0xe0)
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn purgeable(&mut self, h: u32, purgeable: bool) -> Result<(), i16> {
        let result = self
            .handles
            .get_mut(&h)
            .map(|b| b.state = (b.state & !0x40) | if purgeable { 0x40 } else { 0 })
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn detach_resource(&mut self, h: u32) -> Result<(), i16> {
        let result = self
            .handles
            .get_mut(&h)
            .map(|b| b.state &= !0x20)
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn resource_state(&mut self, h: u32, purgeable: bool, locked: bool) -> Result<(), i16> {
        let result = self
            .handles
            .get_mut(&h)
            .map(|b| {
                b.state = 0x20 | if purgeable { 0x40 } else { 0 } | if locked { 0x80 } else { 0 }
            })
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    /// Move a relocatable payload upward within its fixed-block partition.
    /// Prefer an existing high hole; if necessary pack intervening unlocked
    /// payloads downward. Master-pointer pools, pointers and locked handles are
    /// fixed barriers, and all handles retain their logical size/state/data.
    pub fn move_high<B: MemoryBus>(&mut self, memory: &mut B, h: u32) -> Result<(), i16> {
        let result = (|| {
            let old = self.handles.get(&h).cloned().ok_or(NIL_HANDLE)?;
            if old.state & 0x80 != 0 {
                return Err(-117);
            }
            if old.capacity == 0 {
                // Zero-byte payloads have no occupied range to relocate. Keep
                // a valid logical endpoint without allocating phantom bytes.
                memory.write_long(h, self.limit);
                self.handles.get_mut(&h).unwrap().ptr = self.limit;
                return Ok(());
            }
            let start = old.ptr;
            let top = self
                .pointers
                .keys()
                .copied()
                .chain(
                    self.handles
                        .values()
                        .filter(|b| b.state & 0x80 != 0 && b.capacity != 0)
                        .map(|b| b.ptr),
                )
                .chain(self.master_blocks.iter().map(|&(p, _)| p))
                .filter(|&p| p > start)
                .min()
                .unwrap_or(self.limit)
                .min(self.limit);
            let mut best = None;
            for (&p, &n) in self.free.iter() {
                let lo = p.max(start + old.capacity);
                let end = (p + n).min(top);
                if end >= old.capacity {
                    let candidate = (end - old.capacity) & !3;
                    if candidate >= lo && best.is_none_or(|(b, _, _)| candidate > b) {
                        best = Some((candidate, p, n));
                    }
                }
            }
            if top >= old.capacity {
                let candidate = (top - old.capacity) & !3;
                if candidate >= self.next
                    && candidate >= start + old.capacity
                    && best.is_none_or(|(b, _, _)| candidate > b)
                {
                    best = Some((candidate, self.next, 0));
                }
            }
            if let Some((ptr, span, n)) = best {
                if n == 0 {
                    if ptr > self.next {
                        self.free.insert(self.next, ptr - self.next);
                    }
                    self.next = ptr + old.capacity;
                } else {
                    self.free.remove(&span);
                    if ptr > span {
                        self.free.insert(span, ptr - span);
                    }
                    if ptr + old.capacity < span + n {
                        self.free
                            .insert(ptr + old.capacity, span + n - ptr - old.capacity);
                    }
                }
                for i in 0..old.size {
                    let b = memory.read_byte(start + i);
                    memory.write_byte(ptr + i, b);
                }
                self.handles.get_mut(&h).unwrap().ptr = ptr;
                memory.write_long(h, ptr);
                self.release(start, old.capacity);
                return Ok(());
            }
            let saved = (0..old.size)
                .map(|i| memory.read_byte(start + i))
                .collect::<Vec<_>>();
            let mut others = self
                .handles
                .iter()
                .filter(|(&key, b)| {
                    key != h && b.capacity != 0 && b.ptr >= start && b.ptr + b.capacity <= top
                })
                .map(|(&key, b)| (key, b.clone()))
                .collect::<Vec<_>>();
            others.sort_by_key(|(_, b)| b.ptr);
            let target = (top - old.capacity) & !3;
            let required = others
                .iter()
                .try_fold(start, |n, (_, b)| n.checked_add(b.capacity))
                .ok_or(MEM_FULL)?;
            if required > target {
                return Err(MEM_FULL);
            }
            if others.iter().any(|(_, b)| b.state & 0x80 != 0) {
                return Err(-117);
            }
            let mut cursor = start;
            for (key, b) in others {
                for i in 0..b.size {
                    let byte = memory.read_byte(b.ptr + i);
                    memory.write_byte(cursor + i, byte);
                }
                self.handles.get_mut(&key).unwrap().ptr = cursor;
                memory.write_long(key, cursor);
                cursor += b.capacity;
            }
            let ranges = self
                .free
                .iter()
                .filter(|(&p, &n)| p < top && p + n > start)
                .map(|(&p, &n)| (p, n))
                .collect::<Vec<_>>();
            for (p, n) in ranges {
                self.free.remove(&p);
                if p < start {
                    self.free.insert(p, start - p);
                }
                if p + n > top {
                    self.free.insert(top, p + n - top);
                }
            }
            if target > cursor {
                self.free.insert(cursor, target - cursor);
            }
            self.next = self.next.max(top);
            for (i, &byte) in saved.iter().enumerate() {
                memory.write_byte(target + i as u32, byte);
            }
            self.handles.get_mut(&h).unwrap().ptr = target;
            memory.write_long(h, target);
            Ok(())
        })();
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn resize<B: MemoryBus>(&mut self, memory: &mut B, h: u32, size: u32) -> Result<(), i16> {
        let result = (|| {
            let old = self.handles.get(&h).cloned().ok_or(NIL_HANDLE)?;
            if size <= old.capacity {
                self.handles.get_mut(&h).unwrap().size = size;
                return Ok(());
            }
            let (ptr, capacity) = if old.state & 0x80 != 0 {
                let capacity = size.checked_add(3).ok_or(MEM_FULL)? & !3;
                let growth = capacity - old.capacity;
                let end = old.ptr + old.capacity;
                if end == self.next
                    && self
                        .next
                        .checked_add(growth)
                        .is_some_and(|v| v <= self.limit)
                {
                    self.next += growth;
                } else if self.free.get(&end).is_some_and(|n| *n >= growth) {
                    let available = self.free.remove(&end).unwrap();
                    if available > growth {
                        self.free.insert(end + growth, available - growth);
                    }
                } else {
                    return Err(MEM_FULL);
                }
                (old.ptr, capacity)
            } else {
                self.allocate(size)?
            };
            if ptr != old.ptr {
                for i in 0..old.size.min(size) {
                    let byte = memory.read_byte(old.ptr + i);
                    memory.write_byte(ptr + i, byte);
                }
                self.release(old.ptr, old.capacity);
            }
            memory.write_long(h, ptr);
            self.handles.insert(
                h,
                Block {
                    ptr,
                    size,
                    capacity,
                    state: old.state,
                },
            );
            Ok(())
        })();
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
    pub fn dispose<B: MemoryBus>(&mut self, memory: &mut B, h: u32) -> Result<(), i16> {
        let result = self
            .handles
            .remove(&h)
            .map(|b| {
                self.release(b.ptr, b.capacity);
                memory.write_long(h, 0);
                self.masters.push(h);
            })
            .ok_or(NIL_HANDLE);
        self.error = result.as_ref().err().copied().unwrap_or(0);
        result
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub top: i16,
    pub left: i16,
    pub bottom: i16,
    pub right: i16,
}
impl Rect {
    pub fn contains(&self, v: i16, h: i16) -> bool {
        v >= self.top && v < self.bottom && h >= self.left && h < self.right
    }
    pub fn intersection(self, other: Self) -> Option<Self> {
        let r = Self {
            top: self.top.max(other.top),
            left: self.left.max(other.left),
            bottom: self.bottom.min(other.bottom),
            right: self.right.min(other.right),
        };
        (r.top < r.bottom && r.left < r.right).then_some(r)
    }
}

#[cfg(test)]
mod relocation_tests {
    use super::*;
    struct Memory(Vec<u8>);
    impl MemoryBus for Memory {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
    }
    fn scene() -> (Heap, Memory) {
        (Heap::new(0x1000, 0x10000), Memory(alloc::vec![0;0x10000]))
    }
    fn filled(heap: &mut Heap, m: &mut Memory, size: u32, seed: u8) -> u32 {
        let h = heap.new_handle(m, size, false).unwrap();
        let p = heap.data(h).unwrap();
        for i in 0..size {
            m.write_byte(p + i, seed.wrapping_add(i as u8));
        }
        h
    }
    fn bytes(heap: &Heap, m: &mut Memory, h: u32) -> Vec<u8> {
        let b = &heap.handles[&h];
        (0..b.size).map(|i| m.read_byte(b.ptr + i)).collect()
    }
    #[test]
    fn high_hole_preserves_payload_other_handles_and_free_space() {
        let (mut heap, mut m) = scene();
        let h = filled(&mut heap, &mut m, 32, 1);
        let other = filled(&mut heap, &mut m, 48, 33);
        let before = bytes(&heap, &mut m, h);
        let other_ptr = heap.data(other).unwrap();
        let free = heap.free_bytes();
        heap.move_high(&mut m, h).unwrap();
        assert_eq!(heap.data(h).unwrap(), 0x10000 - 32);
        assert_eq!(bytes(&heap, &mut m, h), before);
        assert_eq!(heap.data(other).unwrap(), other_ptr);
        assert_eq!(heap.free_bytes(), free);
        assert_eq!(m.read_long(h), heap.data(h).unwrap());
    }
    #[test]
    fn locked_target_errors_and_locked_successor_is_a_barrier() {
        let (mut heap, mut m) = scene();
        let h = filled(&mut heap, &mut m, 32, 1);
        let other = filled(&mut heap, &mut m, 48, 33);
        heap.lock(h, true).unwrap();
        let p = heap.data(h).unwrap();
        assert_eq!(heap.move_high(&mut m, h), Err(-117));
        assert_eq!(heap.error, -117);
        assert_eq!(heap.data(h).unwrap(), p);
        heap.lock(h, false).unwrap();
        heap.lock(other, true).unwrap();
        heap.move_high(&mut m, h).unwrap();
        assert_eq!(heap.data(h).unwrap(), p);
        assert_eq!(heap.move_high(&mut m, 0), Err(-109));
    }
    #[test]
    fn pointer_partition_compacts_movable_payload_without_crossing_barrier() {
        let (mut heap, mut m) = scene();
        let h = filled(&mut heap, &mut m, 32, 1);
        let other = filled(&mut heap, &mut m, 48, 33);
        let barrier = heap.new_ptr(&mut m, 16, true).unwrap();
        let before = heap.data(h).unwrap();
        let a = bytes(&heap, &mut m, h);
        let b = bytes(&heap, &mut m, other);
        let free = heap.free_bytes();
        heap.move_high(&mut m, h).unwrap();
        assert_eq!(heap.data(h).unwrap() + 32, barrier);
        assert_eq!(heap.data(other).unwrap(), before);
        assert_eq!(bytes(&heap, &mut m, h), a);
        assert_eq!(bytes(&heap, &mut m, other), b);
        assert_eq!(heap.free_bytes(), free);
        assert_eq!(&m.0[barrier as usize..barrier as usize + 16], &[0; 16]);
    }
    #[test]
    fn master_pointer_pool_is_fixed_and_empty_payload_remains_disposable() {
        let (mut heap, mut m) = scene();
        let h = filled(&mut heap, &mut m, 32, 1);
        let before = heap.data(h).unwrap();
        heap.more_masters(&mut m).unwrap();
        let pool = heap.master_blocks.last().copied().unwrap();
        let snapshot = m.0[pool.0 as usize..(pool.0 + pool.1) as usize].to_vec();
        heap.move_high(&mut m, h).unwrap();
        assert_eq!(heap.data(h).unwrap(), before);
        assert_eq!(
            &m.0[pool.0 as usize..(pool.0 + pool.1) as usize],
            snapshot.as_slice()
        );
        let empty = heap.new_handle(&mut m, 0, false).unwrap();
        let free = heap.free_bytes();
        heap.move_high(&mut m, empty).unwrap();
        assert_eq!(heap.size(empty), Ok(0));
        assert_ne!(heap.data(empty).unwrap(), 0);
        heap.dispose(&mut m, empty).unwrap();
        assert_eq!(heap.free_bytes(), free);
    }
}

#[cfg(test)]
mod duplicate_tests {
    use super::*;
    struct M(alloc::vec::Vec<u8>);
    impl MemoryBus for M {
        fn read_byte(&mut self, p: u32) -> u8 {
            self.0[p as usize]
        }
        fn write_byte(&mut self, p: u32, v: u8) {
            self.0[p as usize] = v;
        }
    }
    #[test]
    fn clone_outlives_locked_purgeable_source_and_moves_independently() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut heap = Heap::new(0x2000, 0xf000);
        let h = heap.new_handle(&mut m, 17, true).unwrap();
        let p = heap.data(h).unwrap();
        for i in 0..17 {
            m.write_byte(p + i, (i * 37 + 19) as u8);
        }
        heap.set_state(h, 192).unwrap();
        let copy = heap.duplicate(&mut m, h).unwrap();
        assert_ne!(h, copy);
        assert_eq!(heap.state(h), Ok(192));
        assert_eq!(heap.state(copy), Ok(0));
        heap.dispose(&mut m, h).unwrap();
        heap.move_high(&mut m, copy).unwrap();
        let dest = heap.data(copy).unwrap();
        for i in 0..17 {
            assert_eq!(m.read_byte(dest + i), (i * 37 + 19) as u8);
        }
    }
    #[test]
    fn failed_clone_preserves_source_and_free_space() {
        let mut m = M(alloc::vec![0;0x10000]);
        let mut heap = Heap::new(0x2000, 0x2150);
        let h = heap.new_handle(&mut m, 64, true).unwrap();
        let p = heap.data(h).unwrap();
        m.write_byte(p, 0x75);
        let free = heap.free_bytes();
        assert_eq!(heap.duplicate(&mut m, h), Err(-108));
        assert_eq!(heap.error, -108);
        assert_eq!(heap.free_bytes(), free);
        assert_eq!(m.read_byte(p), 0x75);
        assert_eq!(heap.duplicate(&mut m, 0), Err(-109));
        assert_eq!(heap.error, -109);
        assert_eq!(heap.size(h), Ok(64));
    }
}
