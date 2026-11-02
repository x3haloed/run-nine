//! Native-shaped PenPat calls through the same dispatcher as applications.
use crate::{machine::Memory, managers::{Heap, Rect}, quickdraw::{self, QuickDraw}};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
fn hex(m: &mut Memory, p: u32, n: u32) -> String {
    (0..n).map(|i|format!("{:02x}",m.read_byte(p+i))).collect()
}
fn state(m: &mut Memory,q: &QuickDraw,port:u32)->Result<String,String>{
    let mut s=hex(m,port+48,10);
    for b in q.pattern(m,port,58)? {s.push_str(&format!("{b:02x}"));}
    Ok(s)
}
pub fn probe()->Result<String,String>{
    let mut records=Vec::new();
    for kind in 0..2 {for (pattern,bytes) in [[255u8;8],[0;8],[170,85,170,85,170,85,170,85],[129,66,36,24,24,36,66,129]].into_iter().enumerate(){
        for seed in [0x12345678u32,0] {for flags in [16,31,0] {
            let mut m=Memory::zeroed(0x800000);let mut heap=Heap::new(0x20000,0xe0000);
            let mut q=QuickDraw::new();q.init(&mut m,0x100000)?;
            let port=0x12000;q.open_port(&mut m,&mut heap,port)?;q.set_port(&mut m,port)?;
            if kind==1 {q.make_color(&mut m,&mut heap,port)?;} else {
                m.write_long(port+2,0x16000);m.write_word(port+6,4);
                let bounds=Rect{top:0,left:0,bottom:32,right:32};quickdraw::write_rect(&mut m,port+8,bounds);quickdraw::write_rect(&mut m,port+16,bounds);
                for offset in [24,28] {let h=m.read_long(port+offset);let p=m.read_long(h);quickdraw::write_rect(&mut m,p+2,bounds);}
            }
            q.move_to(&mut m,-7,11)?;q.pen_size(&mut m,3,5)?;q.set_word(&mut m,56,10)?;
            let before=state(&mut m,&q,port)?;let handle=if kind==1{m.read_long(port+58)}else{0};
            m.write_long(0x14000,0xa1b2c3d4);m.write_long(0x1400c,0x10203040);
            for (i,b) in bytes.into_iter().enumerate(){m.write_byte(0x14004+i as u32,b);}
            let mut cpu=CpuCore::new();cpu.pc=0x18000;cpu.dar[15]=0x17000;cpu.dar[0]=seed;cpu.set_ccr(flags);
            m.write_long(0x17000,0x14004);m.write_long(0x17004,0x55667788);
            m.write_word(0x220,(-123i16)as u16);m.write_word(0xa60,(-192i16)as u16);
            assert!(quickdraw::pen_pattern_dispatch(0xa89d,&mut cpu,&mut m,&q)?);assert_eq!(cpu.pc,0x18002);
            if kind==1{assert_eq!(m.read_long(port+58),handle);}
            let mem=m.read_word(0x220)as i16;let res=m.read_word(0xa60)as i16;let after=state(&mut m,&q,port)?;
            if kind==0{q.frame_rect(&mut m,Rect{top:5,left:7,bottom:25,right:26})?;}
            records.push(json!({"kind":kind,"pattern":pattern,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0x17004,"guard":m.read_long(cpu.dar[15]),"prefix":m.read_long(0x14000),"suffix":m.read_long(0x1400c),"mem_error":mem,"res_error":res,"source_hex":hex(&mut m,0x14004,8),"before_hex":before,"after_hex":after,"pixels_hex":hex(&mut m,0x16000,if kind==0{128}else{0})}));
        }}
    }}
    Ok(json!({"schema":1,"probe":"pen-pattern-runtime-raw-v1","records":records}).to_string())
}
