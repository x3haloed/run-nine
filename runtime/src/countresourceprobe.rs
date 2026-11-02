//! Owned resource-chain count replay through the production Resource Manager.
use crate::{
    files::{Files, Fork},
    machine::Memory,
    managers::Heap,
    resfiles::ResFiles,
    resource::{self, Data, Resource},
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
use std::collections::BTreeMap;
const STACK: u32 = 0x700000;
struct Scene {
    m: Memory,
    heap: Heap,
    files: Files,
    manager: ResFiles,
    cache: BTreeMap<([u8; 4], i16), u32>,
    cpu: CpuCore,
}
fn data(id: i16, name: &[u8], bytes: &[u8]) -> Data {
    Data {
        kind: *b"pCnt",
        id,
        attributes: 0,
        name: Some(name.to_vec()),
        bytes: bytes.to_vec(),
    }
}
impl Scene {
    fn new() -> Result<Self, String> {
        let mut s = Self {
            m: Memory::zeroed(0x800000),
            heap: Heap::new(0x20000, 0x500000),
            files: Files::new(),
            manager: ResFiles::default(),
            cache: BTreeMap::new(),
            cpu: CpuCore::new(),
        };
        let system = s
            .heap
            .new_handle(&mut s.m, 30, true)
            .map_err(|e| e.to_string())?;
        let app = s
            .heap
            .new_handle(&mut s.m, 30, true)
            .map_err(|e| e.to_string())?;
        for h in [system, app] {
            let p = s.m.read_long(h);
            s.m.put(p as usize, &resource::empty_fork()[256..])?;
        }
        let p = s.m.read_long(app);
        s.m.write_long(p + 16, system);
        s.m.write_word(p + 20, 2);
        s.m.write_long(0xa50, app);
        s.m.write_word(0xa5a, 2);
        Ok(s)
    }
    fn call(
        &mut self,
        app: &[Resource<'_>],
        op: u16,
        args: &[(u32, u16)],
        result: bool,
        seed: u32,
        flags: u8,
    ) -> Result<u16, String> {
        let mut offset = 0;
        for &(v, n) in args {
            match n {
                1 => self.m.write_byte(STACK + offset, v as u8),
                2 => self.m.write_word(STACK + offset, v as u16),
                4 => self.m.write_long(STACK + offset, v),
                _ => return Err("owned count argument width".into()),
            };
            offset += if n == 1 { 2 } else { u32::from(n) };
        }
        if result {
            self.m.write_word(STACK + offset, 0x7b7b);
        }
        let guard = STACK + offset + if result { 2 } else { 0 };
        self.m.write_long(guard, 0x55667788);
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = STACK;
        self.cpu.dar[0] = seed;
        self.cpu.set_ccr(flags);
        assert!(self.manager.dispatch(
            op,
            &mut self.cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.files,
            app,
            &mut self.cache,
            |_, _, _, _| Err("count probe unexpectedly loaded an application resource".into())
        )?);
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.cpu.dar[15], STACK + offset);
        assert_eq!(self.m.read_long(guard), 0x55667788);
        Ok(if result {
            self.m.read_word(STACK + offset)
        } else {
            0
        })
    }
    fn open(&mut self, app: &[Resource<'_>], name: &[u8], unique: i16) -> Result<i16, String> {
        self.files
            .create(name, [0; 16])
            .map_err(|e| e.to_string())?;
        let fork = resource::serialize(&[
            data(7, b"Shared", &[1, 2]),
            data(unique, b"Unique", &[3, 4]),
        ])?;
        let f = self
            .files
            .open(name, Fork::Resource, 3)
            .map_err(|e| e.to_string())?;
        self.files.write(f, &fork).map_err(|e| e.to_string())?;
        self.files.close(f).map_err(|e| e.to_string())?;
        self.m.write_byte(0x19000, name.len() as u8);
        for (i, b) in name.iter().enumerate() {
            self.m.write_byte(0x19001 + i as u32, *b);
        }
        let r = self.call(app, 0xa9c4, &[(3, 1), (0, 2), (0x19000, 4)], true, 0, 0)? as i16;
        assert!(r != -1);
        assert_eq!(self.m.read_word(0xa60), 0);
        Ok(r)
    }
}
pub fn probe() -> Result<String, String> {
    let bytes = resource::serialize(&[
        data(7, b"Application shared", &[0x11, 0x22]),
        data(12, b"Application second", &[0x33, 0x44]),
        data(19, b"Application third", &[0x55, 0x66]),
    ])?;
    let app = resource::parse(&bytes)?;
    let mut s = Scene::new()?;
    let mut a = 0i16;
    let mut b = 0i16;
    let mut records = Vec::new();
    for phase in 0..9 {
        match phase {
            1 => a = s.open(&app, b"CountResources-A", 23)?,
            2 => b = s.open(&app, b"CountResources-B", 31)?,
            3 => {
                s.call(&app, 0xa998, &[(2, 2)], false, 0, 0)?;
            }
            4 => {
                s.call(&app, 0xa998, &[(a as u16 as u32, 2)], false, 0, 0)?;
            }
            5 => {
                s.call(&app, 0xa998, &[(0, 2)], false, 0, 0)?;
            }
            6 => {
                s.call(&app, 0xa998, &[(b as u16 as u32, 2)], false, 0, 0)?;
            }
            7 => {
                s.call(&app, 0xa99a, &[(a as u16 as u32, 2)], false, 0, 0)?;
            }
            8 => {
                s.call(&app, 0xa99a, &[(b as u16 as u32, 2)], false, 0, 0)?;
            }
            _ => {}
        }
        for op in 0..2 {
            for kind in [*b"pCnt", *b"pNil"] {
                for seed in [0x12345678, 0] {
                    for flags in [16, 31, 0] {
                        let before = s.m.read_word(0xa5a);
                        s.m.write_word(0x220, (-123i16) as u16);
                        s.m.write_word(0xa60, (-192i16) as u16);
                        let result = s.call(
                            &app,
                            if op == 0 { 0xa99c } else { 0xa80d },
                            &[(u32::from_be_bytes(kind), 4)],
                            true,
                            seed,
                            flags,
                        )?;
                        records.push(json!({"phase":phase,"op":op,"type":u32::from_be_bytes(kind),"input_d0":seed,"input_ccr":flags,"result":result as i16,"d0":s.cpu.dar[0],"ccr":s.cpu.get_sr()&31,"stack_delta":i64::from(s.cpu.dar[15])-i64::from(STACK+4),"guard":s.m.read_long(STACK+6),"mem_error":s.m.read_word(0x220)as i16,"res_error":s.m.read_word(0xa60)as i16,"current_preserved":i32::from(before==s.m.read_word(0xa5a))}));
                    }
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"count-resources-runtime-raw-v1","records":records}).to_string())
}
