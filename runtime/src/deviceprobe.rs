//! Public single-display selection replay; no expected observations imported.
use crate::{
    machine::Memory,
    managers::{Heap, Rect},
    quickdraw::{self, read_rect, write_rect, QuickDraw},
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut heap = Heap::new(0x20000, 0x500000);
    let mut q = QuickDraw::new();
    q.init(&mut m, 0x100000)?;
    let main = q.device(&mut m, &mut heap)?;
    let p = m.read_long(main);
    let s = read_rect(&mut m, p + 34);
    let l = s.left;
    let t = s.top;
    let r = s.right;
    let b = s.bottom;
    let coords = [
        [t + 10, l + 10, t + 40, l + 40],
        [t, l, b, r],
        [t - 20, l - 20, t, l],
        [t, r, t + 20, r + 20],
        [t + 10, l - 10, t + 20, l + 10],
        [t + 10, r - 10, t + 20, r + 10],
        [b, l + 10, b + 10, l + 20],
        [b - 1, l + 10, b + 10, l + 20],
        [t + 10, l + 10, t + 20, l + 10],
        [t + 10, l + 10, t + 10, l + 20],
        [t + 20, l + 20, t + 10, l + 10],
        [-1000, -1000, -900, -900],
    ];
    let mut cases = Vec::new();
    for (kind, rect) in coords.into_iter().enumerate() {
        for flags in [16, 31, 0] {
            let [top, left, bottom, right] = rect;
            write_rect(
                &mut m,
                0x120000,
                Rect {
                    top,
                    left,
                    bottom,
                    right,
                },
            );
            let mut cpu = CpuCore::new();
            cpu.pc = 0x6000;
            cpu.dar[0] = 0x12345678;
            cpu.set_ccr(flags);
            cpu.dar[15] = 0x700000 - 8;
            m.write_long(cpu.dar[15], 0x120000);
            m.write_long(0x700000 - 4, 0x7b7b7b7b);
            m.write_long(0x700000, 0x55667788);
            assert!(quickdraw::max_device_dispatch(
                0xaa27, &mut cpu, &mut m, &mut heap, &mut q
            )?);
            assert_eq!(cpu.pc, 0x6002);
            assert_eq!(m.read_long(0x700000), 0x55667788);
            let selected = m.read_long(cpu.dar[15]);
            cases.push(json!({"kind":kind,"input_ccr":flags,"rect":rect,"d0":cpu.dar[0],"ccr":cpu.get_ccr(),"stack_delta":0x700000i64-cpu.dar[15]as i64-4,"selected_role":if selected==0{0}else if selected==main{1}else{2},"main_same":i32::from(selected==main),"null":i32::from(selected==0),"current_same":i32::from(m.read_long(0xcc8)==main)}));
        }
    }
    Ok(json!({"schema":1,"probe":"max-device-v1","devices":{"screen":[s.top,s.left,s.bottom,s.right],"main_nonnull":main!=0,"cases":cases}}).to_string())
}
