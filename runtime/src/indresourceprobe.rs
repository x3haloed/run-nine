//! Owned indexed resource-chain replay through the production Resource Manager.
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
    fn new(raw_map: &[u8]) -> Result<Self, String> {
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
            .new_handle(&mut s.m, raw_map.len() as u32, true)
            .map_err(|e| e.to_string())?;
        let p = s.m.read_long(system);
        s.m.put(p as usize, &resource::empty_fork()[256..])?;
        let p = s.m.read_long(app);
        s.m.put(p as usize, raw_map)?;
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
        result: u32,
        seed: u32,
        flags: u8,
    ) -> Result<u32, String> {
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
        if result == 2 { self.m.write_word(STACK + offset, 0x7b7b); } else if result == 4 { self.m.write_long(STACK + offset, 0x7b7b7b7b); }
        let guard = STACK + offset + result;
        self.m.write_long(guard, 0x55667788);
        self.cpu.pc = 0x6000;
        self.cpu.dar[15] = STACK;
        self.cpu.dar[0] = seed;
        self.cpu.set_ccr(flags);
        let handled = self.manager.dispatch(
            op,
            &mut self.cpu,
            &mut self.m,
            &mut self.heap,
            &mut self.files,
            app,
            &mut self.cache,
            crate::machine::load_resource
        )?;
        if !handled { assert!(crate::machine::resource_enumeration(op,&mut self.cpu,&mut self.m,&mut self.heap,app,&mut self.cache)?); }
        assert_eq!(self.cpu.pc, 0x6002);
        assert_eq!(self.cpu.dar[15], STACK + offset);
        assert_eq!(self.m.read_long(guard), 0x55667788);
        Ok(match result { 2=>u32::from(self.m.read_word(STACK+offset)),4=>self.m.read_long(STACK+offset),_=>0 })
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
        let r = self.call(app, 0xa9c4, &[(3, 1), (0, 2), (0x19000, 4)], 2, 0, 0)? as i16;
        assert!(r != -1);
        assert_eq!(self.m.read_word(0xa60), 0);
        Ok(r)
    }
}

pub fn probe() -> Result<String, String> {
    let bytes = resource::serialize(&[
        data(7,b"Application shared",&[0x11,0x22]),data(12,b"Application second",&[0x33,0x44]),data(19,b"Application third",&[0x55,0x66]),
    ])?;
    let app=resource::parse(&bytes)?;let mut s=Scene::new(app[0].map)?;let mut a=0i16;let mut b=0i16;let mut records=Vec::new();
    for phase in 0..9 {
        match phase {
            1=>a=s.open(&app,b"IndResource-A",23)?,2=>b=s.open(&app,b"IndResource-B",31)?,
            3=>{s.call(&app,0xa998,&[(2,2)],0,0,0)?;},4=>{s.call(&app,0xa998,&[(a as u16 as u32,2)],0,0,0)?;},
            5=>{s.call(&app,0xa998,&[(0,2)],0,0,0)?;},6=>{s.call(&app,0xa998,&[(b as u16 as u32,2)],0,0,0)?;},
            7=>{s.call(&app,0xa99a,&[(a as u16 as u32,2)],0,0,0)?;},8=>{s.call(&app,0xa99a,&[(b as u16 as u32,2)],0,0,0)?;},_=>{}
        }
        for op in 0..2 { for which in 0..11 { for seed in [0x12345678u32,0] { for flags in [16,31,0] {
            let kind=if which==10{*b"pNil"}else{*b"pCnt"};let index=if which==10{1i16}else{which-1};let before=s.m.read_word(0xa5a);
            s.m.write_word(0x220,(-123i16)as u16);s.m.write_word(0xa60,(-192i16)as u16);
            let h=s.call(&app,if op==0{0xa99d}else{0xa80e},&[(index as u16 as u32,2),(u32::from_be_bytes(kind),4)],4,seed,flags)?;
            let mut row=json!({"phase":phase,"op":op,"type":u32::from_be_bytes(kind),"index":index,"input_d0":seed,"input_ccr":flags,"result_handle":h,"d0":s.cpu.dar[0],"ccr":s.cpu.get_sr()&31,"stack_delta":i64::from(s.cpu.dar[15])-i64::from(STACK+6),"guard":s.m.read_long(STACK+10),"mem_error":s.m.read_word(0x220)as i16,"res_error":s.m.read_word(0xa60)as i16,"current_preserved":i32::from(before==s.m.read_word(0xa5a))});
            let(mut home,mut id,mut typ,mut same,mut size,mut name,mut data)=(-1i16,0i16,0u32,0,0u32,String::new(),String::new());
            if h!=0 {
                s.call(&app,0xa9a8,&[(0x1a100,4),(0x1a010,4),(0x1a000,4),(h,4)],0,0,0)?;
                id=s.m.read_word(0x1a000)as i16;typ=s.m.read_long(0x1a010);
                let r=s.call(&app,0xa9a4,&[(h,4)],2,0,0)?as i16;
                home=if r==2{1}else if r==a{2}else if r==b{3}else if r==0{0}else{4};
                size=s.heap.size(h).map_err(|e|e.to_string())?;
                let p=s.heap.data(h).map_err(|e|e.to_string())?;
                for i in 0..size{data.push_str(&format!("{:02x}",s.m.read_byte(p+i)));}
                for i in 0..s.m.read_byte(0x1a100){name.push_str(&format!("{:02x}",s.m.read_byte(0x1a101+u32::from(i))));}
                let h2=s.call(&app,if op==0{0xa99d}else{0xa80e},&[(index as u16 as u32,2),(u32::from_be_bytes(kind),4)],4,0,0)?;same=i32::from(h==h2);
            }
            let r=row.as_object_mut().unwrap();for(k,v)in [("home",json!(home)),("id",json!(id)),("info_type",json!(typ)),("same_handle",json!(same)),("size",json!(size)),("name_hex",json!(name)),("data_hex",json!(data))]{r.insert(k.into(),v);}
            records.push(row);
        }}}}
    }
    Ok(json!({"schema":1,"probe":"ind-resource-runtime-raw-v1","records":records}).to_string())
}
