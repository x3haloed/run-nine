//! Classic QuickDraw globals and monochrome GrafPort state.
use crate::managers::{Heap, Rect};
use m68k::AddressBus;
#[cfg(test)]
mod device_lifetime_tests {
    use super::*;
    use crate::machine::Memory;
    #[test]
    fn pixmap_table_allocation_failure_rolls_back_without_committing_cpu_return() {
        let mut m = Memory::zeroed(0x800000);
        let mut heap = Heap::new(0x20000, 0x500000);
        let mut q = QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let gd = q.device(&mut m, &mut heap).unwrap();
        let before_device = m.read_long(gd);
        let fill = heap.free_bytes() - 64;
        heap.new_ptr(&mut m, fill, false).unwrap();
        let free = heap.free_bytes();
        assert_eq!(free, 64);
        let mut cpu = m68k::CpuCore::new();
        cpu.pc = 0x19000;
        cpu.dar[15] = 0x700000;
        cpu.dar[0] = 0x12345678;
        cpu.set_ccr(31);
        m.write_long(0x700000, 0x7b7b7b7b);
        assert!(device_lifecycle_dispatch(0xaa03, &mut cpu, &mut m, &mut heap, &mut q).is_err());
        assert_eq!(heap.free_bytes(), free);
        assert_eq!(heap.data(gd).unwrap(), before_device);
        assert_eq!(cpu.pc, 0x19000);
        assert_eq!(cpu.dar[15], 0x700000);
        assert_eq!(cpu.dar[0], 0x12345678);
        assert_eq!(cpu.get_sr() & 31, 31);
        assert_eq!(m.read_long(0x700000), 0x7b7b7b7b);
    }
    #[test]
    fn inverse_lookup_follows_relocated_handle_data() {
        let mut m = Memory::zeroed(0x800000);
        let mut heap = Heap::new(0x20000, 0x500000);
        let mut q = QuickDraw::new();
        q.init(&mut m, 0x100000).unwrap();
        let gd = q.device(&mut m, &mut heap).unwrap();
        let p = m.read_long(gd);
        let pm = m.read_long(p + 22);
        let pp = m.read_long(pm);
        let ct = m.read_long(pp + 42);
        let it = heap.new_handle(&mut m, 4, true).unwrap();
        m.write_long(p + 6, it);
        q.make_inverse(&mut m, &mut heap, ct, it, 3).unwrap();
        let before = heap.data(it).unwrap();
        heap.move_high(&mut m, it).unwrap();
        assert_ne!(before, heap.data(it).unwrap());
        // This measured gray-table result differs from the fallback matcher,
        // so a stale data-pointer registration would break the assertion.
        assert_eq!(q.color_index(&mut m, [0, 7 << 13, 1 << 13]).unwrap(), 0);
    }
    #[test]
    fn partial_device_allocation_rolls_back_payloads() {
        let mut m = Memory::zeroed(0x800000);
        let mut heap = Heap::new(0x20000, 0x20150);
        heap.more_masters(&mut m).unwrap();
        let free = heap.free_bytes();
        let mut q = QuickDraw::new();
        assert!(q.new_offscreen_device(&mut m, &mut heap).is_err());
        assert_eq!(heap.free_bytes(), free);
        assert!(q.offscreen.is_empty());
    }
    #[test]
    fn current_or_aliased_device_rejection_precedes_any_release() {
        let mut m = Memory::zeroed(0x800000);
        let mut heap = Heap::new(0x20000, 0x500000);
        heap.more_masters(&mut m).unwrap();
        let free = heap.free_bytes();
        let mut q = QuickDraw::new();
        let h = q.new_offscreen_device(&mut m, &mut heap).unwrap();
        let p = m.read_long(h);
        m.write_long(0xcc8, h);
        assert!(q.dispose_offscreen_device(&mut m, &mut heap, h).is_err());
        assert!(heap.data(h).is_ok());
        m.write_long(0xcc8, 0);
        let data = m.read_long(p + 50);
        let mask = m.read_long(p + 54);
        m.write_long(p + 54, data);
        let occupied = heap.free_bytes();
        assert!(q.dispose_offscreen_device(&mut m, &mut heap, h).is_err());
        assert_eq!(heap.free_bytes(), occupied);
        assert!(heap.data(data).is_ok());
        m.write_long(p + 54, mask);
        q.dispose_offscreen_device(&mut m, &mut heap, h).unwrap();
        assert_eq!(heap.free_bytes(), free);
    }
}
pub fn line_dispatch<B: AddressBus>(
    opcode:u16,cpu:&mut m68k::CpuCore,m:&mut B,q:&QuickDraw,
)->Result<bool,String>{
    if !matches!(opcode,0xa891|0xa892){return Ok(false);}
    let sp=cpu.dar[15];let mut v=m.read_word(sp)as i16;let mut h=m.read_word(sp+2)as i16;
    if opcode==0xa892{
        let p=q.get_port(m)?;if p==0{return Err("Line without current port".into());}
        v=v.wrapping_add(m.read_word(p+48)as i16);h=h.wrapping_add(m.read_word(p+50)as i16);
    }
    q.line_to(m,h,v)?;cpu.dar[15]+=4;cpu.set_ccr(0);cpu.pc+=2;Ok(true)
}

pub fn pen_pattern_dispatch<B: AddressBus>(
    opcode: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    q: &QuickDraw,
) -> Result<bool, String> {
    if opcode != 0xa89d { return Ok(false); }
    let source = m.read_long(cpu.dar[15]);
    q.pen_pattern(m, source)?;
    cpu.dar[15] += 4;
    cpu.set_ccr(0);
    cpu.pc += 2;
    Ok(true)
}

