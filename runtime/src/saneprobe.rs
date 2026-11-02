//! Shared SANE CPU/package replay with owned packed operands and real buffers.
use crate::machine::Memory;
use classic_core::sane::Sane;
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
include!("../../probes/sane/operand_counts.rs");
const OPERANDS: &[u8] = include_bytes!("../../probes/sane/operands.bin");
const WORD: u32 = 0x19000;
const VALUE: u32 = 0x1a004;
const SINGLE: u32 = 0x1b004;
fn hex(m: &mut Memory, p: u32) -> String {
    (0..10)
        .map(|i| format!("{:02x}", m.read_byte(p + i)))
        .collect()
}
struct Scene {
    m: Memory,
    sane: Sane,
    cpu: CpuCore,
}
impl Scene {
    fn call(
        &mut self,
        package: u16,
        selector: u16,
        args: &[u32],
        flags: u8,
    ) -> Result<(u32, u16, i64), String> {
        let mut sp = 0x700000;
        self.m.write_long(sp, 0x55667788);
        for &arg in args {
            sp -= 4;
            self.m.write_long(sp, arg);
        }
        sp -= 2;
        self.m.write_word(sp, selector);
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = sp;
        self.cpu.dar[0] = 0x12345678;
        self.cpu.set_ccr(flags);
        let before = self.cpu.dar;
        assert!(self
            .sane
            .dispatch(package, &mut self.cpu, &mut self.m)
            .map_err(|e| e.to_string())?);
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.m.read_long(0x700000), 0x55667788);
        assert_eq!(self.cpu.dar[15], 0x700000);
        assert_eq!(&self.cpu.dar[..15], &before[..15]);
        Ok((
            self.cpu.dar[0],
            self.cpu.get_sr() & 31,
            0x700000i64 - self.cpu.dar[15] as i64,
        ))
    }
    fn env(&mut self, value: Option<u16>) -> Result<u16, String> {
        self.m.write_word(WORD, value.unwrap_or(0x5a5a));
        self.call(0xa9eb, if value.is_some() { 1 } else { 3 }, &[WORD], 0)?;
        Ok(self.m.read_word(WORD))
    }
    fn measure(
        &mut self,
        package_selector: u16,
        kind: usize,
        input: &[u8],
        mode: u16,
        flags: u8,
    ) -> Result<Value, String> {
        self.m.write_long(VALUE - 4, 0x11223344);
        self.m.write_long(VALUE + 10, 0x55667788);
        for (i, &v) in input.iter().enumerate() {
            self.m.write_byte(VALUE + i as u32, v);
        }
        let before = hex(&mut self.m, VALUE);
        self.env(Some(mode))?;
        let (d0, ccr, delta) = self.call(0xa9ec, package_selector, &[VALUE], flags)?;
        let env = self.env(None)?;
        self.env(Some(0))?;
        self.m.write_long(SINGLE - 4, 0x99aabbcc);
        self.m.write_long(SINGLE + 4, 0xddeeff00);
        self.m.write_long(SINGLE, 0x7b7b7b7b);
        self.call(0xa9eb, 0x1010, &[VALUE, SINGLE], 0)?;
        let single_env = self.env(None)?;
        Ok(
            json!({"kind":kind,"input_env":mode,"input_ccr":flags,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[self.m.read_long(VALUE-4),self.m.read_long(VALUE+10)],"before_hex":before,"after_hex":hex(&mut self.m,VALUE),"single_bits":self.m.read_long(SINGLE),"single_guards":[self.m.read_long(SINGLE-4),self.m.read_long(SINGLE+4)],"single_env":single_env}),
        )
    }
}
pub fn probe() -> Result<String, String> {
    assert_eq!(OPERANDS.len(), (BASE_COUNT + DENSE_COUNT) * 10);
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let initial = s.env(None)?;
    let mut controls = Vec::new();
    for mode in [
        0u16, 0x20, 0x40, 0x2000, 0x4000, 0x6000, 0x0100, 0x1000, 0x1f00, 0x001f, 0xffff,
    ] {
        for flags in [16, 31, 0] {
            for selector in [1u16, 3] {
                s.m.write_word(WORD, if selector == 1 { mode } else { 0x5a5a });
                let (d0, ccr, delta) = s.call(0xa9eb, selector, &[WORD], flags)?;
                controls.push(json!({"input_env":mode,"input_ccr":flags,"selector":selector,"word":s.m.read_word(WORD),"d0":d0,"ccr":ccr,"stack_delta":delta}));
            }
        }
    }
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut atan = Vec::new();
    for (kind, input) in OPERANDS.chunks_exact(10).take(BASE_COUNT).enumerate() {
        for mode in modes {
            for flags in [16, 31, 0] {
                atan.push(s.measure(0x1e, kind, input, mode, flags)?);
            }
        }
    }
    let mut dense = Vec::new();
    for (kind, input) in OPERANDS.chunks_exact(10).skip(BASE_COUNT).enumerate() {
        dense.push(s.measure(0x1e, kind, input, 0, 16)?);
    }
    Ok(json!({"schema":1,"probe":"sane-runtime-raw-v1","initial_env":initial,"controls":controls,"atan":atan,"dense":dense}).to_string())
}
pub fn sine_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanesine/operands.bin");
    let base_count = 62;
    let dense_count = 2178;
    assert_eq!(operands.len(), (base_count + dense_count) * 10);
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let initial = s.env(None)?;
    let mut controls = Vec::new();
    for mode in [
        0u16, 0x20, 0x40, 0x2000, 0x4000, 0x6000, 0x0100, 0x1000, 0x1f00, 0x001f, 0xffff,
    ] {
        for flags in [16, 31, 0] {
            for selector in [1u16, 3] {
                s.m.write_word(WORD, if selector == 1 { mode } else { 0x5a5a });
                let (d0, ccr, delta) = s.call(0xa9eb, selector, &[WORD], flags)?;
                controls.push(json!({"input_env":mode,"input_ccr":flags,"selector":selector,"word":s.m.read_word(WORD),"d0":d0,"ccr":ccr,"stack_delta":delta}));
            }
        }
    }
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut atan = Vec::new();
    for (kind, input) in operands.chunks_exact(10).take(base_count).enumerate() {
        for mode in modes {
            for flags in [16, 31, 0] {
                atan.push(s.measure(0x18, kind, input, mode, flags)?);
            }
        }
    }
    let mut dense = Vec::new();
    for (kind, input) in operands.chunks_exact(10).skip(base_count).enumerate() {
        dense.push(s.measure(0x18, kind, input, 0, 16)?);
    }
    Ok(json!({"schema":1,"probe":"sane-sine-runtime-raw-v1","initial_env":initial,"controls":controls,"sin":atan,"dense":dense}).to_string())
}
pub fn tan_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanesine/operands.bin");
    let base_count = 62;
    let dense_count = 2178;
    assert_eq!(operands.len(), (base_count + dense_count) * 10);
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let initial = s.env(None)?;
    let mut controls = Vec::new();
    for mode in [
        0u16, 0x20, 0x40, 0x2000, 0x4000, 0x6000, 0x0100, 0x1000, 0x1f00, 0x001f, 0xffff,
    ] {
        for flags in [16, 31, 0] {
            for selector in [1u16, 3] {
                s.m.write_word(WORD, if selector == 1 { mode } else { 0x5a5a });
                let (d0, ccr, delta) = s.call(0xa9eb, selector, &[WORD], flags)?;
                controls.push(json!({"input_env":mode,"input_ccr":flags,"selector":selector,"word":s.m.read_word(WORD),"d0":d0,"ccr":ccr,"stack_delta":delta}));
            }
        }
    }
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut atan = Vec::new();
    for (kind, input) in operands.chunks_exact(10).take(base_count).enumerate() {
        for mode in modes {
            for flags in [16, 31, 0] {
                atan.push(s.measure(0x1c, kind, input, mode, flags)?);
            }
        }
    }
    let mut dense = Vec::new();
    for (kind, input) in operands.chunks_exact(10).skip(base_count).enumerate() {
        dense.push(s.measure(0x1c, kind, input, 0, 16)?);
    }
    Ok(json!({"schema":1,"probe":"sane-tan-runtime-raw-v1","initial_env":initial,"controls":controls,"tan":atan,"dense":dense}).to_string())
}
pub fn cos_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanesine/operands.bin");
    let base_count = 62;
    let dense_count = 2178;
    assert_eq!(operands.len(), (base_count + dense_count) * 10);
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let initial = s.env(None)?;
    let mut controls = Vec::new();
    for mode in [
        0u16, 0x20, 0x40, 0x2000, 0x4000, 0x6000, 0x0100, 0x1000, 0x1f00, 0x001f, 0xffff,
    ] {
        for flags in [16, 31, 0] {
            for selector in [1u16, 3] {
                s.m.write_word(WORD, if selector == 1 { mode } else { 0x5a5a });
                let (d0, ccr, delta) = s.call(0xa9eb, selector, &[WORD], flags)?;
                controls.push(json!({"input_env":mode,"input_ccr":flags,"selector":selector,"word":s.m.read_word(WORD),"d0":d0,"ccr":ccr,"stack_delta":delta}));
            }
        }
    }
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut atan = Vec::new();
    for (kind, input) in operands.chunks_exact(10).take(base_count).enumerate() {
        for mode in modes {
            for flags in [16, 31, 0] {
                atan.push(s.measure(0x1a, kind, input, mode, flags)?);
            }
        }
    }
    let mut dense = Vec::new();
    for (kind, input) in operands.chunks_exact(10).skip(base_count).enumerate() {
        dense.push(s.measure(0x1a, kind, input, 0, 16)?);
    }
    Ok(json!({"schema":1,"probe":"sane-cos-runtime-raw-v1","initial_env":initial,"controls":controls,"cos":atan,"dense":dense}).to_string())
}
fn arithmetic_probe(selector: u16) -> Result<String, String> {
    let mut pairs = vec![
        (2, 2),
        (4, 4),
        (6, 4),
        (11, 6),
        (3, 6),
        (3, 7),
        (0, 2),
        (1, 2),
        (0, 21),
        (21, 0),
        (21, 6),
        (23, 3),
        (24, 2),
        (32, 33),
        (33, 32),
        (35, 2),
        (36, 37),
        (15, 4),
        (16, 4),
        (17, 4),
        (17, 6),
        (18, 4),
        (19, 19),
        (20, 6),
        (29, 2),
        (30, 21),
        (31, 2),
        (8, 9),
        (10, 11),
        (13, 13),
        (14, 14),
        (20, 4),
    ];
    if selector != 4 {
        pairs.extend([
            (0, 0),
            (0, 2),
            (1, 3),
            (21, 21),
            (23, 21),
            (21, 23),
            (2, 0),
            (3, 1),
        ]);
    }
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let source = VALUE;
    let dest = SINGLE;
    let mut records = Vec::new();
    for kind in 0..pairs.len() + 32 {
        let (src, dst) = if kind < pairs.len() {
            pairs[kind]
        } else {
            (
                BASE_COUNT + 514 + (kind - pairs.len()) * 3,
                BASE_COUNT + 514 + ((kind - pairs.len()) * 3 + 37) % 128,
            )
        };
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                for (p, index, before, after) in [
                    (source, src, 0x11223344, 0x55667788),
                    (dest, dst, 0x99aabbcc, 0xddeeff00),
                ] {
                    s.m.write_long(p - 4, before);
                    s.m.write_long(p + 10, after);
                    s.m.put(p as usize, &OPERANDS[index * 10..index * 10 + 10])?;
                }
                let source_before = hex(&mut s.m, source);
                let dest_before = hex(&mut s.m, dest);
                s.env(Some(mode))?;
                let arguments = if selector == 0x12 {
                    vec![dest]
                } else {
                    vec![source, dest]
                };
                let (d0, ccr, delta) = s.call(0xa9eb, selector, &arguments, input_ccr)?;
                let env = s.env(None)?;
                let mut row = json!({"kind":kind,"source_index":src,"dest_index":dst,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(source-4),s.m.read_long(source+10),s.m.read_long(dest-4),s.m.read_long(dest+10)],"source_before_hex":source_before,"dest_before_hex":dest_before,"source_after_hex":hex(&mut s.m,source),"dest_after_hex":hex(&mut s.m,dest)});
                if selector != 4 && selector != 6 {
                    row["selector"] = json!(selector);
                }
                records.push(row);
            }
        }
    }
    Ok(json!({"schema":1,"probe":match selector {4=>"sane-mul-runtime-raw-v1",6=>"sane-div-runtime-raw-v1",_=>"sane-cmp-runtime-raw-v1"},"records":records}).to_string())
}
pub fn multiplication_probe() -> Result<String, String> {
    arithmetic_probe(4)
}
pub fn division_probe() -> Result<String, String> {
    arithmetic_probe(6)
}
pub fn comparison_probe() -> Result<String, String> {
    arithmetic_probe(8)
}
pub fn basic_probe() -> Result<String, String> {
    let mut records = Vec::new();
    for selector in [0, 2, 8, 0x12] {
        let raw: Value =
            serde_json::from_str(&arithmetic_probe(selector)?).map_err(|e| e.to_string())?;
        records.extend(raw["records"].as_array().unwrap().iter().cloned());
    }
    Ok(json!({"schema":1,"probe":"sane-basicx-runtime-raw-v1","records":records}).to_string())
}
pub fn conversion_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/saneconvert/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(10).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 10, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 8, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 8])?;
                let before = hex(&mut s.m, VALUE);
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x0810, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..8)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+8)],"source_before_hex":before,"source_after_hex":hex(&mut s.m,VALUE),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-convert-runtime-raw-v1","records":records}).to_string())
}
pub fn classification_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/saneconvert/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(10).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 10, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 2, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 2])?;
                let before = hex(&mut s.m, VALUE);
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x001c, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..2)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"selector":28,"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+2)],"source_before_hex":before,"source_after_hex":hex(&mut s.m,VALUE),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-class-runtime-raw-v1","records":records}).to_string())
}
pub fn extended_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/saneconvert/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for selector in [0x000e, 0x0010] {
        for (kind, input) in operands.chunks_exact(10).enumerate() {
            for mode in modes {
                for input_ccr in [16, 31, 0] {
                    s.m.write_long(VALUE - 4, 0x11223344);
                    s.m.write_long(VALUE + 10, 0x55667788);
                    s.m.put(VALUE as usize, input)?;
                    s.m.write_long(SINGLE - 4, 0x99aabbcc);
                    s.m.write_long(SINGLE + 10, 0xddeeff00);
                    s.m.put(SINGLE as usize, &[0x7b; 10])?;
                    let before = hex(&mut s.m, VALUE);
                    s.env(Some(mode))?;
                    let (d0, ccr, delta) = s.call(0xa9eb, selector, &[VALUE, SINGLE], input_ccr)?;
                    let env = s.env(None)?;
                    let dest = (0..10)
                        .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                        .collect::<String>();
                    records.push(json!({"selector":selector,"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":hex(&mut s.m,VALUE),"dest_hex":dest}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-extended-runtime-raw-v1","records":records}).to_string())
}
pub fn widening_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanewiden/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(8).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 8, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 10, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 10])?;
                let before = (0..8)
                    .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                    .collect::<String>();
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x080e, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+8),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":(0..8).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-widen-runtime-raw-v1","records":records}).to_string())
}
pub fn single_widening_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanef32widen/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(4).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 4, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 10, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 10])?;
                let before = (0..4)
                    .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                    .collect::<String>();
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x100e, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+4),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":(0..4).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-single-widen-runtime-raw-v1","records":records}).to_string())
}
pub fn long_widening_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanei32widen/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(4).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 4, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 10, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 10])?;
                let before = (0..4)
                    .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                    .collect::<String>();
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x280e, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+4),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":(0..4).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-long-widen-runtime-raw-v1","records":records}).to_string())
}
pub fn small_integer_power_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/sanepower/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(20).take(220).enumerate().filter(|(i, _)| i % 10 < 5) {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 10, 0x55667788);
                s.m.put(VALUE as usize, &input[..10])?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 10, 0xddeeff00);
                s.m.put(SINGLE as usize, &input[10..])?;
                let before = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                    .collect::<String>();
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9ec, 0x8012, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":(0..10).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_before_hex":input[10..].iter().map(|v| format!("{v:02x}")).collect::<String>(),"dest_after_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-small-integer-power-runtime-raw-v1","records":records}).to_string())
}
pub fn integer_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/saneinteger/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (kind, input) in operands.chunks_exact(2).enumerate() {
        for mode in modes {
            for input_ccr in [16, 31, 0] {
                s.m.write_long(VALUE - 4, 0x11223344);
                s.m.write_long(VALUE + 2, 0x55667788);
                s.m.put(VALUE as usize, input)?;
                s.m.write_long(SINGLE - 4, 0x99aabbcc);
                s.m.write_long(SINGLE + 10, 0xddeeff00);
                s.m.put(SINGLE as usize, &[0x7b; 10])?;
                let before = (0..2)
                    .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                    .collect::<String>();
                s.env(Some(mode))?;
                let (d0, ccr, delta) = s.call(0xa9eb, 0x200e, &[VALUE, SINGLE], input_ccr)?;
                let env = s.env(None)?;
                let dest = (0..10)
                    .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                    .collect::<String>();
                records.push(json!({"kind":kind,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+2),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"source_after_hex":(0..2).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_hex":dest}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-integer-runtime-raw-v1","records":records}).to_string())
}
pub fn environment_probe() -> Result<String, String> {
    let modes = [
        0u16, 0x20, 0x40, 0x60, 0x2000, 0x4000, 0x6000, 0x80, 0x100, 0x200, 0x400, 0x800, 0x1000,
        0x1f00, 0x1f, 0xffff, 0x5a5a, 0xa5a5, 0x7f80, 0xff00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for phase in 0..2 {
        for saved in modes {
            for index in 0..if phase == 0 { 1 } else { 20 } {
                for input_ccr in [16, 31, 0] {
                    let current = if phase == 0 { saved } else { modes[index] };
                    if phase == 1
                        && (saved & 31 != 0 || ((current & 31) & (current >> 8) & 31) != 0)
                    {
                        continue;
                    }
                    let input_word = if phase == 0 { 0x5a5a } else { saved };
                    let selector = if phase == 0 { 0x17 } else { 0x19 };
                    s.env(Some(current))?;
                    s.m.write_long(VALUE - 4, 0x11223344);
                    s.m.write_word(VALUE, input_word);
                    s.m.write_long(VALUE + 2, 0x55667788);
                    let (d0, ccr, delta) = s.call(0xa9eb, selector, &[VALUE], input_ccr)?;
                    let env = s.env(None)?;
                    records.push(json!({"selector":selector,"input_env":current,"input_word":input_word,"input_ccr":input_ccr,"word":s.m.read_word(VALUE),"output_env":env,"d0":d0,"ccr":ccr,"stack_delta":delta,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+2)]}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-env-runtime-raw-v1","records":records}).to_string())
}
pub fn mixed_probe() -> Result<String, String> {
    mixed_operations_probe(false)
}
pub fn mixed_add_probe() -> Result<String, String> {
    mixed_operations_probe(true)
}
fn mixed_operations_probe(add: bool) -> Result<String, String> {
    let doubles = include_bytes!("../../probes/sanewiden/operands.bin");
    let destinations = [
        2, 2, 4, 6, 2, 3, 20, 17, 0, 1, 2, 3, 2, 2, 6, 7, 15, 16, 20, 6, 2, 3, 36, 32, 2, 27, 35,
        24, 37, 2, 24, 36,
    ];
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for selector in if add {
        [0x0800, 0x0802]
    } else {
        [0x0804, 0x0806]
    } {
        for kind in 0..64 {
            let src = if kind < 32 {
                kind
            } else {
                80 + ((kind - 32) * 3) % 128
            };
            let dst = if kind < 32 {
                destinations[kind]
            } else {
                BASE_COUNT + 514 + ((kind - 32) * 3 + 37) % 128
            };
            for mode in modes {
                for input_ccr in [16, 31, 0] {
                    s.m.write_long(VALUE - 4, 0x11223344);
                    s.m.write_long(VALUE + 8, 0x55667788);
                    s.m.put(VALUE as usize, &doubles[src * 8..src * 8 + 8])?;
                    s.m.write_long(SINGLE - 4, 0x99aabbcc);
                    s.m.write_long(SINGLE + 10, 0xddeeff00);
                    s.m.put(SINGLE as usize, &OPERANDS[dst * 10..dst * 10 + 10])?;
                    let before = (0..8)
                        .map(|i| format!("{:02x}", s.m.read_byte(VALUE + i)))
                        .collect::<String>();
                    let dest_before = hex(&mut s.m, SINGLE);
                    s.env(Some(mode))?;
                    let (d0, ccr, delta) = s.call(0xa9eb, selector, &[VALUE, SINGLE], input_ccr)?;
                    let env = s.env(None)?;
                    records.push(json!({"selector":selector,"kind":kind,"source_index":src,"dest_index":dst,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+8),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":before,"dest_before_hex":dest_before,"source_after_hex":(0..8).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i))).collect::<String>(),"dest_after_hex":hex(&mut s.m,SINGLE)}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":if add {"sane-mixedadd-runtime-raw-v1"} else {"sane-mixed-runtime-raw-v1"},"records":records}).to_string())
}
pub fn round_probe() -> Result<String, String> {
    let operands = include_bytes!("../../probes/saneround/operands.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for selector in [0x14, 0x16] {
        let width = match selector {
            0x2010 => 2,
            0x2810 => 4,
            _ => 10,
        };
        for kind in 0..189 {
            let source = &operands[kind * 10..kind * 10 + 10];
            for mode in modes {
                for input_ccr in [16, 31, 0] {
                    s.m.write_long(VALUE - 4, 0x11223344);
                    s.m.put(VALUE as usize, source)?;
                    s.m.write_long(VALUE + 10, 0x55667788);
                    s.m.write_long(SINGLE - 4, 0x99aabbcc);
                    if width == 10 {
                        s.m.put(SINGLE as usize, source)?;
                    } else {
                        for i in 0..width {
                            s.m.write_byte(SINGLE + i, 0xa5);
                        }
                    }
                    s.m.write_long(SINGLE + width, 0xddeeff00);
                    let before = (0..width)
                        .map(|i| format!("{:02x}", s.m.read_byte(SINGLE + i)))
                        .collect::<String>();
                    s.env(Some(mode))?;
                    let args = if width == 10 {
                        vec![SINGLE]
                    } else {
                        vec![VALUE, SINGLE]
                    };
                    let (d0, ccr, delta) = s.call(0xa9eb, selector, &args, input_ccr)?;
                    let env = s.env(None)?;
                    records.push(json!({"selector":selector,"kind":kind,"dest_width":width,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+width)],"source_before_hex":source.iter().map(|b|format!("{:02x}",b)).collect::<String>(),"dest_before_hex":before,"source_after_hex":hex(&mut s.m,VALUE),"dest_after_hex":(0..width).map(|i|format!("{:02x}",s.m.read_byte(SINGLE+i))).collect::<String>()}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-round-runtime-raw-v1","records":records}).to_string())
}
pub fn narrow_probe() -> Result<String, String> {
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for (dataset, operands) in [
        (
            0,
            include_bytes!("../../probes/saneround/operands.bin").as_slice(),
        ),
        (
            1,
            include_bytes!("../../probes/saneintflags/operands.bin").as_slice(),
        ),
    ] {
        for selector in [0x2010, 0x2810] {
            let width = if selector == 0x2010 { 2 } else { 4 };
            for (kind, source) in operands.chunks_exact(10).enumerate() {
                let value =
                    classic_core::extended::Extended80::from_bytes(source.try_into().unwrap());
                if value
                    .integer_sane(classic_core::extended::Rounding::Nearest, width * 8)
                    .is_none()
                {
                    continue;
                }
                for mode in modes {
                    for input_ccr in [16, 31, 0] {
                        s.m.write_long(VALUE - 4, 0x11223344);
                        s.m.put(VALUE as usize, source)?;
                        s.m.write_long(VALUE + 10, 0x55667788);
                        s.m.write_long(SINGLE - 4, 0x99aabbcc);
                        for i in 0..width {
                            s.m.write_byte(SINGLE + i, 0xa5);
                        }
                        s.m.write_long(SINGLE + width, 0xddeeff00);
                        s.env(Some(mode))?;
                        let (d0, ccr, delta) =
                            s.call(0xa9eb, selector, &[VALUE, SINGLE], input_ccr)?;
                        let env = s.env(None)?;
                        records.push(json!({"dataset":dataset,"selector":selector,"kind":kind,"dest_width":width,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+width)],"source_before_hex":source.iter().map(|b|format!("{:02x}",b)).collect::<String>(),"dest_before_hex":"a5".repeat(width as usize),"source_after_hex":hex(&mut s.m,VALUE),"dest_after_hex":(0..width).map(|i|format!("{:02x}",s.m.read_byte(SINGLE+i))).collect::<String>()}));
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-narrow-runtime-raw-v1","records":records}).to_string())
}
pub fn comparison_formats_probe() -> Result<String, String> {
    let x = include_bytes!("../../probes/sanecompare/sources_x.bin");
    let d = include_bytes!("../../probes/sanecompare/sources_d.bin");
    let dests = include_bytes!("../../probes/sanecompare/destinations.bin");
    let modes = [
        0u16, 0x20, 0x40, 0x2000, 0x2020, 0x2040, 0x4000, 0x4020, 0x4040, 0x6000, 0x6020, 0x6040,
        0x0100, 0x0800, 0x1f00,
    ];
    let mut s = Scene {
        m: Memory::zeroed(0x800000),
        sane: Sane::new(0x800000),
        cpu: CpuCore::new(),
    };
    let mut records = Vec::new();
    for selector in [0x0008, 0x0808, 0x000a, 0x080a] {
        let width = if selector & 0x800 != 0 { 8 } else { 10 };
        let inputs = if width == 8 {
            d.as_slice()
        } else {
            x.as_slice()
        };
        for kind in 0..128 {
            let source = &inputs[kind * width..(kind + 1) * width];
            let dest = &dests[kind * 10..kind * 10 + 10];
            for mode in modes {
                for input_ccr in [16, 31, 0] {
                    s.m.write_long(VALUE - 4, 0x11223344);
                    s.m.put(VALUE as usize, &[0x5a; 10])?;
                    s.m.put(VALUE as usize, source)?;
                    s.m.write_long(VALUE + 10, 0x55667788);
                    s.m.write_long(SINGLE - 4, 0x99aabbcc);
                    s.m.put(SINGLE as usize, dest)?;
                    s.m.write_long(SINGLE + 10, 0xddeeff00);
                    s.env(Some(mode))?;
                    let (d0, ccr, delta) = s.call(0xa9eb, selector, &[VALUE, SINGLE], input_ccr)?;
                    let env = s.env(None)?;
                    records.push(json!({"selector":selector,"kind":kind,"source_width":width,"input_env":mode,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"output_env":env,"source_padding":if width==8{s.m.read_word(VALUE+8)}else{0},"guards":[s.m.read_long(VALUE-4),s.m.read_long(VALUE+10),s.m.read_long(SINGLE-4),s.m.read_long(SINGLE+10)],"source_before_hex":source.iter().map(|b|format!("{:02x}",b)).collect::<String>(),"dest_before_hex":dest.iter().map(|b|format!("{:02x}",b)).collect::<String>(),"source_after_hex":(0..width).map(|i|format!("{:02x}",s.m.read_byte(VALUE+i as u32))).collect::<String>(),"dest_after_hex":hex(&mut s.m,SINGLE)}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"sane-compare-runtime-raw-v1","records":records}).to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signaling_comparison_halt_preserves_both_nan_operand_bytes_and_cpu_return() {
        for selector in [0x000a, 0x080a] {
            let mut s = Scene {
                m: Memory::zeroed(0x800000),
                sane: Sane::new(0x800000),
                cpu: CpuCore::new(),
            };
            let source = if selector & 0x800 != 0 {
                0x7ff8000000000001u64.to_be_bytes().to_vec()
            } else {
                classic_core::extended::Extended80 {
                    sign_exp: 0x7fff,
                    significand: 0x4000000000000001,
                }
                .to_bytes()
                .to_vec()
            };
            s.m.put(VALUE as usize, &source).unwrap();
            s.m.put(
                SINGLE as usize,
                &classic_core::extended::Extended80::from_integer(1).to_bytes(),
            )
            .unwrap();
            s.env(Some(1)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[VALUE, SINGLE], 31)
                .unwrap_err()
                .contains("0x100"));
            assert_eq!(s.cpu.pc, 0x6000);
            assert_eq!(s.cpu.dar[15], 0x700000 - 10);
            assert_eq!(s.cpu.get_sr() & 31, 31);
            assert_eq!(hex(&mut s.m, SINGLE), "3fff8000000000000000");
            for (i, b) in source.iter().enumerate() {
                assert_eq!(s.m.read_byte(VALUE + i as u32), *b);
            }
            assert_eq!(s.env(None).unwrap(), 1);
            // The quiet form does not signal invalid for the same quiet NaN.
            let quiet = selector - 2;
            let (_, ccr, _) = s.call(0xa9eb, quiet, &[VALUE, SINGLE], 31).unwrap();
            assert_eq!(ccr, 2);
            assert_eq!(s.env(None).unwrap(), 1);
            s.env(Some(0)).unwrap();
            if selector & 0x800 != 0 {
                s.m.put(0x7ffff8, &0x3ff0000000000000u64.to_be_bytes())
                    .unwrap();
                assert_eq!(
                    s.call(0xa9eb, selector, &[0x7ffff8, SINGLE], 31).unwrap().1,
                    4
                );
                assert!(s
                    .call(0xa9eb, selector, &[0x7ffffc, SINGLE], 31)
                    .unwrap_err()
                    .contains("8 bytes"));
            } else {
                assert!(s
                    .call(0xa9eb, selector, &[0x7ffff8, SINGLE], 31)
                    .unwrap_err()
                    .contains("10 bytes"));
            }
        }
    }
    #[test]
    fn tangent_divide_by_zero_halt_preserves_the_in_place_operand() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        let bytes = classic_core::extended::Extended80 {
            sign_exp: 0x3fff,
            significand: 0xc90fdaa22168c235,
        }
        .to_bytes();
        s.m.put(VALUE as usize, &bytes).unwrap();
        s.env(Some(8)).unwrap();
        assert!(s
            .call(0xa9ec, 0x1c, &[VALUE], 31)
            .unwrap_err()
            .contains("0x1800"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 6);
        assert_eq!(s.cpu.get_sr() & 31, 31);
        assert_eq!(hex(&mut s.m, VALUE), "3fffc90fdaa22168c235");
        assert_eq!(s.env(None).unwrap(), 8);
        s.env(Some(0)).unwrap();
        s.m.put(0x7ffff6, &bytes).unwrap();
        s.call(0xa9ec, 0x1c, &[0x7ffff6], 16).unwrap();
        assert_eq!(hex(&mut s.m, 0x7ffff6), "7fff0000000000000000");
        assert!(s
            .call(0xa9ec, 0x1c, &[0x7ffff8], 16)
            .unwrap_err()
            .contains("10 bytes"));
    }
    #[test]
    fn integer_conversion_rejections_are_atomic_and_destinations_have_exact_width() {
        for selector in [0x2010, 0x2810] {
            let width = if selector == 0x2010 { 2 } else { 4 };
            let mut s = Scene {
                m: Memory::zeroed(0x800000),
                sane: Sane::new(0x800000),
                cpu: CpuCore::new(),
            };
            let large = classic_core::extended::Extended80 {
                sign_exp: 0x403e,
                significand: 1 << 63,
            }
            .to_bytes();
            s.m.put(VALUE as usize, &large).unwrap();
            s.m.write_long(SINGLE, 0xa5a5a5a5);
            s.env(Some(0x2180)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[VALUE, SINGLE], 31)
                .unwrap_err()
                .contains("magnitude >= 2^63"));
            assert_eq!(s.cpu.pc, 0x6000);
            assert_eq!(s.cpu.dar[15], 0x700000 - 10);
            assert_eq!(s.cpu.get_sr() & 31, 31);
            assert_eq!(s.m.read_long(SINGLE), 0xa5a5a5a5);
            assert_eq!(hex(&mut s.m, VALUE), "403e8000000000000000");
            assert_eq!(s.env(None).unwrap(), 0x2180);
            s.m.put(
                VALUE as usize,
                &classic_core::extended::Extended80 {
                    sign_exp: 0x7fff,
                    significand: 1 << 62,
                }
                .to_bytes(),
            )
            .unwrap();
            s.env(Some(1)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[VALUE, SINGLE], 16)
                .unwrap_err()
                .contains("0x100"));
            assert_eq!(s.cpu.pc, 0x6000);
            assert_eq!(s.m.read_long(SINGLE), 0xa5a5a5a5);
            assert_eq!(s.env(None).unwrap(), 1);
            s.env(Some(0)).unwrap();
            s.m.put(
                VALUE as usize,
                &classic_core::extended::Extended80::from_integer(-7).to_bytes(),
            )
            .unwrap();
            s.call(0xa9eb, selector, &[VALUE, 0x800000 - width], 0)
                .unwrap();
            if width == 2 {
                assert_eq!(s.m.read_word(0x7ffffe), (-7i16) as u16);
            } else {
                assert_eq!(s.m.read_long(0x7ffffc), (-7i32) as u32);
            }
            assert!(s
                .call(0xa9eb, selector, &[VALUE, 0x800001 - width], 0)
                .unwrap_err()
                .contains("bytes"));
            assert!(s
                .call(0xa9eb, selector, &[0x7ffff8, SINGLE], 0)
                .unwrap_err()
                .contains("10 bytes"));
        }
    }
    #[test]
    fn mixed_arithmetic_checks_eight_byte_sources_and_halts_before_writing() {
        for selector in [0x0800, 0x0802, 0x0804, 0x0806] {
            let mut s = Scene {
                m: Memory::zeroed(0x800000),
                sane: Sane::new(0x800000),
                cpu: CpuCore::new(),
            };
            s.m.put(VALUE as usize, &0x7ff0000000000001u64.to_be_bytes())
                .unwrap();
            s.m.put(
                SINGLE as usize,
                &classic_core::extended::Extended80::from_integer(1).to_bytes(),
            )
            .unwrap();
            s.env(Some(1)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[VALUE, SINGLE], 16)
                .unwrap_err()
                .contains("0x100"));
            assert_eq!(s.cpu.pc, 0x6000);
            assert_eq!(s.cpu.dar[15], 0x700000 - 10);
            assert_eq!(hex(&mut s.m, SINGLE), "3fff8000000000000000");
            assert_eq!(s.m.read_long(VALUE + 4), 1);
            assert_eq!(s.env(None).unwrap(), 1);
            s.env(Some(0)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[0x7ffffc, SINGLE], 16)
                .unwrap_err()
                .contains("8 bytes"));
            s.m.put(0x7ffff8, &1f64.to_bits().to_be_bytes()).unwrap();
            s.call(0xa9eb, selector, &[0x7ffff8, SINGLE], 16).unwrap();
            assert_eq!(
                hex(&mut s.m, SINGLE),
                match selector {
                    0x0800 => "40008000000000000000",
                    0x0802 => "00000000000000000000",
                    _ => "3fff8000000000000000",
                }
            );
        }
    }
    #[test]
    fn procedure_environments_nest_and_unsupported_restore_halt_is_atomic() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.env(Some(0x2180)).unwrap();
        s.call(0xa9eb, 0x17, &[VALUE], 16).unwrap();
        assert_eq!(s.m.read_word(VALUE), 0x2180);
        assert_eq!(s.env(None).unwrap(), 0);
        s.env(Some(0x0420)).unwrap();
        s.call(0xa9eb, 0x17, &[SINGLE], 0).unwrap();
        s.env(Some(0x1200)).unwrap();
        s.call(0xa9eb, 0x19, &[SINGLE], 0).unwrap();
        assert_eq!(s.env(None).unwrap(), 0x1620);
        s.call(0xa9eb, 0x19, &[VALUE], 16).unwrap();
        assert_eq!(s.env(None).unwrap(), 0x3780);
        assert_eq!(s.m.read_word(VALUE), 0x2180);
        assert_eq!(s.m.read_word(SINGLE), 0x0420);
        s.env(Some(0x0100)).unwrap();
        s.m.write_word(VALUE, 1);
        assert!(s
            .call(0xa9eb, 0x19, &[VALUE], 16)
            .unwrap_err()
            .contains("0x100"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 6);
        assert_eq!(s.m.read_word(VALUE), 1);
        assert_eq!(s.env(None).unwrap(), 0x0100);
    }
    #[test]
    fn basic_operations_preserve_inputs_on_halt_and_support_aliased_addition() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.put(
            VALUE as usize,
            &classic_core::extended::Extended80 {
                sign_exp: 0x7fff,
                significand: 1,
            }
            .to_bytes(),
        )
        .unwrap();
        s.m.put(
            SINGLE as usize,
            &classic_core::extended::Extended80::from_integer(1).to_bytes(),
        )
        .unwrap();
        s.env(Some(1)).unwrap();
        assert!(s
            .call(0xa9eb, 8, &[VALUE, SINGLE], 16)
            .unwrap_err()
            .contains("0x100"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(hex(&mut s.m, VALUE), "7fff0000000000000001");
        assert_eq!(hex(&mut s.m, SINGLE), "3fff8000000000000000");
        assert_eq!(s.env(None).unwrap(), 1);
        s.env(Some(0)).unwrap();
        s.call(0xa9eb, 0, &[SINGLE, SINGLE], 16).unwrap();
        assert_eq!(hex(&mut s.m, SINGLE), "40008000000000000000");
        s.call(0xa9eb, 2, &[SINGLE, SINGLE], 16).unwrap();
        assert_eq!(hex(&mut s.m, SINGLE), "00000000000000000000");
    }
    #[test]
    fn classification_does_not_quiet_or_halt_on_signaling_nan() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.put(
            VALUE as usize,
            &classic_core::extended::Extended80 {
                sign_exp: 0x7fff,
                significand: 1,
            }
            .to_bytes(),
        )
        .unwrap();
        s.env(Some(0xffff)).unwrap();
        s.m.write_word(SINGLE, 0x7b7b);
        let (d0, ccr, delta) = s.call(0xa9eb, 0x1c, &[VALUE, SINGLE], 0).unwrap();
        assert_eq!((d0, ccr, delta), (0x12345678, 16, 0));
        assert_eq!(s.m.read_word(SINGLE), 1);
        assert_eq!(hex(&mut s.m, VALUE), "7fff0000000000000001");
        assert_eq!(s.env(None).unwrap(), 0xffff);
        assert!(s
            .call(0xa9eb, 0x1c, &[VALUE, 0x7fffff], 0)
            .unwrap_err()
            .contains("2 bytes"));
        assert_eq!(s.env(None).unwrap(), 0xffff);
    }
    #[test]
    fn extended_conversions_handle_aliasing_and_halt_without_partial_writes() {
        for selector in [0x000e, 0x0010] {
            let mut s = Scene {
                m: Memory::zeroed(0x800000),
                sane: Sane::new(0x800000),
                cpu: CpuCore::new(),
            };
            s.m.put(
                VALUE as usize,
                &classic_core::extended::Extended80 {
                    sign_exp: 0x7fff,
                    significand: 1,
                }
                .to_bytes(),
            )
            .unwrap();
            s.m.put(SINGLE as usize, &[0x7b; 10]).unwrap();
            s.env(Some(1)).unwrap();
            assert!(s
                .call(0xa9eb, selector, &[VALUE, SINGLE], 16)
                .unwrap_err()
                .contains("0x100"));
            assert_eq!(s.cpu.pc, 0x6000);
            assert_eq!(s.cpu.dar[15], 0x700000 - 10);
            assert_eq!(hex(&mut s.m, VALUE), "7fff0000000000000001");
            assert_eq!(hex(&mut s.m, SINGLE), "7b7b7b7b7b7b7b7b7b7b");
            assert_eq!(s.env(None).unwrap(), 1);
            s.env(Some(0)).unwrap();
            s.call(0xa9eb, selector, &[VALUE, VALUE], 16).unwrap();
            assert_eq!(hex(&mut s.m, VALUE), "7fff4000000000000001");
            assert_eq!(s.env(None).unwrap(), 0x100);
        }
    }
    #[test]
    fn sine_halt_does_not_commit_operand_environment_or_cpu_return() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.put(
            VALUE as usize,
            &classic_core::extended::Extended80::from_integer(1).to_bytes(),
        )
        .unwrap();
        s.env(Some(0x10)).unwrap();
        assert!(s
            .call(0xa9ec, 0x18, &[VALUE], 16)
            .unwrap_err()
            .contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 6);
        assert_eq!(hex(&mut s.m, VALUE), "3fff8000000000000000");
        assert_eq!(s.env(None).unwrap(), 0x10);
    }
    #[test]
    fn division_halt_is_atomic_and_integer_aliasing_reads_before_write() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.put(VALUE as usize, &[0; 10]).unwrap();
        s.m.put(
            SINGLE as usize,
            &classic_core::extended::Extended80::from_integer(1).to_bytes(),
        )
        .unwrap();
        s.env(Some(8)).unwrap();
        assert!(s
            .call(0xa9eb, 6, &[VALUE, SINGLE], 16)
            .unwrap_err()
            .contains("0x800"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(hex(&mut s.m, SINGLE), "3fff8000000000000000");
        assert_eq!(s.env(None).unwrap(), 8);
        s.env(Some(0x1f00)).unwrap();
        s.m.write_word(VALUE, 0x8000);
        s.call(0xa9eb, 0x200e, &[VALUE, VALUE], 16).unwrap();
        assert_eq!(hex(&mut s.m, VALUE), "c00e8000000000000000");
        assert_eq!(s.env(None).unwrap(), 0x1f00);
        assert!(s
            .call(0xa9eb, 0x200e, &[0x7fffff, SINGLE], 16)
            .unwrap_err()
            .contains("2 bytes"));
        assert_eq!(hex(&mut s.m, SINGLE), "3fff8000000000000000");
    }
    #[test]
    fn integer_power_checks_operands_and_halts_without_partial_commit() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        let exponent = classic_core::extended::Extended80::from_integer(-1).to_bytes();
        let base = classic_core::extended::Extended80 {
            sign_exp: 0x3ffe, significand: 0xc000000000000000,
        }.to_bytes();
        s.m.put(VALUE as usize, &exponent).unwrap();
        s.m.put(SINGLE as usize, &base).unwrap();
        s.env(Some(0x10)).unwrap(); // Reciprocal(3/4) raises inexact.
        assert!(s.call(0xa9ec, 0x8012, &[VALUE, SINGLE], 31)
            .unwrap_err().contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(s.cpu.dar[0], 0x12345678);
        assert_eq!(s.cpu.get_sr() & 31, 31);
        assert_eq!(hex(&mut s.m, VALUE), "bfff8000000000000000");
        assert_eq!(hex(&mut s.m, SINGLE), "3ffec000000000000000");
        assert_eq!(s.env(None).unwrap(), 0x10);
        assert!(s.call(0xa9ec, 0x8012, &[0x7ffff8, SINGLE], 16)
            .unwrap_err().contains("10 bytes"));
        assert!(s.call(0xa9ec, 0x8012, &[VALUE, 0x7ffff8], 16)
            .unwrap_err().contains("10 bytes"));
        s.env(Some(0)).unwrap();
        let three = classic_core::extended::Extended80::from_integer(3).to_bytes();
        s.m.put(VALUE as usize, &three).unwrap();
        s.m.write_word(0x6ffff6, 0x8012);
        s.m.write_long(0x6ffff8, SINGLE);
        s.m.write_long(0x6ffffc, VALUE);
        s.cpu.pc = 0x6000;
        s.cpu.dar[15] = 0x6ffff6;
        s.cpu.set_ccr(31);
        let registers = s.cpu.dar;
        assert!(!s.sane.dispatch(0xa9ec, &mut s.cpu, &mut s.m).unwrap());
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar, registers);
        assert_eq!(s.cpu.get_sr() & 31, 31);
        assert_eq!(hex(&mut s.m, SINGLE), "3ffec000000000000000");
        assert_eq!(s.env(None).unwrap(), 0);
    }
    #[test]
    fn long_widen_rounding_halt_is_atomic_and_source_width_is_four() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.write_long(0x7ffffc, 0x80000000);
        s.env(Some(0x40)).unwrap();
        s.call(0xa9eb, 0x280e, &[0x7ffffc, SINGLE], 31).unwrap();
        assert_eq!(hex(&mut s.m, SINGLE), "c01e8000000000000000");
        assert_eq!(s.m.read_long(0x7ffffc), 0x80000000);
        s.m.write_long(VALUE, 0x7fffffff);
        s.m.put(SINGLE as usize, &[0x7b; 10]).unwrap();
        s.env(Some(0x50)).unwrap(); // Single precision, inexact halt enabled.
        assert!(s.call(0xa9eb, 0x280e, &[VALUE, SINGLE], 31)
            .unwrap_err().contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(s.cpu.dar[0], 0x12345678);
        assert_eq!(s.cpu.get_sr() & 31, 31);
        assert_eq!(hex(&mut s.m, SINGLE), "7b7b7b7b7b7b7b7b7b7b");
        assert_eq!(s.m.read_long(VALUE), 0x7fffffff);
        assert_eq!(s.env(None).unwrap(), 0x50);
        assert!(s.call(0xa9eb, 0x280e, &[0x7ffffe, SINGLE], 16)
            .unwrap_err().contains("4 bytes"));
        assert!(s.call(0xa9eb, 0x280e, &[VALUE, 0x7ffff8], 16)
            .unwrap_err().contains("10 bytes"));
        s.env(Some(0x40)).unwrap();
        s.call(0xa9eb, 0x280e, &[VALUE, SINGLE], 16).unwrap();
        assert_eq!(hex(&mut s.m, SINGLE), "401e8000000000000000");
        assert_eq!(s.env(None).unwrap(), 0x10c0);
    }
    #[test]
    fn single_widen_reads_only_four_bytes_and_halts_before_committing() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        // Exactly four accessible source bytes at the guest memory boundary.
        s.m.write_long(0x7ffffc, 0x00000001);
        s.env(Some(0)).unwrap();
        s.call(0xa9eb, 0x100e, &[0x7ffffc, SINGLE], 31).unwrap();
        assert_eq!(hex(&mut s.m, SINGLE), "3f6a8000000000000000");
        assert_eq!(s.cpu.get_sr() & 31, 16);
        assert_eq!(s.m.read_long(0x7ffffc), 1);
        s.m.write_long(VALUE, 0x7f800001); // Signaling NaN, invalid halt enabled.
        s.m.put(SINGLE as usize, &[0x7b; 10]).unwrap();
        s.env(Some(1)).unwrap();
        assert!(s.call(0xa9eb, 0x100e, &[VALUE, SINGLE], 31)
            .unwrap_err().contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(s.cpu.dar[0], 0x12345678);
        assert_eq!(s.cpu.get_sr() & 31, 31);
        assert_eq!(hex(&mut s.m, SINGLE), "7b7b7b7b7b7b7b7b7b7b");
        assert_eq!(s.m.read_long(VALUE), 0x7f800001);
        assert_eq!(s.env(None).unwrap(), 1);
        assert!(s.call(0xa9eb, 0x100e, &[0x7ffffe, SINGLE], 16)
            .unwrap_err().contains("4 bytes"));
        assert!(s.call(0xa9eb, 0x100e, &[VALUE, 0x7ffff8], 16)
            .unwrap_err().contains("10 bytes"));
        assert_eq!(hex(&mut s.m, SINGLE), "7b7b7b7b7b7b7b7b7b7b");
    }
    #[test]
    fn widen_checks_entire_buffers_and_preserves_state_on_unsupported_halt() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        s.m.put(VALUE as usize, &0x3ff0000000000001u64.to_be_bytes())
            .unwrap();
        s.m.put(SINGLE as usize, &[0x7b; 10]).unwrap();
        s.env(Some(0x50)).unwrap();
        assert!(s
            .call(0xa9eb, 0x080e, &[VALUE, SINGLE], 16)
            .unwrap_err()
            .contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(hex(&mut s.m, SINGLE), "7b7b7b7b7b7b7b7b7b7b");
        assert_eq!(s.env(None).unwrap(), 0x50);
        assert!(s
            .call(0xa9eb, 0x080e, &[0x7ffffc, SINGLE], 16)
            .unwrap_err()
            .contains("8 bytes"));
        assert!(s
            .call(0xa9eb, 0x080e, &[VALUE, 0x7ffff8], 16)
            .unwrap_err()
            .contains("10 bytes"));
        assert_eq!(s.env(None).unwrap(), 0x50);
    }
    #[test]
    fn double_conversion_halt_preserves_destination_and_source() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        let input = classic_core::extended::Extended80 {
            sign_exp: 0x3fff,
            significand: (1 << 63) + 1024,
        }
        .to_bytes();
        s.m.put(VALUE as usize, &input).unwrap();
        s.m.put(SINGLE as usize, &[0x7b; 8]).unwrap();
        s.env(Some(0x10)).unwrap();
        assert!(s
            .call(0xa9eb, 0x0810, &[VALUE, SINGLE], 16)
            .unwrap_err()
            .contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(hex(&mut s.m, VALUE), "3fff8000000000000400");
        assert_eq!(s.m.read_long(SINGLE), 0x7b7b7b7b);
        assert_eq!(s.m.read_long(SINGLE + 4), 0x7b7b7b7b);
        assert_eq!(s.env(None).unwrap(), 0x10);
        s.env(Some(0)).unwrap();
        s.call(0xa9eb, 0x0810, &[VALUE, VALUE], 16).unwrap();
        assert_eq!(hex(&mut s.m, VALUE), "3ff00000000000000400");
        assert_eq!(s.env(None).unwrap(), 0x1000);
    }
    #[test]
    fn multiplication_aliases_read_both_operands_before_commit_and_halt_is_atomic() {
        let mut s = Scene {
            m: Memory::zeroed(0x800000),
            sane: Sane::new(0x800000),
            cpu: CpuCore::new(),
        };
        let input = classic_core::extended::Extended80 {
            sign_exp: 0x3fff,
            significand: u64::MAX,
        }
        .to_bytes();
        s.m.put(VALUE as usize, &input).unwrap();
        s.env(Some(0x10)).unwrap();
        assert!(s
            .call(0xa9eb, 4, &[VALUE, VALUE], 16)
            .unwrap_err()
            .contains("halt callback"));
        assert_eq!(s.cpu.pc, 0x6000);
        assert_eq!(s.cpu.dar[15], 0x700000 - 10);
        assert_eq!(hex(&mut s.m, VALUE), "3fffffffffffffffffff");
        assert_eq!(s.env(None).unwrap(), 0x10);
        s.env(Some(0)).unwrap();
        s.call(0xa9eb, 4, &[VALUE, VALUE], 16).unwrap();
        assert_eq!(hex(&mut s.m, VALUE), "4000fffffffffffffffe");
        assert_eq!(s.env(None).unwrap(), 0x1000);
    }
    #[test]
    fn unsupported_halt_and_bad_operand_do_not_commit_math() {
        let mut m = Memory::zeroed(0x800000);
        let mut sane = Sane::new(0x800000);
        let mut cpu = CpuCore::new();
        m.write_word(WORD, 0x10);
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_word(0x700000, 1);
        m.write_long(0x700002, WORD);
        assert!(sane.dispatch(0xa9eb, &mut cpu, &mut m).unwrap());
        let input = classic_core::extended::Extended80 {
            sign_exp: 0x3fff,
            significand: 1 << 63,
        }
        .to_bytes();
        m.put(VALUE as usize, &input).unwrap();
        cpu.pc = 0x6000;
        cpu.dar[15] = 0x700000;
        m.write_word(0x700000, 0x1e);
        m.write_long(0x700002, VALUE);
        assert_eq!(
            sane.dispatch(0xa9ec, &mut cpu, &mut m),
            Err(classic_core::sane::Error::HaltUnsupported(0x1000))
        );
        assert_eq!(cpu.pc, 0x6000);
        assert_eq!(cpu.dar[15], 0x700000);
        let observed: Vec<u8> = (0..10).map(|i| m.read_byte(VALUE + i)).collect();
        assert_eq!(observed.as_slice(), &input);
        m.write_long(0x700002, 0x7ffffc);
        assert!(matches!(
            sane.dispatch(0xa9ec, &mut cpu, &mut m),
            Err(classic_core::sane::Error::Address(_, 10))
        ));
        assert_eq!(cpu.pc, 0x6000);
    }
}
