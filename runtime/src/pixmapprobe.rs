//! Public pixel-map services exercised with owned data and real trap dispatch.
//! The32bit case supplies a public current-device descriptor only; the hosted
//! display backend remains indexed8bit. This does not exercise32bit rendering.
use crate::{
    callbacks::Arg,
    machine::Memory,
    managers::Heap,
    quickdraw::{self, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
struct Scene {
    m: Memory,
    heap: Heap,
    q: QuickDraw,
    cpu: CpuCore,
    device_pm: u32,
}
impl Scene {
    fn call(
        &mut self,
        op: u16,
        args: &[Arg],
        result: u32,
        flags: u8,
    ) -> Result<(u32, Value), String> {
        let mut sp = 0x700000;
        self.m.write_long(sp, 0x55667788);
        sp -= result;
        let end = sp;
        if result == 4 {
            self.m.write_long(sp, 0x7b7b7b7b);
        }
        for arg in args {
            match arg {
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
        if !quickdraw::device_lifecycle_dispatch(
            op,
            &mut self.cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.q,
        )? {
            return Err(format!("unsupported PixMap probe trap{op:#x}"));
        }
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.cpu.dar[15], end);
        let abi = json!({"d0":self.cpu.dar[0],"ccr":self.cpu.get_sr()&31,"stack_delta":self.cpu.dar[15]-sp,"guard":self.m.read_long(end+result),"memory_error":self.m.read_word(0x220)as i16});
        Ok((self.m.read_long(end), abi))
    }
    fn hex(&mut self, p: u32, n: u32) -> String {
        (0..n)
            .map(|i| format!("{:02x}", self.m.read_byte(p + i)))
            .collect()
    }
    fn snapshot(&mut self, h: u32) -> Result<Value, String> {
        let p = self.heap.data(h).map_err(|e| e.to_string())?;
        let dp = self.heap.data(self.device_pm).map_err(|e| e.to_string())?;
        let table = self.m.read_long(p + 42);
        let size = self.heap.size(h).map_err(|e| e.to_string())?;
        let state = self.heap.state(h).map_err(|e| e.to_string())?;
        let ts = if table != 0 {
            self.heap.size(table).map_err(|e| e.to_string())?
        } else {
            0
        };
        let table_state = if table != 0 {
            self.heap.state(table).map_err(|e| e.to_string())? as i16
        } else {
            -1
        };
        let tp = if table != 0 {
            self.heap.data(table).map_err(|e| e.to_string())?
        } else {
            0
        };
        Ok(
            json!({"size":size,"state":state,"base_zero":(self.m.read_long(p)==0)as i32,"base_is_device":(self.m.read_long(p)==self.m.read_long(dp))as i32,"table_present":(table!=0)as i32,"table_is_device":(table==self.m.read_long(dp+42))as i32,"table_size":ts,"table_state":table_state,"map_hex":self.hex(p,50),"table_hex":self.hex(tp,ts)}),
        )
    }
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    q.color_capable = true;
    let gd = q.device(&mut m, &mut heap)?;
    q.set_depth(&mut m, &mut heap, 8)
        .map_err(|e| e.to_string())?;
    let gp = heap.data(gd).map_err(|e| e.to_string())?;
    let device_pm = m.read_long(gp + 22);
    let mut s = Scene {
        m,
        heap,
        q,
        cpu: CpuCore::new(),
        device_pm,
    };
    let mut rows = Vec::new();
    for depth in [8u16, 32] {
        // Model the current device's public descriptor.32bit storage is never read
        // or rendered: these services copy metadata and caller-owned bytes only.
        let dp = s.heap.data(device_pm).map_err(|e| e.to_string())?;
        s.m.write_word(dp + 4, 0x8000 | s.q.width as u16 * (depth / 8));
        s.m.write_word(dp + 30, if depth == 32 { 16 } else { 0 });
        s.m.write_word(dp + 32, depth);
        s.m.write_word(dp + 34, if depth == 32 { 3 } else { 1 });
        s.m.write_word(dp + 36, 8);
        for flags in [16u8, 31, 0] {
            let pixels = s
                .heap
                .new_ptr(&mut s.m, 64, false)
                .map_err(|e| e.to_string())?;
            for i in 0..64 {
                s.m.write_byte(pixels + i, (i as u8) ^ 0x5a);
            }
            let before = s.heap.free_bytes();
            let (h, new_abi) = s.call(0xaa03, &[], 4, flags)?;
            let initial = s.snapshot(h)?;
            let (other, _) = s.call(0xaa03, &[], 4, 0)?;
            let p = s.heap.data(h).map_err(|e| e.to_string())?;
            let op = s.heap.data(other).map_err(|e| e.to_string())?;
            let oldtable = s.m.read_long(op + 42);
            let sourceold = s.m.read_long(p + 42);
            s.call(0xaa24, &[Arg::Long(sourceold)], 0, 0)?;
            // Same owned palette fixture as the native probe; GetCTable's ownership
            // conversion is independently covered by the existing native comparison.
            let colors = [
                0x10, 0x20, 0x30, 0x40, 0, 0, 0, 1, 0, 0, 255, 255, 0x12, 0x34, 0x56, 0x78, 0, 1,
                0xab, 0xcd, 0x23, 0x45, 0x67, 0x89,
            ];
            let table = s
                .heap
                .new_handle(&mut s.m, 24, false)
                .map_err(|e| e.to_string())?;
            let tp = s.heap.data(table).map_err(|e| e.to_string())?;
            for (i, b) in colors.iter().enumerate() {
                s.m.write_byte(tp + i as u32, *b);
            }
            s.m.write_long(p + 42, table);
            s.m.write_long(p, pixels);
            s.m.write_word(p + 4, 0x8008);
            for (i, v) in [1u16, 2, 3, 4].iter().enumerate() {
                s.m.write_word(p + 6 + 2 * i as u32, *v);
            }
            s.m.write_long(p + 22, 90 << 16);
            s.m.write_long(p + 26, 96 << 16);
            s.m.write_word(p + 32, 8);
            s.m.write_word(p + 34, 1);
            s.m.write_word(p + 36, 8);
            let (_, copy_abi) = s.call(0xaa05, &[Arg::Long(h), Arg::Long(other)], 0, flags)?;
            let copied = s.snapshot(other)?;
            let desttable = s.m.read_long(op + 42);
            let copied_base = s.m.read_long(op) == pixels;
            s.m.write_word(tp + 10, 0x7777);
            let dtp = s.heap.data(desttable).map_err(|e| e.to_string())?;
            let n = s.heap.size(desttable).map_err(|e| e.to_string())?;
            let copy_after = s.hex(dtp, n);
            let (_, dispose_abi) = s.call(0xaa04, &[Arg::Long(other)], 0, flags)?;
            s.call(0xaa04, &[Arg::Long(h)], 0, 0)?;
            let size = s.heap.ptr_size(pixels).map_err(|e| e.to_string())?;
            let balanced = s.heap.free_bytes() == before;
            let base_bytes = s.hex(pixels, 64);
            s.heap.dispose_ptr(pixels).map_err(|e| e.to_string())?;
            rows.push(json!({"depth":depth,"input_ccr":flags,"new_abi":new_abi,"initial":initial,"copy_abi":copy_abi,"copied":copied,"distinct_maps":(h!=other)as i32,"distinct_tables":(table!=desttable)as i32,"destination_table_reused":(oldtable==desttable)as i32,"copied_base_same":copied_base as i32,"copy_table_after_source_mutation":copy_after,"dispose_abi":dispose_abi,"base_size_after_dispose":size,"free_balanced":balanced as i32,"base_bytes_after_dispose":base_bytes}));
        }
    }
    let mut nil = Vec::new();
    for flags in [16u8, 31, 0] {
        let (_, abi) = s.call(0xaa04, &[Arg::Long(0)], 0, flags)?;
        nil.push(json!({"input_ccr":flags,"abi":abi}));
    }
    Ok(serde_json::to_string_pretty(
        &json!({"schema":1,"probe":"pixmap-raw-v1","records":rows,"nil_dispose":nil}),
    )
    .unwrap())
}
