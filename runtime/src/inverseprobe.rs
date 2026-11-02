//! Independent Color Manager/port replay; native observations are not inputs.
use crate::{
    callbacks::Arg,
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, read_rect, write_rect, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
fn rect(r: Rect) -> [i16; 4] {
    [r.top, r.left, r.bottom, r.right]
}
fn palette(kind: usize) -> Vec<[u16; 3]> {
    let n = [2, 4, 8, 16, 4, 4, 4, 6][kind];
    (0..n)
        .map(|i| match kind {
            0 => [if i == 0 { 65535 } else { 0 }; 3],
            1 | 5 | 6 => {
                let which = if kind == 5 {
                    [3, 2, 1, 0][i]
                } else if kind == 6 {
                    [1, 3, 0, 2][i]
                } else {
                    i
                };
                [
                    if which == 0 || which == 3 { 65535 } else { 0 },
                    if which == 1 || which == 3 { 65535 } else { 0 },
                    if which == 2 || which == 3 { 65535 } else { 0 },
                ]
            }
            2 => [
                if i & 4 != 0 { 65535 } else { 0 },
                if i & 2 != 0 { 65535 } else { 0 },
                if i & 1 != 0 { 65535 } else { 0 },
            ],
            3 => [65535 - i as u16 * 4369; 3],
            4 => [
                [0x1234, 0x5678, 0x9abc],
                [0xf000, 0x7000, 0x1000],
                [0x2222, 0xdddd, 0x5555],
                [0x9999, 0x3333, 0xeeee],
            ][i],
            _ => [
                [0x2000, 0x6000, 0xa000],
                [0xe000, 0x6000, 0x2000],
                [0x2000, 0xc000, 0x4000],
                [0xa000, 0x4000, 0xe000],
                [0, 0, 0],
                [65535; 3],
            ][i],
        })
        .collect()
}
struct Scene {
    m: Memory,
    heap: Heap,
    q: QuickDraw,
    cpu: CpuCore,
}
impl Scene {
    fn call(
        &mut self,
        op: u16,
        args: &[Arg],
        result: u32,
        flags: u8,
    ) -> Result<(u32, u16, i64, u32), String> {
        let mut sp = 0x700000;
        self.m.write_long(sp, 0x55667788);
        sp -= result;
        let end = sp;
        if result == 4 {
            self.m.write_long(sp, 0x7b7b7b7b);
        } else if result == 2 {
            self.m.write_word(sp, 0x7b7b);
        }
        for a in args {
            match a {
                Arg::Word(v) => {
                    sp -= 2;
                    self.m.write_word(sp, *v)
                }
                Arg::Long(v) => {
                    sp -= 4;
                    self.m.write_long(sp, *v)
                }
            }
        }
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = sp;
        self.cpu.dar[0] = 0x12345678;
        self.cpu.set_ccr(flags);
        if !crate::colors::dispatch(op, &mut self.cpu, &mut self.m, &mut self.heap, &mut self.q)?
            && !quickdraw::device_lifecycle_dispatch(
                op,
                &mut self.cpu,
                &mut self.m,
                &mut self.heap,
                &mut self.q,
            )?
        {
            return Err("inverse probe API not dispatched".into());
        }
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.m.read_long(0x700000), 0x55667788);
        assert_eq!(self.cpu.dar[15], end);
        Ok((
            self.cpu.dar[0],
            self.cpu.get_sr() & 31,
            end as i64 - self.cpu.dar[15] as i64,
            if result == 4 {
                self.m.read_long(end)
            } else if result == 2 {
                self.m.read_word(end) as u32
            } else {
                0
            },
        ))
    }
    fn index(&mut self, rgb: [u16; 3]) -> Result<u32, String> {
        for (i, v) in rgb.into_iter().enumerate() {
            self.m.write_word(0x120000 + i as u32 * 2, v);
        }
        Ok(self.call(0xaa33, &[Arg::Long(0x120000)], 4, 0)?.3)
    }
    fn exercise(&mut self, ct: u32, it: u32, n: usize, bits: u16) -> Result<Value, String> {
        let main = self.call(0xaa32, &[], 4, 0)?.3;
        let list = self.m.read_long(0x8a8);
        let gd = self
            .call(0xaa2f, &[Arg::Word(0), Arg::Long(u32::MAX)], 4, 0)?
            .3;
        let p = self.m.read_long(gd);
        let pmh = self.m.read_long(p + 22);
        let pm = self.m.read_long(pmh);
        let oldit = self.m.read_long(p + 6);
        self.heap
            .dispose(&mut self.m, oldit)
            .map_err(|e| e.to_string())?;
        self.m.write_long(p + 6, it);
        let oldct = self.m.read_long(pm + 42);
        self.call(0xaa24, &[Arg::Long(oldct)], 0, 0)?;
        self.m.write_long(pm + 42, ct);
        let pixels = self
            .heap
            .new_ptr(&mut self.m, 256, true)
            .map_err(|e| e.to_string())?;
        self.m.write_long(pm, pixels);
        self.m.write_word(pm + 4, 0x8010);
        let bounds = Rect {
            top: 10,
            left: 20,
            bottom: 26,
            right: 36,
        };
        write_rect(&mut self.m, pm + 6, bounds);
        write_rect(&mut self.m, p + 34, bounds);
        self.m.write_long(pm + 22, 72 << 16);
        self.m.write_long(pm + 26, 72 << 16);
        self.m.write_word(pm + 32, 8);
        self.m.write_word(pm + 34, 1);
        self.m.write_word(pm + 36, 8);
        self.m.write_word(p + 10, bits);
        self.m.write_word(p + 20, 0x4401);
        self.call(0xaa31, &[Arg::Long(gd)], 0, 0)?;
        let mut indices = Vec::new();
        for r in 0..(1u16 << bits) {
            for g in 0..(1u16 << bits) {
                for b in 0..(1u16 << bits) {
                    indices.push(self.index([
                        r << (16 - bits),
                        g << (16 - bits),
                        b << (16 - bits),
                    ])?);
                }
            }
        }
        let mut exact = Vec::new();
        for i in 0..n {
            let cp = self.m.read_long(ct) + 10 + i as u32 * 8;
            let rgb = [
                self.m.read_word(cp),
                self.m.read_word(cp + 2),
                self.m.read_word(cp + 4),
            ];
            exact.push(self.index(rgb)?);
        }
        let old = self.q.get_port(&mut self.m)?;
        let port = 0x110000;
        self.call(0xaa00, &[Arg::Long(port)], 0, 0)?;
        let ph = self.m.read_long(port + 2);
        let pp = self.m.read_long(ph);
        let vh = self.m.read_long(port + 24);
        let ch = self.m.read_long(port + 28);
        let vp = self.m.read_long(vh);
        let cp = self.m.read_long(ch);
        let vn = self.m.read_word(vp) as u32;
        let cn = self.m.read_word(cp) as u32;
        let port_value = json!({"row_bytes":self.m.read_word(pp+4),"pixel_size":self.m.read_word(pp+32),"base_same":i32::from(self.m.read_long(pp)==pixels),"table_same":i32::from(self.m.read_long(pp+42)==ct),"map_distinct":i32::from(ph!=pmh),"pen_indices":[self.m.read_long(port+80),self.m.read_long(port+84)],"rgb_fg":[self.m.read_word(port+36),self.m.read_word(port+38),self.m.read_word(port+40)],"rgb_bg":[self.m.read_word(port+42),self.m.read_word(port+44),self.m.read_word(port+46)],"port_rect":rect(read_rect(&mut self.m,port+16)),"bitmap_bounds":rect(read_rect(&mut self.m,pp+6)),"visible_hex":hex(&mut self.m,vp,vn),"clip_hex":hex(&mut self.m,cp,cn)});
        self.call(0xaa02, &[Arg::Long(port)], 0, 0)?;
        self.q.set_port(&mut self.m, old)?;
        let current = i32::from(self.call(0xaa32, &[], 4, 0)?.3 == gd);
        let same_list = i32::from(self.m.read_long(0x8a8) == list);
        self.call(0xaa31, &[Arg::Long(main)], 0, 0)?;
        self.call(0xaa30, &[Arg::Long(gd)], 0, 0)?;
        self.heap.dispose_ptr(pixels).map_err(|e| e.to_string())?;
        Ok(
            json!({"indices":indices,"exact_indices":exact,"port":port_value,"current_same":current,"list_same":same_list}),
        )
    }
}
pub fn probe() -> Result<String, String> {
    probe_bits(3)
}
pub fn probe_four() -> Result<String, String> {
    probe_bits(4)
}
fn probe_bits(bits: u16) -> Result<String, String> {
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        heap: Heap::new(0x20000, 0x500000),
        q: QuickDraw::new(),
        cpu: CpuCore::new(),
    };
    s.q.init(&mut s.m, 0x100000)?;
    s.call(0xaa32, &[], 4, 0)?;
    let mut main_table = String::new();
    let mut main_indices = Vec::new();
    if bits == 4 {
        s.q.color_capable = true;
        s.q.set_depth(&mut s.m, &mut s.heap, 8)
            .map_err(|e| e.to_string())?;
        let pm = s.q.device_bitmap(&mut s.m)?;
        let h = s.m.read_long(pm + 42);
        let p = s.m.read_long(h);
        main_table = hex(&mut s.m, p, 2056);
        for r in 0..16 {
            for g in 0..16 {
                for b in 0..16 {
                    main_indices.push(s.index([r << 12, g << 12, b << 12])?);
                }
            }
        }
    }
    let free = s.heap.free_bytes();
    let mut records = Vec::new();
    for kind in 0..8 {
        for device in 0..2 {
            for seed in 0..2 {
                for flags in [16u8, 31, 0] {
                    let pal = palette(kind);
                    let size = 8 + pal.len() as u32 * 8;
                    let ct = s
                        .heap
                        .new_handle(&mut s.m, size, true)
                        .map_err(|e| e.to_string())?;
                    let p = s.m.read_long(ct);
                    s.m.write_long(
                        p,
                        if seed == 0 {
                            0
                        } else {
                            0x12340000 + kind as u32
                        },
                    );
                    s.m.write_word(p + 4, if device == 0 { 0 } else { 0x8000 });
                    s.m.write_word(p + 6, pal.len() as u16 - 1);
                    for (i, rgb) in pal.iter().enumerate() {
                        s.m.write_word(
                            p + 8 + i as u32 * 8,
                            if kind == 1 {
                                (i * 7 + 1) as u16
                            } else {
                                i as u16
                            },
                        );
                        for (j, v) in rgb.iter().enumerate() {
                            s.m.write_word(p + 10 + i as u32 * 8 + j as u32 * 2, *v);
                        }
                    }
                    let it = s
                        .heap
                        .new_handle(&mut s.m, 4, true)
                        .map_err(|e| e.to_string())?;
                    let guard = s
                        .heap
                        .new_handle(&mut s.m, 37, false)
                        .map_err(|e| e.to_string())?;
                    let gp = s.m.read_long(guard);
                    for i in 0..37 {
                        s.m.write_byte(gp + i, (i * 19 + 7) as u8);
                    }
                    let before = hex(&mut s.m, p, size);
                    let (d0, ccr, delta, _) = s.call(
                        0xaa39,
                        &[Arg::Long(ct), Arg::Long(it), Arg::Word(bits)],
                        0,
                        flags,
                    )?;
                    let error = s.call(0xaa40, &[], 2, 0)?.3;
                    let itsize = s.heap.size(it).map_err(|e| e.to_string())?;
                    let ip = s.m.read_long(it);
                    let after_ptr = s.m.read_long(ct);
                    let after = hex(&mut s.m, after_ptr, size);
                    let ith = hex(&mut s.m, ip, itsize);
                    let mut record = json!({"kind":kind,"device":device,"seed":seed,"input_ccr":flags,"resolution":bits,"d0":d0,"ccr":ccr,"stack_delta":delta,"qd_error":error,"itable_size":itsize,"before_hex":before,"after_hex":after,"itable_hex":ith});
                    let sampled = s.exercise(ct, it, pal.len(), bits)?;
                    record
                        .as_object_mut()
                        .unwrap()
                        .extend(sampled.as_object().unwrap().clone());
                    let gp = s.m.read_long(guard);
                    record["guard_hex"] = json!(hex(&mut s.m, gp, 37));
                    s.heap.dispose(&mut s.m, guard).map_err(|e| e.to_string())?;
                    records.push(record);
                }
            }
        }
    }
    assert_eq!(
        s.heap.free_bytes(),
        free,
        "inverse/port replay leaked guest payloads"
    );
    if bits == 4 {
        Ok(json!({"schema":1,"probe":"inverse-four-runtime-raw-v1","main_table_hex":main_table,"main_indices":main_indices,"records":records}).to_string())
    } else {
        Ok(json!({"schema":1,"probe":"inverse-runtime-raw-v1","records":records}).to_string())
    }
}