pub fn device_lifecycle_dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    heap: &mut Heap,
    q: &mut QuickDraw,
) -> Result<bool, String> {
    let sp = cpu.dar[15];
    match op {
        0xab1d if cpu.dar[0] == 0x00040007 => {
            let h = m.read_long(sp);
            let p = heap
                .data(h)
                .map_err(|e| format!("CTabChanged handle: {e}"))?;
            if heap.size(h).map_err(|e| e.to_string())? < 8 {
                return Err("CTabChanged short table outside measured profile".into());
            }
            m.write_long(p, q.next_color_seed());
            cpu.dar[15] += 4;
            cpu.set_ccr(0);
        }
        0xaa28 => {
            m.write_long(sp, q.next_color_seed());
            cpu.set_ccr(0);
        }
        0xaa03 => {
            let h = q.new_pixmap(m, heap)?;
            m.write_long(sp, h);
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            m.write_word(0x220, 0);
        }
        0xaa05 => {
            let dest = m.read_long(sp);
            let source = m.read_long(sp + 4);
            q.copy_pixmap(m, heap, source, dest)?;
            cpu.dar[15] += 8;
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            m.write_word(0x220, 0);
        }
        0xaa04 => {
            let h = m.read_long(sp);
            if h == 0 {
                heap.error = 0;
                cpu.set_ccr(0);
            } else {
                q.dispose_pixmap(m, heap, h)?;
                cpu.dar[0] = 0;
                cpu.set_ccr(4);
            }
            cpu.dar[15] += 4;
            m.write_word(0x220, 0);
        }
        0xaa2f => {
            if m.read_long(sp) != u32::MAX {
                return Err(
                    "NewGDevice driver-backed initialization outside measured profile".into(),
                );
            }
            let h = q.new_offscreen_device(m, heap)?;
            m.write_long(sp + 6, h);
            cpu.dar[15] += 6;
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            m.write_word(0x220, 0);
        }
        0xaa31 => {
            let h = m.read_long(sp);
            let main = q.device(m, heap)?;
            if h != main && !q.offscreen.contains(&h) {
                return Err("SetGDevice unknown device".into());
            }
            heap.data(h).map_err(|e| format!("SetGDevice: {e}"))?;
            m.write_long(0xcc8, h);
            cpu.dar[15] += 4;
            cpu.set_ccr(0);
        }
        0xaa32 => {
            q.device(m, heap)?;
            let h = m.read_long(0xcc8);
            m.write_long(sp, h);
        }
        0xaa30 => {
            let h = m.read_long(sp);
            q.dispose_offscreen_device(m, heap, h)?;
            cpu.dar[15] += 4;
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            m.write_word(0x220, 0);
        }
        0xaa24 => {
            let h = m.read_long(sp);
            heap.dispose(m, h)
                .map_err(|e| format!("DisposeCTable: {e}"))?;
            cpu.dar[15] += 4;
            cpu.dar[0] = 0;
            cpu.set_ccr(4);
            m.write_word(0x220, 0);
        }
        _ => return Ok(false),
    }
    cpu.pc += 2;
    Ok(true)
}
/// Single active software display profile. Other device topologies need their
/// own measured depth/tie selection policy.
pub fn max_device_dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    heap: &mut Heap,
    q: &mut QuickDraw,
) -> Result<bool, String> {
    if op != 0xaa27 {
        return Ok(false);
    }
    let sp = cpu.dar[15];
    let rect_ptr = m.read_long(sp);
    let rect = read_rect(m, rect_ptr);
    let main = q.device(m, heap)?;
    let p = m.read_long(main);
    if m.read_long(p + 30) != 0 {
        return Err("GetMaxDevice multiple display selection outside measured profile".into());
    }
    let screen = read_rect(m, p + 34);
    let intersects = rect.left < rect.right
        && rect.top < rect.bottom
        && rect.left < screen.right
        && rect.right > screen.left
        && rect.top < screen.bottom
        && rect.bottom > screen.top;
    m.write_long(sp + 4, if intersects { main } else { 0 });
    cpu.dar[15] += 4;
    cpu.pc += 2;
    cpu.set_ccr(0);
    Ok(true)
}
pub fn origin_dispatch<B: AddressBus>(
    op: u16,
    cpu: &mut m68k::CpuCore,
    m: &mut B,
    heap: &mut Heap,
    q: &QuickDraw,
) -> Result<bool, String> {
    if op == 0xa870 || op == 0xa871 {
        let sp = cpu.dar[15];
        let p = m.read_long(sp);
        let port = q.get_port(m)?;
        let bitmap = q.port_bitmap(m, port)?;
        let bounds = read_rect(m, bitmap + 6);
        let v = m.read_word(p) as i16;
        let h = m.read_word(p + 2) as i16;
        m.write_word(
            p,
            if op == 0xa870 {
                v.wrapping_sub(bounds.top)
            } else {
                v.wrapping_add(bounds.top)
            } as u16,
        );
        m.write_word(
            p + 2,
            if op == 0xa870 {
                h.wrapping_sub(bounds.left)
            } else {
                h.wrapping_add(bounds.left)
            } as u16,
        );
        cpu.dar[15] += 4;
        cpu.pc += 2;
        return Ok(true);
    }
    if op != 0xa878 {
        return Ok(false);
    }
    let sp = cpu.dar[15];
    let v = m.read_word(sp) as i16;
    let h = m.read_word(sp + 2) as i16;
    q.set_origin(m, heap, h, v)?;
    cpu.dar[15] += 4;
    cpu.pc += 2;
    cpu.set_ccr(0);
    Ok(true)
}
/// Read-only diagnostics at the shared renderer boundary. Normal execution
/// leaves observation disabled and does not allocate or retain text runs.
pub struct TextObservation {
    pub bytes: Vec<u8>,
    pub point: [i16; 2],
    pub font: i16,
    pub size: i16,
    pub face: u8,
    pub mode: u16,
    pub clip: Rect,
}
pub struct QuickDraw {
    color_seeds: std::rc::Rc<classic_core::color_seed::ColorSeeds>,
    slot: Option<u32>,
    pub screen_base: u32,
    pub width: i16,
    pub height: i16,
    device: Option<u32>,
    offscreen: std::collections::BTreeSet<u32>,
    inverse_handles: std::collections::BTreeSet<u32>,
    pub color_capable: bool,
    pub depth: u16,
    pub draw_error: i16,
    ports: std::cell::RefCell<std::collections::BTreeSet<u32>>,
    color_cache: std::cell::RefCell<Option<(u32, u32, std::collections::BTreeMap<[u16; 3], u8>)>>,
    text_observations: std::cell::RefCell<Option<Vec<TextObservation>>>,
}
impl QuickDraw {
    pub fn make_inverse<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        ct: u32,
        it: u32,
        res: u16,
    ) -> Result<(), String> {
        if !matches!(res, 3 | 4) || ct == 0 || it == 0 || ct == it {
            return Err("MakeITable arguments outside measured three/four-bit profile".into());
        }
        let table = heap
            .data(ct)
            .map_err(|e| format!("MakeITable color table: {e}"))?;
        let n = m.read_word(table + 6) as i16 as i32 + 1;
        if !(1..=256).contains(&n) || heap.size(ct).map_err(|e| e.to_string())? < 8 + 8 * n as u32 {
            return Err("short or unsupported MakeITable color table".into());
        }
        let palette: Vec<[u16; 3]> = (0..n as u32)
            .map(|i| {
                let p = table + 10 + 8 * i;
                [m.read_word(p), m.read_word(p + 2), m.read_word(p + 4)]
            })
            .collect();
        let seed = m.read_long(table);
        let grid = classic_core::pixels::inverse_grid(&palette,res).ok_or("invalid inverse palette")?;
        let allocation = 6 + grid.len() as u32 + 518;
        // Match the measured logical allocation; the trailing owner-private
        // area is reserved, without importing Apple's private cache format.
        heap.resize(m, it, allocation)
            .map_err(|e| format!("MakeITable allocation: {e}"))?;
        let p = heap.data(it).map_err(|e| e.to_string())?;
        for i in 0..allocation {
            m.write_byte(p + i, 0);
        }
        m.write_long(p, seed);
        m.write_word(p + 4, res);
        for (i, value) in grid.into_iter().enumerate() {
            m.write_byte(p + 6 + i as u32, value);
        }
        self.inverse_handles.insert(it);
        *self.color_cache.borrow_mut() = None;
        self.draw_error = 0;
        Ok(())
    }
    pub fn open_color_port<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        port: u32,
    ) -> Result<(), String> {
        self.device(m, heap)?;
        let current = m.read_long(0xcc8);
        let gd = heap.data(current).map_err(|e| e.to_string())?;
        let source = m.read_long(gd + 22);
        let p = heap.data(source).map_err(|e| e.to_string())?;
        let bytes: Vec<u8> = (0..50).map(|i| m.read_byte(p + i)).collect();
        self.open_port(m, heap, port)?;
        self.make_color(m, heap, port)?;
        let pm = m.read_long(port + 2);
        let dest = heap.data(pm).map_err(|e| e.to_string())?;
        for (i, value) in bytes.into_iter().enumerate() {
            m.write_byte(dest + i as u32, value);
        }
        let fg = self.color_index(m, [0; 3])?;
        let bg = self.color_index(m, [65535; 3])?;
        m.write_long(port + 80, fg as u32);
        m.write_long(port + 84, bg as u32);
        Ok(())
    }
    /// Native NewPixMap clones the current device descriptor, with a fresh
    /// zeroed24byte color-table allocation rather than the device's colors.
    pub fn color_seeds(&self) -> std::rc::Rc<classic_core::color_seed::ColorSeeds> {
        self.color_seeds.clone()
    }
    pub fn next_color_seed(&self) -> u32 {
        self.color_seeds.next()
    }
    pub fn new_pixmap<B: AddressBus>(&mut self, m: &mut B, heap: &mut Heap) -> Result<u32, String> {
        self.device(m, heap)?;
        let gd = m.read_long(0xcc8);
        let gp = heap
            .data(gd)
            .map_err(|e| format!("NewPixMap current device: {e}"))?;
        let device_pm = m.read_long(gp + 22);
        let source = heap
            .data(device_pm)
            .map_err(|e| format!("NewPixMap device map: {e}"))?;
        if heap.size(device_pm).map_err(|e| e.to_string())? < 50 {
            return Err("NewPixMap short device descriptor".into());
        }
        let descriptor: Vec<_> = (0..50).map(|i| m.read_byte(source + i)).collect();
        let h = heap
            .new_handle(m, 50, false)
            .map_err(|e| format!("NewPixMap allocation: {e}"))?;
        let table = match heap.new_handle(m, 24, true) {
            Ok(t) => t,
            Err(e) => {
                let _ = heap.dispose(m, h);
                return Err(format!("NewPixMap table allocation: {e}"));
            }
        };
        let p = heap.data(h).map_err(|e| e.to_string())?;
        for (i, b) in descriptor.iter().enumerate() {
            m.write_byte(p + i as u32, *b);
        }
        m.write_long(p + 42, table);
        Ok(h)
    }
    pub fn copy_pixmap<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        source: u32,
        dest: u32,
    ) -> Result<(), String> {
        let sp = heap
            .data(source)
            .map_err(|e| format!("CopyPixMap source: {e}"))?;
        let dp = heap
            .data(dest)
            .map_err(|e| format!("CopyPixMap destination: {e}"))?;
        if heap.size(source).map_err(|e| e.to_string())? != 50
            || heap.size(dest).map_err(|e| e.to_string())? != 50
        {
            return Err("CopyPixMap descriptor size outside measured public50byte profile".into());
        }
        if source == dest {
            return Ok(());
        }
        let st = m.read_long(sp + 42);
        let dt = m.read_long(dp + 42);
        if st == 0
            || dt == 0
            || st == dt
            || [source, dest].contains(&st)
            || [source, dest].contains(&dt)
        {
            return Err("CopyPixMap missing/aliased color tables outside measured profile".into());
        }
        let tp = heap
            .data(st)
            .map_err(|e| format!("CopyPixMap source table: {e}"))?;
        let n = heap.size(st).map_err(|e| e.to_string())?;
        heap.data(dt)
            .map_err(|e| format!("CopyPixMap destination table: {e}"))?;
        let descriptor: Vec<_> = (0..50).map(|i| m.read_byte(sp + i)).collect();
        let colors: Vec<_> = (0..n).map(|i| m.read_byte(tp + i)).collect();
        heap.resize(m, dt, n)
            .map_err(|e| format!("CopyPixMap table resize: {e}"))?;
        let tp = heap.data(dt).map_err(|e| e.to_string())?;
        let dp = heap.data(dest).map_err(|e| e.to_string())?;
        for (i, b) in colors.iter().enumerate() {
            m.write_byte(tp + i as u32, *b);
        }
        for (i, b) in descriptor.iter().enumerate() {
            m.write_byte(dp + i as u32, *b);
        }
        m.write_long(dp + 42, dt);
        Ok(())
    }
    pub fn dispose_pixmap<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<(), String> {
        let p = heap
            .data(h)
            .map_err(|e| format!("DisposePixMap handle: {e}"))?;
        if heap.size(h).map_err(|e| e.to_string())? != 50 {
            return Err("DisposePixMap size outside measured50byte profile".into());
        }
        let t = m.read_long(p + 42);
        if t == h {
            return Err("DisposePixMap aliased table".into());
        }
        if t != 0 {
            heap.data(t)
                .map_err(|e| format!("DisposePixMap table: {e}"))?;
        }
        if t != 0 {
            heap.dispose(m, t).map_err(|e| e.to_string())?;
        }
        heap.dispose(m, h).map_err(|e| e.to_string())?;
        Ok(())
    }
    pub fn new_offscreen_device<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
    ) -> Result<u32, String> {
        let mut handles = Vec::new();
        for size in [62, 50, 4, 2, 2, 32] {
            match heap.new_handle(m, size, true) {
                Ok(h) => handles.push(h),
                Err(error) => {
                    for h in handles {
                        let _ = heap.dispose(m, h);
                    }
                    return Err(format!("NewGDevice allocation failed: {error}"));
                }
            }
        }
        let h = handles[0];
        let p = m.read_long(h);
        let pm = m.read_long(handles[1]);
        m.write_long(p + 6, handles[2]);
        m.write_long(p + 22, handles[1]);
        m.write_long(p + 50, handles[3]);
        m.write_long(p + 54, handles[4]);
        m.write_long(pm + 42, handles[5]);
        self.offscreen.insert(h);
        Ok(h)
    }
    pub fn dispose_offscreen_device<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        h: u32,
    ) -> Result<(), String> {
        if !self.offscreen.contains(&h) {
            return Err("DisposeGDevice is not an owned offscreen device".into());
        }
        if m.read_long(0xcc8) == h {
            return Err("DisposeGDevice of current device outside measured profile".into());
        }
        if heap.size(h).map_err(|e| e.to_string())? < 62 {
            return Err("short GDevice".into());
        }
        let p = m.read_long(h);
        let pm = m.read_long(p + 22);
        let mut children = vec![m.read_long(p + 6), m.read_long(p + 50), m.read_long(p + 54)];
        if pm != 0 {
            let pp = heap.data(pm).map_err(|e| e.to_string())?;
            if heap.size(pm).map_err(|e| e.to_string())? < 50 {
                return Err("short device PixMap".into());
            }
            children.push(m.read_long(pp + 42));
            children.push(pm);
        }
        let mut seen = std::collections::BTreeSet::new();
        for &child in children.iter().filter(|&&c| c != 0) {
            if child == h
                || self.device == Some(child)
                || self.offscreen.contains(&child)
                || !seen.insert(child)
            {
                return Err("DisposeGDevice aliased child outside measured profile".into());
            }
            heap.data(child)
                .map_err(|e| format!("DisposeGDevice child: {e}"))?;
        }
        for child in children.into_iter().filter(|&c| c != 0) {
            self.inverse_handles.remove(&child);
            heap.dispose(m, child).map_err(|e| e.to_string())?;
        }
        heap.dispose(m, h).map_err(|e| e.to_string())?;
        self.offscreen.remove(&h);
        Ok(())
    }
    /// Translate screen-fixed state into the new local coordinate system.
    /// Pen and clip coordinates deliberately remain in application space.
    pub fn set_origin<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        h: i16,
        v: i16,
    ) -> Result<(), String> {
        let port = self.get_port(m)?;
        let r = read_rect(m, port + 16);
        let dh = h
            .checked_sub(r.left)
            .ok_or("SetOrigin horizontal overflow")?;
        let dv = v.checked_sub(r.top).ok_or("SetOrigin vertical overflow")?;
        let offset = |r: Rect| -> Result<Rect, String> {
            Ok(Rect {
                top: r.top.checked_add(dv).ok_or("SetOrigin top overflow")?,
                bottom: r
                    .bottom
                    .checked_add(dv)
                    .ok_or("SetOrigin bottom overflow")?,
                left: r.left.checked_add(dh).ok_or("SetOrigin left overflow")?,
                right: r.right.checked_add(dh).ok_or("SetOrigin right overflow")?,
            })
        };
        let bitmap = self.port_bitmap(m, port)?;
        let port_rect = offset(r)?;
        let bounds = offset(read_rect(m, bitmap + 6))?;
        let vis = m.read_long(port + 24);
        crate::regions::offset_handle(m, heap, vis, dh, dv)?;
        write_rect(m, port + 16, port_rect);
        write_rect(m, bitmap + 6, bounds);
        Ok(())
    }
    pub fn new() -> Self {
        Self {
            color_seeds: Default::default(),
            slot: None,
            screen_base: 0x500000,
            width: 640,
            height: 480,
            device: None,
            offscreen: Default::default(),
            inverse_handles: Default::default(),
            color_capable: false,
            depth: 1,
            draw_error: 0,
            ports: Default::default(),
            color_cache: Default::default(),
            text_observations: Default::default(),
        }
    }
    pub fn row_bytes(&self) -> u16 {
        if self.depth == 8 {
            self.width as u16
        } else {
            ((self.width as u16 + 15) / 16) * 2
        }
    }
    /// One active monochrome screen, expressed using the public GDevice layout.
    pub fn device<B: AddressBus>(&mut self, m: &mut B, heap: &mut Heap) -> Result<u32, String> {
        if let Some(h) = self.device {
            return Ok(h);
        }
        let h = heap
            .new_handle(m, 62, true)
            .map_err(|e| format!("GDevice: {e}"))?;
        let p = heap.data(h).map_err(|e| format!("GDevice: {e}"))?;
        let pm = heap
            .new_handle(m, 50, true)
            .map_err(|e| format!("device PixMap: {e}"))?;
        let pp = heap.data(pm).map_err(|e| format!("device PixMap: {e}"))?;
        self.bitmap(m, pp);
        m.write_word(pp + 4, self.row_bytes() | 0x8000);
        m.write_long(pp + 22, 72 << 16);
        m.write_long(pp + 26, 72 << 16);
        m.write_word(pp + 32, 1); // one bit per pixel
        m.write_word(pp + 34, 1); // one component
        m.write_word(pp + 36, 1); // one bit per component
        let colors = heap
            .new_handle(m, 24, true)
            .map_err(|e| format!("device colors: {e}"))?;
        let cp = heap
            .data(colors)
            .map_err(|e| format!("device colors: {e}"))?;
        m.write_word(cp + 6, 1); // last index: two entries
        for off in [10, 12, 14] {
            m.write_word(cp + off, 0xffff);
        } // index zero = white
        m.write_word(cp + 16, 1); // index one = black
        m.write_long(pp + 42, colors);
        m.write_word(p, 0xffff);
        m.write_word(p + 20, (1 << 5) | (1 << 11) | (1 << 13) | (1 << 15));
        m.write_long(p + 22, pm);
        write_rect(
            m,
            p + 34,
            Rect {
                top: 0,
                left: 0,
                bottom: self.height,
                right: self.width,
            },
        );
        m.write_long(0x8a4, h); // MainDevice
        m.write_long(0x8a8, h); // DeviceList
        m.write_long(0xcc8, h); // TheGDevice
        self.device = Some(h);
        Ok(h)
    }
    pub fn init<B: AddressBus>(&mut self, m: &mut B, slot: u32) -> Result<(), String> {
        let globals = slot
            .checked_sub(202)
            .ok_or("InitGraf globals before address zero")?;
        self.slot = Some(slot);
        for i in 0..206 {
            m.write_byte(globals + i, 0);
        }
        m.write_long(globals + 76, 1);
        self.bitmap(m, globals + 80);
        for (i, b) in crate::cursors::builtin(0).unwrap().iter().enumerate() {
            m.write_byte(globals + 94 + i as u32, *b);
        }
        // Patterns are eight rows of eight bits, in QuickDraw's native order.
        for (offset, rows) in [
            (162, [0x77, 0xdd, 0x77, 0xdd, 0x77, 0xdd, 0x77, 0xdd]),
            (170, [0x88, 0x22, 0x88, 0x22, 0x88, 0x22, 0x88, 0x22]),
            (178, [0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55, 0xaa, 0x55]),
            (186, [255; 8]),
            (194, [0; 8]),
        ] {
            for (i, b) in rows.iter().enumerate() {
                m.write_byte(globals + offset + i as u32, *b);
            }
        }
        // InitGraf registers the globals; it does not create a current port.
        m.write_long(slot, 0);
        m.write_long(0x824, self.screen_base);
        m.write_word(0x106, self.row_bytes());
        Ok(())
    }
    pub fn bitmap<B: AddressBus>(&self, m: &mut B, p: u32) {
        m.write_long(p, self.screen_base);
        m.write_word(p + 4, self.row_bytes());
        write_rect(
            m,
            p + 6,
            Rect {
                top: 0,
                left: 0,
                bottom: self.height,
                right: self.width,
            },
        );
    }
    pub fn random<B: AddressBus>(&self, m: &mut B) -> Result<u32, String> {
        let slot = self.slot.ok_or("Random before InitGraf")?;
        let address = slot - 126; // QDGlobals.randSeed, before thePort slot.
        let result = classic_core::random::next(m.read_long(address));
        m.write_long(address, result);
        Ok(result)
    }
    pub fn get_port<B: AddressBus>(&self, m: &mut B) -> Result<u32, String> {
        Ok(m.read_long(self.slot.ok_or("QuickDraw globals not initialized")?))
    }
    pub fn set_port<B: AddressBus>(&self, m: &mut B, port: u32) -> Result<(), String> {
        m.write_long(self.slot.ok_or("QuickDraw globals not initialized")?, port);
        Ok(())
    }
    pub fn open_port<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        port: u32,
    ) -> Result<(), String> {
        for i in 0..108 {
            m.write_byte(port + i, 0);
        }
        self.bitmap(m, port + 2);
        let bounds = Rect {
            top: 0,
            left: 0,
            bottom: self.height,
            right: self.width,
        };
        write_rect(m, port + 16, bounds);
        for (offset, rect) in [
            (24, bounds),
            (
                28,
                Rect {
                    top: -32767,
                    left: -32767,
                    bottom: 32767,
                    right: 32767,
                },
            ),
        ] {
            let handle = heap
                .new_handle(m, 10, true)
                .map_err(|e| format!("OpenPort region: {e}"))?;
            let p = heap.data(handle).map_err(|e| e.to_string())?;
            m.write_word(p, 10);
            write_rect(m, p + 2, rect);
            m.write_long(port + offset, handle);
        }
        for i in 0..8 {
            m.write_byte(port + 40 + i, 255);
            m.write_byte(port + 58 + i, 255);
        }
        m.write_word(port + 52, 1);
        m.write_word(port + 54, 1);
        m.write_word(port + 56, 8);
        m.write_word(port + 72, 1);
        m.write_long(port + 80, 33);
        m.write_long(port + 84, 30);
        self.ports.borrow_mut().insert(port);
        self.set_port(m, port)
    }
    pub fn close_port<B: AddressBus>(
        &self,
        m: &mut B,
        heap: &mut Heap,
        port: u32,
    ) -> Result<(), String> {
        self.ports.borrow_mut().remove(&port);
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            for offset in [2, 8, 32, 58, 62] {
                let h = m.read_long(port + offset);
                if h != 0 {
                    heap.dispose(m, h).map_err(|e| format!("CloseCPort: {e}"))?;
                }
            }
        }
        for offset in [24, 28] {
            let h = m.read_long(port + offset);
            heap.dispose(m, h)
                .map_err(|e| format!("ClosePort region: {e}"))?;
            m.write_long(port + offset, 0);
        }
        Ok(())
    }
    pub fn make_color<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        port: u32,
    ) -> Result<(), String> {
        let bounds = read_rect(m, port + 8);
        let device = self.device(m, heap)?;
        let dp = m.read_long(device);
        let source_pm = m.read_long(dp + 22);
        let source = m.read_long(source_pm);
        let pm = heap
            .new_handle(m, 50, false)
            .map_err(|e| format!("port PixMap: {e}"))?;
        let pp = heap.data(pm).map_err(|e| format!("port PixMap: {e}"))?;
        for i in 0..50 {
            let b = m.read_byte(source + i);
            m.write_byte(pp + i, b);
        }
        write_rect(m, pp + 6, bounds);
        let gv = heap
            .new_handle(m, 26, true)
            .map_err(|e| format!("grafVars: {e}"))?;
        m.write_long(port + 2, pm);
        m.write_word(port + 6, 0xc000);
        m.write_long(port + 8, gv);
        m.write_long(port + 12, 0);
        m.write_word(port + 14, 0x8000);
        m.write_word(port + 68, 1);
        for (offset, bit) in [(32, 0), (58, 255), (62, 255)] {
            let h = heap
                .new_handle(m, 28, true)
                .map_err(|e| format!("PixPat: {e}"))?;
            let p = heap.data(h).map_err(|e| format!("PixPat: {e}"))?;
            for i in 0..8 {
                m.write_byte(p + 20 + i, bit);
            }
            m.write_long(port + offset, h);
        }
        for i in 0..6 {
            m.write_byte(port + 36 + i, 0);
            m.write_byte(port + 42 + i, 255);
        }
        Ok(())
    }
    pub fn port_bitmap<B: AddressBus>(&self, m: &mut B, port: u32) -> Result<u32, String> {
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            let h = m.read_long(port + 2);
            let p = m.read_long(h);
            if !matches!(m.read_word(p + 32), 1 | 8) {
                return Err("only one-bit and indexed eight-bit PixMaps are implemented".into());
            }
            Ok(p)
        } else {
            Ok(port + 2)
        }
    }
    pub fn pattern<B: AddressBus>(
        &self,
        m: &mut B,
        port: u32,
        offset: u32,
    ) -> Result<[u8; 8], String> {
        let mut p = port + offset;
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            let h = m.read_long(p);
            if h == 0 {
                return Err("nil PixPat".into());
            }
            p = m.read_long(h);
            if m.read_word(p) != 0 {
                return Err("non-monochrome PixPat is not implemented".into());
            }
            p += 20;
        }
        let mut pattern = [0; 8];
        for (i, b) in pattern.iter_mut().enumerate() {
            *b = m.read_byte(p + i as u32);
        }
        Ok(pattern)
    }
    pub fn pen_pattern<B: AddressBus>(&self, m: &mut B, source: u32) -> Result<(), String> {
        let port = self.get_port(m)?;
        if port == 0 || source == 0 {
            return Err("PenPat without current port or pattern".into());
        }
        // Copy first, so caller storage may overlap the public pen pattern.
        let bytes: [u8; 8] = std::array::from_fn(|i| m.read_byte(source + i as u32));
        let mut destination = port + 58;
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            let handle = m.read_long(destination);
            let data = if handle != 0 { m.read_long(handle) } else { 0 };
            if data == 0 || m.read_word(data) != 0 {
                return Err("PenPat replacing a non-monochrome PixPat is not implemented".into());
            }
            destination = data + 20;
        }
        for (i, byte) in bytes.into_iter().enumerate() {
            m.write_byte(destination + i as u32, byte);
        }
        Ok(())
    }
    pub fn set_word<B: AddressBus>(
        &self,
        m: &mut B,
        offset: u32,
        value: u16,
    ) -> Result<(), String> {
        let port = self.get_port(m)?;
        if port == 0 {
            return Err("QuickDraw operation without a current port".into());
        }
        m.write_word(port + offset, value);
        Ok(())
    }
    pub fn move_to<B: AddressBus>(&self, m: &mut B, h: i16, v: i16) -> Result<(), String> {
        self.set_word(m, 48, v as u16)?;
        self.set_word(m, 50, h as u16)
    }
    pub fn pen_size<B: AddressBus>(
        &self,
        m: &mut B,
        width: i16,
        height: i16,
    ) -> Result<(), String> {
        self.set_word(m, 52, height as u16)?;
        self.set_word(m, 54, width as u16)
    }
}
pub fn read_rect<B: AddressBus>(m: &mut B, p: u32) -> Rect {
    Rect {
        top: m.read_word(p) as i16,
        left: m.read_word(p + 2) as i16,
        bottom: m.read_word(p + 4) as i16,
        right: m.read_word(p + 6) as i16,
    }
}
pub fn write_rect<B: AddressBus>(m: &mut B, p: u32, r: Rect) {
    for (i, v) in [r.top, r.left, r.bottom, r.right].iter().enumerate() {
        m.write_word(p + 2 * i as u32, *v as u16);
    }
}

