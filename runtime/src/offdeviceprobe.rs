//! Independent allocation/lifecycle replay through production device services.
use crate::{
    callbacks::Arg,
    machine::Memory,
    managers::Heap,
    quickdraw::{self, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
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
        flags: u8,
        args: &[Arg],
        result: u32,
    ) -> Result<(u32, u16, i64, u32), String> {
        let mut sp = 0x700000;
        self.m.write_long(sp, 0x55667788);
        sp -= result;
        let end = sp;
        if result == 4 {
            self.m.write_long(sp, 0x7b7b7b7b);
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
        assert!(quickdraw::device_lifecycle_dispatch(
            op,
            &mut self.cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.q
        )?);
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.m.read_long(0x700000), 0x55667788);
        Ok((
            self.cpu.dar[0],
            self.cpu.get_sr() & 31,
            end as i64 - self.cpu.dar[15] as i64,
            if result == 4 {
                self.m.read_long(end)
            } else {
                0
            },
        ))
    }
    fn current(&mut self) -> Result<u32, String> {
        Ok(self.call(0xaa32, 0, &[], 4)?.3)
    }
    fn record(&mut self, h: u32) -> Result<Value, String> {
        let p = self.m.read_long(h);
        let pm = self.m.read_long(p + 22);
        let pp = self.m.read_long(pm);
        let it = self.m.read_long(p + 6);
        let ip = self.m.read_long(it);
        let table = self.m.read_long(pp + 42);
        let tp = self.m.read_long(table);
        let data = self.m.read_long(p + 50);
        let mask = self.m.read_long(p + 54);
        let isize = self.heap.size(it).map_err(|e| e.to_string())?;
        let tsize = self.heap.size(table).map_err(|e| e.to_string())?;
        let dsize = self.heap.size(data).map_err(|e| e.to_string())?;
        let msize = self.heap.size(mask).map_err(|e| e.to_string())?;
        let handles = [h, pm, it, data, mask, table];
        let distinct = handles.iter().all(|h| *h != 0)
            && handles
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                == 6;
        let dp = self.m.read_long(data);
        let mp = self.m.read_long(mask);
        let data_hex = hex(&mut self.m, dp, dsize);
        let mask_hex = hex(&mut self.m, mp, msize);
        Ok(
            json!({"gd_size":self.heap.size(h).map_err(|e|e.to_string())?,"gd_hex":hex(&mut self.m,p,62),"pm_size":self.heap.size(pm).map_err(|e|e.to_string())?,"pm_hex":hex(&mut self.m,pp,50),"itable_size":isize,"itable_seed":self.m.read_long(ip)as i32,"itable_res":if isize>=6{self.m.read_word(ip+4)as i16}else{-1},"table_size":tsize,"table_hex":hex(&mut self.m,tp,tsize),"ccdata_size":dsize,"ccmask_size":msize,"ccdata_hex":data_hex,"ccmask_hex":mask_hex,"children_distinct":i32::from(distinct)}),
        )
    }
}
pub fn probe() -> Result<String, String> {
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        heap: Heap::new(0x20000, 0x500000),
        q: QuickDraw::new(),
        cpu: CpuCore::new(),
    };
    s.q.init(&mut s.m, 0x100000)?;
    let main = s.current()?;
    let list = s.m.read_long(0x8a8);
    // Reserve master slots once so free-byte accounting isolates payload lifetime.
    s.heap.more_masters(&mut s.m).map_err(|e| e.to_string())?;
    let free = s.heap.free_bytes();
    let mut devices = Vec::new();
    let mut records = Vec::new();
    for refnum in [0i16, -1, 42, -51] {
        for flags in [16, 31, 0] {
            let (d0, ccr, delta, h) = s.call(
                0xaa2f,
                flags,
                &[Arg::Word(refnum as u16), Arg::Long(u32::MAX)],
                4,
            )?;
            let mut record = s.record(h)?;
            let observed = json!({"refnum":refnum,"mode":-1,"input_ccr":flags,"d0":d0,"ccr":ccr,"stack_delta":delta,"new_nonnull":i32::from(h!=0),"distinct_main":i32::from(h!=main),"distinct_previous":i32::from(!devices.contains(&h)),"current_same":i32::from(s.current()?==main),"list_same":i32::from(s.m.read_long(0x8a8)==list)});
            record
                .as_object_mut()
                .unwrap()
                .extend(observed.as_object().unwrap().clone());
            records.push(record);
            devices.push(h);
        }
    }
    let mut tables = Vec::new();
    for (index, &device) in devices.iter().take(3).enumerate() {
        let flags = [16, 31, 0][index];
        let replacement = s
            .heap
            .new_handle(&mut s.m, 32, true)
            .map_err(|e| e.to_string())?;
        let dp = s.m.read_long(device);
        let pm = s.m.read_long(dp + 22);
        let pp = s.m.read_long(pm);
        let old = s.m.read_long(pp + 42);
        let (d0, ccr, delta, _) = s.call(0xaa24, flags, &[Arg::Long(old)], 0)?;
        assert!(s.heap.data(old).is_err());
        s.m.write_long(pp + 42, replacement);
        tables.push(json!({"index":index,"input_ccr":flags,"d0":d0,"ccr":ccr,"stack_delta":delta,"replacement_distinct":i32::from(replacement!=0&&replacement!=old),"current_same":i32::from(s.current()?==main),"list_same":i32::from(s.m.read_long(0x8a8)==list)}));
    }
    let mut switches = Vec::new();
    for (target, h) in [devices[0], devices[1], main].into_iter().enumerate() {
        for flags in [16, 31, 0] {
            let (d0, ccr, delta, _) = s.call(0xaa31, flags, &[Arg::Long(h)], 0)?;
            switches.push(json!({"target":target,"input_ccr":flags,"d0":d0,"ccr":ccr,"stack_delta":delta,"selected_same":i32::from(s.current()?==h),"list_same":i32::from(s.m.read_long(0x8a8)==list)}));
        }
    }
    let mut disposals = Vec::new();
    for (index, &h) in devices.iter().enumerate() {
        let flags = [16, 31, 0][index % 3];
        let (d0, ccr, delta, _) = s.call(0xaa30, flags, &[Arg::Long(h)], 0)?;
        assert!(s.heap.data(h).is_err());
        disposals.push(json!({"index":index,"input_ccr":flags,"d0":d0,"ccr":ccr,"stack_delta":delta,"current_same":i32::from(s.current()?==main),"list_same":i32::from(s.m.read_long(0x8a8)==list)}));
    }
    assert_eq!(
        s.heap.free_bytes(),
        free,
        "offscreen payloads or replacement tables leaked"
    );
    Ok(json!({"schema":1,"probe":"off-device-runtime-raw-v1","records":records,"tables":tables,"switches":switches,"disposals":disposals}).to_string())
}
