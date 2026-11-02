//! Original eight-color API replay using production ports and trap dispatch.
use crate::{machine::Memory, managers::Heap, quickdraw::QuickDraw};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const PORT: u32 = 0x1c000;
const STACK: u32 = 0x700000;
const COLORS: [u32; 8] = [30, 33, 205, 341, 409, 273, 137, 69];
struct Scene {
    m: Memory,
    heap: Heap,
    q: QuickDraw,
    cpu: CpuCore,
}
impl Scene {
    fn new(kind: u16) -> Result<Self, String> {
        let mut s = Self {
            m: Memory::zeroed(0x800000),
            heap: Heap::new(0x20000, 0x500000),
            q: QuickDraw::new(),
            cpu: CpuCore::new(),
        };
        s.q.color_capable = true;
        s.q.init(&mut s.m, 0x100000)?;
        s.q.device(&mut s.m, &mut s.heap)?;
        s.q.set_depth(&mut s.m, &mut s.heap, 8)
            .map_err(|e| e.to_string())?;
        if kind == 0 {
            s.q.open_port(&mut s.m, &mut s.heap, PORT)?;
        } else {
            s.q.open_color_port(&mut s.m, &mut s.heap, PORT)?;
        }
        s.q.set_port(&mut s.m, PORT)?;
        Ok(s)
    }
    fn call(&mut self, op: u16, value: u32, seed: u32, flags: u8) -> Result<(), String> {
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = STACK - 4;
        self.cpu.dar[0] = seed;
        self.cpu.set_ccr(flags);
        self.m.write_long(STACK, 0x55667788);
        self.m.write_long(STACK - 4, value);
        assert!(crate::colors::dispatch(
            op,
            &mut self.cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.q
        )?);
        assert_eq!(self.cpu.pc, 0x6002);
        Ok(())
    }
    fn bytes(&mut self) -> Vec<u8> {
        (0..108).map(|i| self.m.read_byte(PORT + i)).collect()
    }
}
pub fn probe(read: bool) -> Result<String, String> {
    let mut records = Vec::new();
    for kind in 0..2 {
        let mut s = Scene::new(kind)?;
        for op in 0..2 {
            for (i, color) in COLORS.into_iter().enumerate() {
                for seed in [0x12345678, 0] {
                    for flags in [16, 31, 0] {
                        s.call(0xa862, if read { color } else { 205 }, 0, 0)?;
                        s.call(0xa863, if read { COLORS[7 - i] } else { 273 }, 0, 0)?;
                        let mut before = s.bytes();
                        s.m.write_word(0x220, (-123i16) as u16);
                        s.m.write_word(0xa60, (-192i16) as u16);
                        s.m.write_long(0x19000, 0x10203040);
                        s.m.write_word(0x19004, 0x1234);
                        s.m.write_word(0x19006, 0x5678);
                        s.m.write_word(0x19008, 0x9abc);
                        s.m.write_long(0x1900a, 0x89abcdef);
                        s.call(
                            if read { 0xaa19 + op } else { 0xa862 + op },
                            if read { 0x19004 } else { color },
                            seed,
                            flags,
                        )?;
                        let mut after = s.bytes();
                        if !read {
                            before[80..88].fill(0);
                            after[80..88].fill(0);
                            if kind == 1 {
                                before[36..48].fill(0);
                                after[36..48].fill(0);
                            }
                        }
                        let mut r = json!({"port_kind":kind,"op":op,"color":color,"input_d0":seed,"input_ccr":flags,"d0":s.cpu.dar[0],"ccr":s.cpu.get_sr()&31,"stack_delta":i64::from(s.cpu.dar[15])-i64::from(STACK),"guard":s.m.read_long(STACK),"mem_error":s.m.read_word(0x220) as i16,"res_error":s.m.read_word(0xa60) as i16,"fg_color":s.m.read_long(PORT+80),"bg_color":s.m.read_long(PORT+84),"foreground":s.q.port_rgb(&mut s.m,PORT,true),"background":s.q.port_rgb(&mut s.m,PORT,false),"other_port_bytes_preserved":i32::from(before==after)});
                        if read {
                            r["result"] = json!([
                                s.m.read_word(0x19004),
                                s.m.read_word(0x19006),
                                s.m.read_word(0x19008)
                            ]);
                            r["result_before_guard"] = json!(s.m.read_long(0x19000));
                            r["result_after_guard"] = json!(s.m.read_long(0x1900a));
                        }
                        records.push(r);
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":if read{"color-read-runtime-raw-v1"}else{"legacy-color-runtime-raw-v1"},"depth":8,"records":records}).to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn undocumented_color_assignment_rejects_before_any_guest_write() {
        let mut s = Scene::new(1).unwrap();
        s.call(0xa862, 205, 0, 0).unwrap();
        let port = s.bytes();
        s.cpu.pc = 0x6000;
        s.cpu.dar[15] = STACK - 4;
        s.cpu.dar[0] = 0x12345678;
        s.cpu.set_ccr(31);
        s.m.write_long(STACK - 4, 0xdeadbeef);
        s.m.write_long(STACK, 0x55667788);
        s.m.write_word(0x220, 0xabcd);
        s.m.write_word(0xa60, 0x1234);
        let registers = s.cpu.dar;
        let sr = s.cpu.get_sr();
        assert!(
            crate::colors::dispatch(0xa862, &mut s.cpu, &mut s.m, &mut s.heap, &mut s.q).is_err()
        );
        assert_eq!(s.bytes(), port);
        assert_eq!(s.cpu.dar, registers);
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.get_sr(), sr);
        assert_eq!(s.m.read_long(STACK - 4), 0xdeadbeef);
        assert_eq!(s.m.read_long(STACK), 0x55667788);
        assert_eq!(s.m.read_word(0x220), 0xabcd);
        assert_eq!(s.m.read_word(0xa60), 0x1234);
    }
}