impl QuickDraw {
    /// Transfer a monochrome source bit through the current port's regions.
    pub fn transfer<B: AddressBus>(
        &self,
        m: &mut B,
        port: u32,
        x: i16,
        y: i16,
        source: bool,
        mode: u16,
    ) -> Result<(), String> {
        let vis = m.read_long(port + 24);
        let clip = m.read_long(port + 28);
        if !crate::regions::contains(m, vis, y, x)? || !crate::regions::contains(m, clip, y, x)? {
            return Ok(());
        }
        let surface = self.port_surface(m, port)?;
        if surface.depth == 8 || m.read_word(port + 6) & 0xc000 == 0xc000 {
            let (fg, bg) = self.port_inks(m, port)?;
            if let Some((value, op)) = Self::pattern_pixel(source, mode, fg, bg) {
                surface.write(m, x, y, value, op);
            }
            return Ok(());
        }
        let bitmap = self.port_bitmap(m, port)?;
        let bounds = read_rect(m, bitmap + 6);
        if !bounds.contains(y, x) {
            return Ok(());
        }
        let row = m.read_word(bitmap + 4) & 0x3fff;
        let base = m.read_long(bitmap);
        let ink = if source {
            m.read_long(port + 80) != 30
        } else {
            m.read_long(port + 84) != 30
        };
        Self::transfer_ink(m, base, row, bounds, x, y, ink, mode)
    }
    #[allow(clippy::too_many_arguments)]
    fn transfer_ink<B: AddressBus>(
        m: &mut B,
        base: u32,
        row: u16,
        bounds: Rect,
        x: i16,
        y: i16,
        source: bool,
        mode: u16,
    ) -> Result<(), String> {
        let col = (x as i32 - bounds.left as i32) as u32;
        let addr = base
            .checked_add((y as i32 - bounds.top as i32) as u32 * row as u32)
            .and_then(|p| p.checked_add(col / 8))
            .ok_or("bitmap address overflow")?;
        let mask = 0x80u8 >> (col & 7);
        let byte = m.read_byte(addr);
        let dest = byte & mask != 0;
        let result = match mode & 7 {
            0 => source,
            1 => source | dest,
            2 => source ^ dest,
            3 => (!source) & dest,
            4 => !source,
            5 => (!source) | dest,
            6 => (!source) ^ dest,
            7 => source & dest,
            _ => unreachable!(),
        };
        m.write_byte(addr, if result { byte | mask } else { byte & !mask });
        Ok(())
    }
    pub fn paint_rect<B: AddressBus>(
        &self,
        m: &mut B,
        r: Rect,
        pattern: [u8; 8],
        mode: u16,
    ) -> Result<(), String> {
        let port = self.get_port(m)?;
        if port == 0 {
            return Err("rectangle drawing without current port".into());
        }
        let bitmap = self.port_bitmap(m, port)?;
        let bounds = read_rect(m, bitmap + 6);
        if let Some(r) = r.intersection(bounds) {
            // Resolve the immutable port regions once for this synchronous
            // primitive, rather than parsing both handles for every pixel.
            let vis = m.read_long(port + 24);
            let clip = m.read_long(port + 28);
            let area = crate::regions::Region::rectangle(r)
                .combine(
                    &crate::regions::Region::read(m, vis)?,
                    crate::regions::Operation::Intersection,
                )
                .combine(
                    &crate::regions::Region::read(m, clip)?,
                    crate::regions::Operation::Intersection,
                );
            let base = m.read_long(bitmap);
            let row = m.read_word(bitmap + 4) & 0x3fff;
            let foreground = m.read_long(port + 80) != 30;
            let background = m.read_long(port + 84) != 30;
            let surface = self.port_surface(m, port)?;
            let inks = if surface.depth == 8 || m.read_word(port + 6) & 0xc000 == 0xc000 {
                Some(self.port_inks(m, port)?)
            } else {
                None
            };
            for band in area.bands {
                for y in band.top..band.bottom {
                    for (left, right) in &band.spans {
                        for x in *left..*right {
                            let bit =
                                pattern[(y as i32 & 7) as usize] & (0x80 >> (x as i32 & 7)) != 0;
                            if let Some((fg, bg)) = inks {
                                if let Some((value, op)) = Self::pattern_pixel(bit, mode, fg, bg) {
                                    surface.write(m, x, y, value, op);
                                }
                                continue;
                            }
                            Self::transfer_ink(
                                m,
                                base,
                                row,
                                bounds,
                                x,
                                y,
                                if bit { foreground } else { background },
                                mode,
                            )?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
    pub fn frame_rect<B: AddressBus>(&self, m: &mut B, r: Rect) -> Result<(), String> {
        let port = self.get_port(m)?;
        if port == 0 {
            return Err("FrameRect without current port".into());
        }
        let height = m.read_word(port + 52) as i16;
        let width = m.read_word(port + 54) as i16;
        if width <= 0 || height <= 0 || (m.read_word(port + 66) as i16) < 0 {
            return Ok(());
        }
        let pattern = self.pattern(m, port, 58)?;
        let mode = m.read_word(port + 56);
        let inner = Rect {
            top: r.top.saturating_add(height),
            left: r.left.saturating_add(width),
            bottom: r.bottom.saturating_sub(height),
            right: r.right.saturating_sub(width),
        };
        let bitmap = self.port_bitmap(m, port)?;
        let bounds = read_rect(m, bitmap + 6);
        if let Some(bounds) = r.intersection(bounds) {
            for y in bounds.top..bounds.bottom {
                for x in bounds.left..bounds.right {
                    if !inner.contains(y, x) {
                        let source =
                            pattern[(y as i32 & 7) as usize] & (0x80 >> (x as i32 & 7)) != 0;
                        self.transfer(m, port, x, y, source, mode)?;
                    }
                }
            }
        }
        Ok(())
    }
    pub fn line_to<B: AddressBus>(&self,m:&mut B,h:i16,v:i16)->Result<(),String>{
        let port=self.get_port(m)?;
        if port==0{return Err("LineTo without current port".into());}
        let (mut x0,mut y0)=(i32::from(m.read_word(port+50)as i16),i32::from(m.read_word(port+48)as i16));
        let (mut x1,mut y1)=(i32::from(h),i32::from(v));
        let width=i32::from(m.read_word(port+54)as i16);let height=i32::from(m.read_word(port+52)as i16);
        if width<0||height<0||(width==0&&height==0)||(m.read_word(port+66)as i16)<0{return self.move_to(m,h,v);}
        let pattern=self.pattern(m,port,58)?;let mode=m.read_word(port+56);
        let bitmap=self.port_bitmap(m,port)?;let bounds=read_rect(m,bitmap+6);
        let dx=(x1-x0).abs();let dy=(y1-y0).abs();let mut points=Vec::with_capacity((dx.max(dy)+1)as usize);
        if dx>=dy{
            if x0>x1{std::mem::swap(&mut x0,&mut x1);std::mem::swap(&mut y0,&mut y1);}
            let step=if y1>=y0{1}else{-1};let mut error=(dx-dy)/2;let mut y=y0;
            for x in x0..=x1{points.push((x,y));error-=dy;if dx!=0&&error<=0{y+=step;error+=dx;}}
        }else{
            if y0>y1{std::mem::swap(&mut x0,&mut x1);std::mem::swap(&mut y0,&mut y1);}
            let step=if x1>=x0{1}else{-1};let mut error=(dy-dx)/2;let mut x=x0;
            for y in y0..=y1{points.push((x,y));error-=dx;if error<0{x+=step;error+=dy;}}
        }
        // Accumulate stroke boundaries, then transfer every covered pixel once.
        // Repeated rectangle stamping would incorrectly cancel thick XOR lines.
        // Retaining zero-width boundary intervals also preserves hairline sweeps.
        let transpose=height==0;let mut spans=std::collections::BTreeMap::<i32,(i32,i32)>::new();
        let (first,last,left,right)=if transpose{(i32::from(bounds.left),i32::from(bounds.right),i32::from(bounds.top),i32::from(bounds.bottom))}else{(i32::from(bounds.top),i32::from(bounds.bottom),i32::from(bounds.left),i32::from(bounds.right))};
        for (x,y)in points{
            let (row,start,rows,columns)=if transpose{(x,y,width,height)}else{(y,x,height,width)};
            for line in row.max(first)..(row+rows).min(last){
                let l=start.max(left);let r=(start+columns-1).min(right-1);
                spans.entry(line).and_modify(|span|{span.0=span.0.min(l);span.1=span.1.max(r);}).or_insert((l,r));
            }
        }
        for (row,(l,r))in spans{for column in l.max(left)..=r.min(right-1){
            let(x,y)=if transpose{(row,column)}else{(column,row)};
            let bit=pattern[(y&7)as usize]&(0x80>>(x&7))!=0;
            self.transfer(m,port,x as i16,y as i16,bit,mode)?;
        }}
        self.move_to(m,h,v)
    }
    pub fn begin_text_observation(&self) -> Result<(), String> {
        let mut current = self.text_observations.borrow_mut();
        if current.is_some() { return Err("text observation already active".into()); }
        *current = Some(Vec::new());
        Ok(())
    }
    pub fn take_text_observations(&self) -> Option<Vec<TextObservation>> {
        self.text_observations.borrow_mut().take()
    }
    pub fn text<B: AddressBus>(
        &self,
        m: &mut B,
        fonts: &crate::fonts::Fonts,
        text: &[u8],
    ) -> Result<(), String> {
        let port = self.get_port(m)?;
        if port == 0 {
            return Err("text drawing without current port".into());
        }
        let font = m.read_word(port + 68) as i16;
        let size = m.read_word(port + 74) as i16;
        let face = m.read_byte(port + 70);
        let mode = m.read_word(port + 72);
        if m.read_long(port + 76) != 0 {
            return Err("nonzero SpaceExtra is not implemented".into());
        }
        if let Some(observations) = self.text_observations.borrow_mut().as_mut() {
            let clip = m.read_long(port + 28);
            let data = m.read_long(clip);
            observations.push(TextObservation {
                bytes: text.to_vec(),
                point: [m.read_word(port + 50) as i16, m.read_word(port + 48) as i16],
                font, size, face, mode,
                clip: read_rect(m, data + 2),
            });
        }
        let info = fonts.select(font, size, face)?.info;
        let mut x = m.read_word(port + 50) as i16;
        let y = m.read_word(port + 48) as i16;
        for byte in text {
            let glyph = fonts.glyph(font, size, face, *byte)?;
            if mode == 0 {
                for dy in -info[0]..info[1] {
                    for dx in 0..glyph.advance.max(0) {
                        self.transfer(m, port, x.wrapping_add(dx), y.wrapping_add(dy), false, 0)?;
                    }
                }
            }
            if *byte >= 32 {
                for gy in 0..glyph.height {
                    for gx in 0..glyph.width {
                        if glyph.coverage[gy * glyph.width + gx] >= 128 {
                            self.transfer(
                                m,
                                port,
                                x.wrapping_add(glyph.left).wrapping_add(gx as i16),
                                y.wrapping_add(glyph.top).wrapping_add(gy as i16),
                                true,
                                mode,
                            )?;
                        }
                    }
                }
            }
            x = x.wrapping_add(glyph.advance);
        }
        m.write_word(port + 50, x as u16);
        Ok(())
    }
}

impl QuickDraw {
    pub fn surface<B: AddressBus>(
        &self,
        m: &mut B,
        bitmap: u32,
    ) -> Result<classic_core::pixels::Surface, String> {
        let row = m.read_word(bitmap + 4);
        let depth = if row & 0x8000 != 0 {
            m.read_word(bitmap + 32)
        } else {
            1
        };
        if !matches!(depth, 1 | 8) {
            return Err(format!("unsupported pixel depth {depth}"));
        }
        Ok(classic_core::pixels::Surface {
            base: m.read_long(bitmap),
            row: row & 0x3fff,
            bounds: read_rect(m, bitmap + 6),
            depth,
        })
    }
    pub fn port_surface<B: AddressBus>(
        &self,
        m: &mut B,
        port: u32,
    ) -> Result<classic_core::pixels::Surface, String> {
        let bitmap = self.port_bitmap(m, port)?;
        let mut surface = self.surface(m, bitmap)?;
        // Legacy screen GrafPorts carry a BitMap, not a PixMap. Their storage
        // follows the screen device depth; offscreen BitMaps remain one-bit.
        if m.read_word(port + 6) & 0xc000 != 0xc000 && surface.base == self.screen_base {
            surface.depth = self.depth;
        }
        Ok(surface)
    }
    pub fn palette<B: AddressBus>(&self, m: &mut B, bitmap: u32) -> Result<Vec<[u16; 3]>, String> {
        let h = m.read_long(bitmap + 42);
        if h == 0 {
            return Err("nil color table".into());
        }
        let p = m.read_long(h);
        let size = m.read_word(p + 6);
        if size > 255 {
            return Err("color table exceeds indexed eight-bit profile".into());
        }
        Ok((0..=size as u32)
            .map(|i| {
                [
                    m.read_word(p + 10 + i * 8),
                    m.read_word(p + 12 + i * 8),
                    m.read_word(p + 14 + i * 8),
                ]
            })
            .collect())
    }
    pub fn device_bitmap<B: AddressBus>(&self, m: &mut B) -> Result<u32, String> {
        let h = self.device.ok_or("graphics device not initialized")?;
        let p = m.read_long(h);
        let pm = m.read_long(p + 22);
        Ok(m.read_long(pm))
    }
    pub fn color_index<B: AddressBus>(&self, m: &mut B, color: [u16; 3]) -> Result<u8, String> {
        let device = m.read_long(0xcc8);
        if device == 0 {
            return Err("current graphics device not initialized".into());
        }
        let device_data = m.read_long(device);
        let pm = m.read_long(device_data + 22);
        let bitmap = m.read_long(pm);
        let h = m.read_long(bitmap + 42);
        let table = m.read_long(h);
        let seed = m.read_long(table);
        let mut cache = self.color_cache.borrow_mut();
        if cache
            .as_ref()
            .is_none_or(|(p, s, _)| *p != table || *s != seed)
        {
            *cache = Some((table, seed, Default::default()));
        }
        let map = &mut cache.as_mut().unwrap().2;
        if let Some(&index) = map.get(&color) {
            return Ok(index);
        }
        let palette = self.palette(m, bitmap)?;
        let ih = m.read_long(device_data + 6);
        let ip = if ih != 0 { m.read_long(ih) } else { 0 };
        let index = if ip != 0
            && self.inverse_handles.contains(&ih)
            && matches!(m.read_word(ip + 4), 3 | 4)
            && m.read_long(ip) == seed
        {
            if let Some(i) = palette.iter().position(|p| *p == color) {
                i as u8
            } else {
                let bits = m.read_word(ip + 4) as u32;
                let shift = 16 - bits;
                let position = ((color[0] as u32 >> shift) << (2*bits))
                    | ((color[1] as u32 >> shift) << bits)
                    | (color[2] as u32 >> shift);
                m.read_byte(ip + 6 + position)
            }
        } else if Some(device) == self.device && m.read_word(bitmap + 32) == 8 {
            // Native eight-bit main-device lookup uses a four-bit RGB grid.
            classic_core::pixels::inverse_lookup(color, &palette, 4).ok_or("empty color table")?
        } else {
            classic_core::pixels::nearest(color, &palette).ok_or("empty color table")?
        };
        if map.len() >= 512 {
            map.clear();
        }
        map.insert(color, index);
        Ok(index)
    }
    pub fn screen_colors<B: AddressBus>(&self, m: &mut B) -> Result<Vec<u32>, String> {
        if self.depth == 1 {
            return Ok(vec![0xffffff, 0]);
        }
        let b = self.device_bitmap(m)?;
        Ok(self
            .palette(m, b)?
            .into_iter()
            .map(|rgb| {
                ((rgb[0] as u32 >> 8) << 16) | ((rgb[1] as u32 >> 8) << 8) | (rgb[2] as u32 >> 8)
            })
            .collect())
    }
    pub fn has_depth(&self, depth: u16) -> u16 {
        match depth {
            1 => 1,
            8 if self.color_capable => 128,
            _ => 0,
        }
    }
    pub fn set_depth<B: AddressBus>(
        &mut self,
        m: &mut B,
        heap: &mut Heap,
        depth: u16,
    ) -> Result<(), i16> {
        let depth = if depth == 128 { 8 } else { depth };
        if self.has_depth(depth) == 0 {
            return Err(-50);
        }
        if depth == self.depth {
            return Ok(());
        }
        let handle = self.device.ok_or(-50i16)?;
        let p = m.read_long(handle);
        let pm = m.read_long(p + 22);
        let bitmap = m.read_long(pm);
        let table = m.read_long(bitmap + 42);
        let palette = if depth == 8 {
            classic_core::pixels::default_palette()
        } else {
            vec![[65535; 3], [0; 3]]
        };
        heap.resize(m, table, 8 + palette.len() as u32 * 8)?;
        let table_ptr = m.read_long(table);
        let seed = u32::from(depth); // canonical default palette identity
        m.write_long(table_ptr, seed);
        m.write_word(table_ptr + 4, 0x8000);
        m.write_word(table_ptr + 6, palette.len() as u16 - 1);
        for (i, rgb) in palette.iter().enumerate() {
            let entry = table_ptr + 8 + i as u32 * 8;
            m.write_word(entry, 0);
            for (j, v) in rgb.iter().enumerate() {
                m.write_word(entry + 2 + j as u32 * 2, *v);
            }
        }
        self.depth = depth;
        m.write_word(p + 4, 0); // indexed device
        let flags = m.read_word(p + 20);
        m.write_word(p + 20, if depth == 8 { flags | 1 } else { flags & !1 });
        m.write_long(p + 42, if depth == 8 { 128 } else { 1 });
        m.write_word(bitmap + 4, self.row_bytes() | 0x8000);
        m.write_word(bitmap + 32, depth);
        m.write_word(bitmap + 34, 1);
        m.write_word(bitmap + 36, depth);
        // Every port opened through this QuickDraw instance shares the live
        // screen storage unless its bitmap has been redirected by the guest.
        for &port in self.ports.borrow().iter() {
            let color = m.read_word(port + 6) & 0xc000 == 0xc000;
            let b = if color {
                let h = m.read_long(port + 2);
                m.read_long(h)
            } else {
                port + 2
            };
            if m.read_long(b) != self.screen_base {
                continue;
            }
            m.write_word(b + 4, self.row_bytes() | if color { 0x8000 } else { 0 });
            if color {
                m.write_word(b + 32, depth);
                m.write_word(b + 34, 1);
                m.write_word(b + 36, depth);
                m.write_long(b + 42, table);
            }
        }
        if let Some(slot) = self.slot {
            self.bitmap(m, slot - 122);
        }
        m.write_word(0x106, self.row_bytes());
        for i in 0..self.row_bytes() as u32 * self.height as u32 {
            m.write_byte(self.screen_base + i, 0);
        }
        Ok(())
    }
}

impl QuickDraw {
    pub fn port_rgb<B: AddressBus>(&self, m: &mut B, port: u32, foreground: bool) -> [u16; 3] {
        if m.read_word(port + 6) & 0xc000 == 0xc000 {
            let p = port + if foreground { 36 } else { 42 };
            [m.read_word(p), m.read_word(p + 2), m.read_word(p + 4)]
        } else {
            classic_core::legacy_colors::rgb(m.read_long(port + if foreground { 80 } else { 84 }))
                .unwrap_or([0; 3])
        }
    }
    fn port_inks<B: AddressBus>(&self, m: &mut B, port: u32) -> Result<(u8, u8), String> {
        let fg = self.port_rgb(m, port, true);
        let bg = self.port_rgb(m, port, false);
        let b = self.port_bitmap(m, port)?;
        let surface = self.port_surface(m, port)?;
        let device = self.device_bitmap(m)?;
        let color = m.read_word(port + 6) & 0xc000 == 0xc000;
        if surface.depth == self.depth
            && (!color || m.read_long(b + 42) == m.read_long(device + 42))
        {
            return Ok((self.color_index(m, fg)?, self.color_index(m, bg)?));
        }
        let table = if surface.depth == 1 {
            vec![[65535; 3], [0; 3]]
        } else {
            self.palette(m, b)?
        };
        Ok((
            classic_core::pixels::nearest(fg, &table).ok_or("empty port table")?,
            classic_core::pixels::nearest(bg, &table).ok_or("empty port table")?,
        ))
    }
    /// Indexed color pattern modes act on pattern coverage. XOR inverts the
    /// destination index, while Bic paints the background; RGB is not a raw
    /// index operand for these operations.
    fn pattern_pixel(mut bit: bool, mode: u16, fg: u8, bg: u8) -> Option<(u8, u16)> {
        if mode & 4 != 0 {
            bit = !bit;
        }
        match mode & 3 {
            0 => Some((if bit { fg } else { bg }, 0)),
            1 => {
                if bit {
                    Some((fg, 0))
                } else {
                    None
                }
            }
            2 => {
                if bit {
                    Some((255, 2))
                } else {
                    None
                }
            }
            3 => {
                if bit {
                    Some((bg, 0))
                } else {
                    None
                }
            }
            _ => unreachable!(),
        }
    }
}
