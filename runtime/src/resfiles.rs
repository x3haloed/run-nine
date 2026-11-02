//! Stored resource maps. Classic state and file bytes; host storage stays in Files.
use crate::{
    files::{Files, Fork},
    managers::Heap,
    resource::{self, Data, Resource},
};
use m68k::{AddressBus, CpuCore};
use std::collections::BTreeMap;
struct Entry {
    data: Data,
    handle: u32,
}
struct Map {
    reference: i16,
    volume: i16,
    name: Vec<u8>,
    permission: u8,
    handle: u32,
    entries: Vec<Entry>,
}
#[derive(Default)]
pub struct ResFiles {
    maps: Vec<Map>,
    // Application color-table seeds are unique within this runtime. The native
    // initial counter depends on prior system activity and is not reproduced.
    color_seeds: std::rc::Rc<classic_core::color_seed::ColorSeeds>,
}
impl ResFiles {
    pub fn with_color_seeds(
        color_seeds: std::rc::Rc<classic_core::color_seed::ColorSeeds>,
    ) -> Self {
        Self {
            maps: Vec::new(),
            color_seeds,
        }
    }

    fn locate(&self, h: u32) -> Option<(usize, usize)> {
        self.maps.iter().enumerate().find_map(|(i, map)| {
            map.entries
                .iter()
                .position(|e| e.handle != 0 && e.handle == h)
                .map(|j| (i, j))
        })
    }
    fn current<B: AddressBus>(&self, m: &mut B) -> Option<usize> {
        let r = m.read_word(0xa5a) as i16;
        self.maps.iter().position(|map| map.reference == r)
    }
    pub fn application_visible<B: AddressBus>(&self, m: &mut B) -> bool {
        let current = m.read_word(0xa5a) as i16;
        current == 2 || self.maps.iter().any(|map| map.reference == current)
    }
    fn sync<B: AddressBus>(
        &mut self,
        index: usize,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<(), String> {
        let map = &self.maps[index];
        let data: Vec<_> = map.entries.iter().map(|e| e.data.clone()).collect();
        let bytes = resource::serialize(&data)?;
        let parsed = resource::parse(&bytes)?;
        let start = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as usize;
        let mut raw = bytes[start..].to_vec();
        let old = heap.data(map.handle).map_err(|e| e.to_string())?;
        let next = m.read_long(old + 16);
        raw[16..20].copy_from_slice(&next.to_be_bytes());
        raw[20..22].copy_from_slice(&map.reference.to_be_bytes());
        for r in &parsed {
            let h = map
                .entries
                .iter()
                .find(|e| e.data.kind == r.kind && e.data.id == r.id)
                .ok_or("resource map entry lost")?
                .handle;
            raw[r.reference_offset + 8..r.reference_offset + 12].copy_from_slice(&h.to_be_bytes());
        }
        heap.resize(m, map.handle, raw.len() as u32)
            .map_err(|e| e.to_string())?;
        let p = heap.data(map.handle).map_err(|e| e.to_string())?;
        for (i, b) in raw.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        Ok(())
    }
    fn load<B: AddressBus>(
        &mut self,
        i: usize,
        j: usize,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<u32, String> {
        if self.maps[i].entries[j].handle == 0 {
            let e = &self.maps[i].entries[j];
            let decoded =
                crate::vendor::compressed::decompress_if_needed(e.data.attributes, &e.data.bytes)
                    .map_err(|e| format!("resource decompression: {e:?}"))?;
            let bytes = decoded.as_deref().unwrap_or(&e.data.bytes);
            let h = heap
                .new_handle(m, bytes.len() as u32, false)
                .map_err(|e| e.to_string())?;
            let p = heap.data(h).map_err(|e| e.to_string())?;
            for (k, b) in bytes.iter().enumerate() {
                m.write_byte(p + k as u32, *b);
            }
            heap.resource_state(
                h,
                e.data.attributes & 0x20 != 0,
                e.data.attributes & 0x10 != 0,
            )
            .map_err(|e| e.to_string())?;
            self.maps[i].entries[j].handle = h;
            self.sync(i, m, heap)?;
        }
        Ok(self.maps[i].entries[j].handle)
    }
    fn open<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        files: &mut Files,
        volume: i16,
        parent: u32,
        name: Vec<u8>,
        permission: u8,
    ) -> Result<Result<i16, i16>, String> {
        if ![0, 1, 3].contains(&permission) {
            return Err("resource-file sharing permission is not implemented".into());
        }
        let storage = match files.resource_volume(volume) {
            Ok(v) => v,
            Err(e) => return Ok(Err(e)),
        };
        let volume = storage.resource_volume_id();
        if ![0, 2].contains(&parent) {
            return Ok(Err(-120));
        }
        if permission != 1 {
            if let Some(map) = self.maps.iter().find(|map| {
                map.volume == volume && map.name.eq_ignore_ascii_case(&name) && map.permission != 1
            }) {
                m.write_word(0xa5a, map.reference as u16);
                return Ok(Ok(map.reference));
            }
        }
        let bytes = match storage.fork_bytes(&name, Fork::Resource) {
            Ok(b) => b.to_vec(),
            Err(e) => return Ok(Err(e)),
        };
        if bytes.len() < 16 {
            return Ok(Err(-39));
        }
        let parsed = match resource::parse(&bytes) {
            Ok(p) => p,
            Err(_) => return Ok(Err(-199)),
        };
        let entries = parsed
            .iter()
            .map(|r| Entry {
                data: Data {
                    kind: r.kind,
                    id: r.id,
                    attributes: r.attributes,
                    name: r.name.map(|n| n.to_vec()),
                    bytes: r.data.to_vec(),
                },
                handle: 0,
            })
            .collect();
        let reference = match storage.open(&name, Fork::Resource, permission) {
            Ok(r) => r,
            Err(e) => return Ok(Err(e)),
        };
        let h = heap.new_handle(m, 30, true).map_err(|e| e.to_string())?;
        let p = heap.data(h).map_err(|e| e.to_string())?;
        let top = m.read_long(0xa50);
        m.write_long(p + 16, top);
        self.maps.push(Map {
            reference,
            volume,
            name,
            permission,
            handle: h,
            entries,
        });
        self.sync(self.maps.len() - 1, m, heap)?;
        m.write_long(0xa50, h);
        m.write_word(0xa5a, reference as u16);
        Ok(Ok(reference))
    }
    fn persist<B: AddressBus>(
        &mut self,
        i: usize,
        only: Option<u32>,
        m: &mut B,
        heap: &mut Heap,
        files: &mut Files,
    ) -> Result<i16, String> {
        if self.maps[i].permission == 1 {
            return Err("writing a read-only resource map is not implemented".into());
        }
        if let Some(h) = only {
            if self.maps[i]
                .entries
                .iter()
                .any(|e| e.handle != h && e.data.attributes & 2 != 0)
            {
                return Err(
                    "WriteResource with other pending dirty resources is not implemented".into(),
                );
            }
        }
        let mut entries: Vec<_> = self.maps[i]
            .entries
            .iter()
            .map(|e| e.data.clone())
            .collect();
        for (data, entry) in entries.iter_mut().zip(&self.maps[i].entries) {
            if data.attributes & 2 != 0 && only.is_none_or(|h| h == entry.handle) {
                let n = heap.size(entry.handle).map_err(|e| e.to_string())?;
                let p = heap.data(entry.handle).map_err(|e| e.to_string())?;
                data.bytes = (0..n).map(|k| m.read_byte(p + k)).collect();
                data.attributes &= !3;
            }
        }
        let bytes = resource::serialize(&entries)?;
        let map = &self.maps[i];
        let result = (|| -> Result<(), i16> {
            let storage = files.resource_volume(map.volume)?;
            storage.seek(map.reference, 1, 0)?;
            storage.write(map.reference, &bytes)?;
            storage.set_eof(map.reference, bytes.len())?;
            Ok(())
        })();
        if let Err(e) = result {
            return Ok(e);
        }
        for (e, data) in self.maps[i].entries.iter_mut().zip(entries) {
            e.data = data;
        }
        self.sync(i, m, heap)?;
        Ok(0)
    }
    fn close<B: AddressBus>(
        &mut self,
        i: usize,
        m: &mut B,
        heap: &mut Heap,
        files: &mut Files,
    ) -> Result<i16, String> {
        if self.maps[i].permission != 1
            && self.maps[i]
                .entries
                .iter()
                .any(|e| e.data.attributes & 2 != 0)
        {
            let error = self.persist(i, None, m, heap, files)?;
            if error != 0 {
                return Ok(error);
            }
        }
        let map = &self.maps[i];
        if let Err(error) = files
            .resource_volume(map.volume)
            .and_then(|f| f.close(map.reference))
        {
            return Ok(error);
        }
        let map = self.maps.remove(i);
        let p = heap.data(map.handle).map_err(|e| e.to_string())?;
        let next = m.read_long(p + 16);
        let mut h = m.read_long(0xa50);
        if h == map.handle {
            m.write_long(0xa50, next)
        } else {
            let mut found = false;
            for _ in 0..1024 {
                if h == 0 {
                    break;
                }
                let p = m.read_long(h);
                let n = m.read_long(p + 16);
                if n == map.handle {
                    m.write_long(p + 16, next);
                    found = true;
                    break;
                }
                h = n;
            }
            if !found {
                return Err("closing resource map is absent from map chain".into());
            }
        }
        if m.read_word(0xa5a) as i16 == map.reference {
            let top = m.read_long(0xa50);
            let p = m.read_long(top);
            let reference = m.read_word(p + 20);
            m.write_word(0xa5a, reference);
        }
        for e in map.entries {
            if e.handle != 0 {
                heap.dispose(m, e.handle).map_err(|e| e.to_string())?;
            }
        }
        heap.dispose(m, map.handle).map_err(|e| e.to_string())?;
        Ok(0)
    }
    fn relinquish<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        app: &[Resource<'_>],
        cache: &mut BTreeMap<([u8; 4], i16), u32>,
        h: u32,
        detach: bool,
    ) -> Result<i16, String> {
        if let Some((i, j)) = self.locate(h) {
            if !detach && self.maps[i].entries[j].data.attributes & 2 != 0 {
                return Err("ReleaseResource of changed data is not implemented".into());
            }
            if detach {
                heap.detach_resource(h).map_err(|e| e.to_string())?;
            } else {
                heap.dispose(m, h).map_err(|e| e.to_string())?;
            }
            self.maps[i].entries[j].handle = 0;
            self.sync(i, m, heap)?;
            return Ok(0);
        }
        let key = cache
            .iter()
            .find(|(_, value)| **value == h)
            .map(|(key, _)| *key);
        let error = if let Some(key) = key {
            let resource = app
                .iter()
                .find(|r| (r.kind, r.id) == key)
                .ok_or("application resource cache is missing its source metadata")?;
            let mut map_handle = m.read_long(0xa50);
            let mut entry = None;
            for _ in 0..1024 {
                if map_handle == 0 {
                    break;
                }
                let map = m.read_long(map_handle);
                if m.read_word(map + 20) == 2 {
                    entry = Some(map + resource.reference_offset as u32 + 8);
                    break;
                }
                map_handle = m.read_long(map + 16);
            }
            let entry = entry.ok_or("application resource map is not linked")?;
            if !detach {
                heap.dispose(m, h).map_err(|e| e.to_string())?;
            } else {
                heap.detach_resource(h).map_err(|e| e.to_string())?;
            }
            cache.remove(&key);
            m.write_long(entry, 0);
            0i16
        } else {
            -192
        };
        Ok(error)
    }
    fn get<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        kind: [u8; 4],
        id: i16,
        one: bool,
        app: &[Resource<'_>],
        cache: &mut BTreeMap<([u8; 4], i16), u32>,
        load_app: &mut impl FnMut(
            &Resource<'_>,
            &mut B,
            &mut Heap,
            &mut BTreeMap<([u8; 4], i16), u32>,
        ) -> Result<u32, String>,
    ) -> Result<u32, String> {
        let current = self.current(m);
        if let Some(i) = current {
            for index in (0..=i).rev() {
                if one && index != i {
                    break;
                }
                if let Some(j) = self.maps[index]
                    .entries
                    .iter()
                    .position(|e| e.data.kind == kind && e.data.id == id)
                {
                    return self.load(index, j, m, heap);
                }
            }
        }
        if (current.is_some() && !one) || m.read_word(0xa5a) == 2 {
            if let Some(r) = app.iter().find(|r| r.kind == kind && r.id == id) {
                return load_app(r, m, heap, cache);
            }
        }
        Ok(0)
    }
    #[allow(clippy::too_many_arguments)]
    pub fn dispatch<B: AddressBus>(
        &mut self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &mut Heap,
        files: &mut Files,
        app: &[Resource<'_>],
        cache: &mut BTreeMap<([u8; 4], i16), u32>,
        mut load_app: impl FnMut(
            &Resource<'_>,
            &mut B,
            &mut Heap,
            &mut BTreeMap<([u8; 4], i16), u32>,
        ) -> Result<u32, String>,
    ) -> Result<bool, String> {
        let sp = cpu.dar[15];
        if matches!(op, 0xa99d | 0xa80e) {
            let index = m.read_word(sp) as i16;
            let kind = m.read_long(sp + 2).to_be_bytes();
            let one = op == 0xa80e;
            let current = self.current(m);
            if one && current.is_none() && !matches!(m.read_word(0xa5a), 0 | 2) {
                return Err("Get1IndResource current resource map is not owned".into());
            }
            let mut h = 0;
            if index > 0 {
                m.write_word(0x220, 0);
                let mut remaining = index as usize - 1;
                let maps: Vec<usize> = if one {
                    current.into_iter().collect()
                } else {
                    (0..self.maps.len()).rev().collect()
                };
                for i in maps {
                    for j in 0..self.maps[i].entries.len() {
                        if self.maps[i].entries[j].data.kind == kind {
                            if remaining == 0 {
                                h = self.load(i, j, m, heap)?;
                                break;
                            }
                            remaining -= 1;
                        }
                    }
                    if h != 0 { break; }
                }
                if h == 0 && (!one || m.read_word(0xa5a) == 2) {
                    if let Some(r) = app.iter().filter(|r| r.kind == kind).nth(remaining) {
                        h = load_app(r, m, heap, cache)?;
                    }
                }
            }
            let error = if h == 0 { -192i16 } else { 0 };
            if kind == *b"snd " && std::env::args().any(|a| a == "--trace-sound") {
                println!("resource sound indexed lookup: trap={op:#06x} current={} index={index} handle={h:#x} error={error}", m.read_word(0xa5a));
            }
            m.write_long(sp + 6, h);
            m.write_word(0xa60, error as u16);
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr(0);
            cpu.dar[15] += 6;
            cpu.pc += 2;
            return Ok(true);
        }
        if matches!(op, 0xa99c | 0xa80d) {
            let kind = m.read_long(sp).to_be_bytes();
            let count = if op == 0xa99c {
                // CountResources starts at the chain head, independent of
                // CurResFile. Duplicate IDs in distinct maps each count.
                app.iter().filter(|r| r.kind == kind).count()
                    + self.maps.iter().map(|map| map.entries.iter().filter(|e| e.data.kind == kind).count()).sum::<usize>()
            } else if m.read_word(0xa5a) == 2 {
                app.iter().filter(|r| r.kind == kind).count()
            } else if m.read_word(0xa5a) == 0 {
                0 // This runtime's System resource map is independently empty.
            } else if let Some(i) = self.current(m) {
                self.maps[i].entries.iter().filter(|e| e.data.kind == kind).count()
            } else {
                return Err("Count1Resources current resource map is not owned".into());
            };
            let count = i16::try_from(count).map_err(|_| "resource count exceeds signed word profile")?;
            if kind==*b"snd "&&std::env::args().any(|a|a=="--trace-sound") {println!("resource sound count: trap={op:#06x} current={} count={count}",m.read_word(0xa5a));}
            m.write_word(sp + 4, count as u16);
            m.write_word(0x220, 0);
            m.write_word(0xa60, 0);
            cpu.dar[0] = 0;
            cpu.set_ccr(0);
            cpu.dar[15] += 4;
            cpu.pc += 2;
            return Ok(true);
        }
        if matches!(op, 0xa9a3 | 0xa992) {
            let h = m.read_long(sp);
            let error = self.relinquish(m, heap, app, cache, h, op == 0xa992)?;
            m.write_word(0xa60, error as u16);
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr(0);
            cpu.dar[15] += 4;
            cpu.pc += 2;
            return Ok(true);
        }
        if op == 0xaa18 {
            let id = m.read_word(sp) as i16;
            let mut h = self.get(m, heap, *b"clut", id, false, app, cache, &mut load_app)?;
            if h != 0 {
                if heap.size(h).map_err(|e| e.to_string())? < 8 {
                    return Err("GetCTable short resource outside measured profile".into());
                }
                let error = self.relinquish(m, heap, app, cache, h, true)?;
                if error != 0 {
                    return Err("GetCTable lost resource ownership".into());
                }
                let p = heap.data(h).map_err(|e| e.to_string())?;
                m.write_long(p, self.color_seeds.next());
            } else if let Some(bytes) = system_color_table(id) {
                h = heap
                    .new_handle(m, bytes.len() as u32, false)
                    .map_err(|e| e.to_string())?;
                let p = heap.data(h).map_err(|e| e.to_string())?;
                for (i, b) in bytes.iter().enumerate() {
                    m.write_byte(p + i as u32, *b);
                }
            }
            m.write_long(sp + 2, h);
            m.write_word(0xa60, 0);
            m.write_word(0x220, 0);
            cpu.dar[0] = 0;
            cpu.set_ccr(if h == 0 { 0 } else { 4 });
            cpu.dar[15] += 2;
            cpu.pc += 2;
            return Ok(true);
        }
        if matches!(op, 0xa820 | 0xa9a1) && self.current(m).is_some() {
            return Err("named lookup from stored resource maps is not implemented".into());
        }
        let mut error = 0i16;
        let pop;
        if op == 0xa9c4 || op == 0xa81a || (op == 0xaa52 && cpu.dar[0] as u16 == 13) {
            let (volume, parent, name, permission, size) = if op == 0xaa52 {
                let spec = m.read_long(sp + 2);
                let volume = m.read_word(spec) as i16;
                let parent = m.read_long(spec + 2);
                let n = m.read_byte(spec + 6) as u32;
                let name:Vec<u8> = (0..n).map(|k| m.read_byte(spec + 7 + k)).collect();
                (volume, parent, name, m.read_byte(sp), 6)
            } else {
                let p = m.read_long(sp + if op == 0xa81a { 2 } else { 4 });
                let n = m.read_byte(p) as u32;
                let name:Vec<u8> = (0..n).map(|k| m.read_byte(p + 1 + k)).collect();
                (
                    m.read_word(sp + if op == 0xa81a { 10 } else { 2 }) as i16,
                    if op == 0xa81a { m.read_long(sp + 6) } else { 0 },
                    name,
                    m.read_byte(sp),
                    if op == 0xa81a { 12 } else { 8 },
                )
            };
            if std::env::args().any(|a|a=="--trace-sound") {println!("resource open: name={:?} volume={volume} parent={parent} permission={permission}",String::from_utf8_lossy(&name));}
            let result = self.open(m, heap, files, volume, parent, name, permission)?;
            let reference = match result {
                Ok(r) => r,
                Err(e) => {
                    error = e;
                    -1
                }
            };
            if std::env::args().any(|a|a=="--trace-sound") {println!("resource open result: reference={reference} error={error}");}
            m.write_word(sp + size, reference as u16);
            pop = size;
        } else {
            match op {
                0xa994 => {
                    let r = m.read_word(0xa5a);
                    m.write_word(sp, r);
                    cpu.pc += 2;
                    return Ok(true);
                }
                0xa9af => {
                    let r = m.read_word(0xa60);
                    m.write_word(sp, r);
                    cpu.pc += 2;
                    return Ok(true);
                }
                0xa998 => {
                    let r = m.read_word(sp) as i16;
                    if [0, 2].contains(&r) || self.maps.iter().any(|map| map.reference == r) {
                        m.write_word(0xa5a, r as u16)
                    } else {
                        error = -193
                    }
                    pop = 2;
                }
                0xa99a | 0xa999 => {
                    let reference = m.read_word(sp) as i16;
                    if let Some(i) = self.maps.iter().position(|map| map.reference == reference) {
                        error = if op == 0xa99a {
                            self.close(i, m, heap, files)?
                        } else {
                            self.persist(i, None, m, heap, files)?
                        };
                    } else {
                        return Err(
                            "updating/closing an unowned resource map is not implemented".into(),
                        );
                    }
                    pop = 2;
                }
                0xa81f | 0xa9a0 => {
                    let id = m.read_word(sp) as i16;
                    let kind = m.read_long(sp + 2).to_be_bytes();
                    let h = self.get(m, heap, kind, id, op == 0xa81f, app, cache, &mut load_app)?;
                    if kind==*b"snd "&&std::env::args().any(|a|a=="--trace-sound") {println!("resource sound lookup: id={id} handle={h:#x} error={}",m.read_word(0xa60) as i16);}
                    m.write_long(sp + 6, h);
                    pop = 6;
                }
                0xa80d | 0xa80e => {
                    let Some(i) = self.current(m) else {
                        return Ok(false);
                    };
                    let kind = m
                        .read_long(sp + if op == 0xa80d { 0 } else { 2 })
                        .to_be_bytes();
                    let indices: Vec<_> = self.maps[i]
                        .entries
                        .iter()
                        .enumerate()
                        .filter(|(_, e)| e.data.kind == kind)
                        .map(|(j, _)| j)
                        .collect();
                    if op == 0xa80d {
                        m.write_word(sp + 4, indices.len() as u16);
                        pop = 4
                    } else {
                        let index = m.read_word(sp) as i16;
                        let h = if index > 0 {
                            if let Some(j) = indices.get(index as usize - 1) {
                                self.load(i, *j, m, heap)?
                            } else {
                                0
                            }
                        } else {
                            0
                        };
                        if h == 0 {
                            error = -192
                        }
                        m.write_long(sp + 6, h);
                        pop = 6;
                    }
                }
                0xa9ab => {
                    let Some(i) = self.current(m) else {
                        return Err(
                            "adding to application/system resource map is not implemented".into(),
                        );
                    };
                    if self.maps[i].permission == 1 {
                        return Err("AddResource in a read-only map is not implemented".into());
                    }
                    let name = m.read_long(sp);
                    let id = m.read_word(sp + 4) as i16;
                    let kind = m.read_long(sp + 6).to_be_bytes();
                    let h = m.read_long(sp + 10);
                    if self.locate(h).is_some()
                        || cache.values().any(|v| *v == h)
                        || self.maps[i]
                            .entries
                            .iter()
                            .any(|e| e.data.kind == kind && e.data.id == id)
                    {
                        return Err(
                            "duplicate resource handle or ID addition is not implemented".into(),
                        );
                    }
                    let n = heap.size(h).map_err(|e| e.to_string())?;
                    let p = heap.data(h).map_err(|e| e.to_string())?;
                    let bytes = (0..n).map(|k| m.read_byte(p + k)).collect();
                    let length = if name == 0 {
                        0
                    } else {
                        m.read_byte(name) as u32
                    };
                    let name = if length == 0 {
                        None
                    } else {
                        Some((0..length).map(|k| m.read_byte(name + 1 + k)).collect())
                    };
                    let state = heap.state(h).map_err(|e| e.to_string())?;
                    heap.resource_state(h, state & 0x40 != 0, state & 0x80 != 0)
                        .map_err(|e| e.to_string())?;
                    self.maps[i].entries.push(Entry {
                        data: Data {
                            kind,
                            id,
                            attributes: 2,
                            name,
                            bytes,
                        },
                        handle: h,
                    });
                    self.sync(i, m, heap)?;
                    pop = 14;
                }
                0xa9a4 => {
                    let h = m.read_long(sp);
                    if let Some((i, _)) = self.locate(h) {
                        m.write_word(sp + 4, self.maps[i].reference as u16)
                    } else if cache.values().any(|v| *v == h) {
                        return Ok(false);
                    } else {
                        m.write_word(sp + 4, 0xffff);
                        error = -192
                    }
                    pop = 4;
                }
                0xa9a6 => {
                    let h = m.read_long(sp);
                    if let Some((i, j)) = self.locate(h) {
                        m.write_word(sp + 4, self.maps[i].entries[j].data.attributes as u16)
                    } else {
                        return Err("GetResAttrs of an unowned resource is not implemented".into());
                    }
                    pop = 4;
                }
                0xa9a8 => {
                    let h = m.read_long(sp + 12);
                    let Some((i, j)) = self.locate(h) else {
                        return Ok(false);
                    };
                    let e = &self.maps[i].entries[j];
                    let np = m.read_long(sp);
                    let kp = m.read_long(sp + 4);
                    let ip = m.read_long(sp + 8);
                    m.write_word(ip, e.data.id as u16);
                    m.write_long(kp, u32::from_be_bytes(e.data.kind));
                    let name = e.data.name.as_deref().unwrap_or(&[]);
                    m.write_byte(np, name.len() as u8);
                    for (k, b) in name.iter().enumerate() {
                        m.write_byte(np + 1 + k as u32, *b);
                    }
                    pop = 16;
                }
                0xa9aa | 0xa9b0 | 0xa9ad => {
                    let h = m.read_long(sp);
                    let Some((i, j)) = self.locate(h) else {
                        return Err(format!(
                            "resource mutation/release of an unowned handle is not implemented: trap={op:#x} handle={h:#x} app_cached={} PC={:#x}",
                            cache.values().any(|v| *v == h), cpu.pc
                        ));
                    };
                    match op {
                        0xa9aa => {
                            if self.maps[i].permission == 1 {
                                return Err(
                                    "ChangedResource in a read-only map is not implemented".into(),
                                );
                            }
                            self.maps[i].entries[j].data.attributes |= 2;
                            self.sync(i, m, heap)?;
                        }
                        0xa9b0 => {
                            error = self.persist(i, Some(h), m, heap, files)?;
                        }
                        0xa9ad => return Err("RemoveResource is not implemented".into()),
                        _ => unreachable!(),
                    }
                    pop = 4;
                }
                _ => return Ok(false),
            }
        }
        m.write_word(0xa60, error as u16);
        if matches!(op, 0xa9a3 | 0xa992) {
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr(0);
        }
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}

// Canonical observed system palette data, independently returned on each call.
fn system_color_table(id: i16) -> Option<&'static [u8]> {
    Some(match id {
        1 => include_bytes!("../data/color-tables/1.bin"),
        2 => include_bytes!("../data/color-tables/2.bin"),
        4 => include_bytes!("../data/color-tables/4.bin"),
        8 => include_bytes!("../data/color-tables/8.bin"),
        33 => include_bytes!("../data/color-tables/33.bin"),
        34 => include_bytes!("../data/color-tables/34.bin"),
        36 => include_bytes!("../data/color-tables/36.bin"),
        40 => include_bytes!("../data/color-tables/40.bin"),
        _ => return None,
    })
}
