//! Production cursor API replay; movement coordinates are retained native inputs.
use crate::machine::Memory;
use classic_core::cursor::{STATE, VISIBLE, OBSCURE};
use m68k::{AddressBus, CpuCore};
use serde_json::{json, Value};
const OPS: [u16; 21] = [0,2,2,1,1,2,2,2,1,3,2,1,2,0,3,1,1,2,2,2,0];
const TRAPS: [u16; 4] = [0xa850,0xa852,0xa853,0xa856];
fn state(m: &mut Memory) -> (i16,u8,u8) {
    (m.read_word(STATE) as i16,m.read_byte(VISIBLE),m.read_byte(OBSCURE))
}
pub fn state_probe() -> Result<String,String> {
    let mut m = Memory::zeroed(0x10000);
    let mut cpu = CpuCore::new();
    let mut records = Vec::new();
    for seed in [0x12345678u32,0] {
        for flags in [16u8,31,0] {
            classic_core::cursor::initialize(&mut m);
            for (step,op) in OPS.into_iter().enumerate() {
                let before=state(&mut m);
                cpu.pc=0x6000;cpu.dar[15]=0xf000;cpu.dar[0]=seed;cpu.set_ccr(flags);
                m.write_long(0xf000,0x55667788);
                m.write_word(0x220,(-123i16)as u16);m.write_word(0xa60,(-192i16)as u16);
                assert!(crate::cursors::dispatch(TRAPS[op as usize],&mut cpu,&mut m)?);
                let after=state(&mut m);assert_eq!(cpu.pc,0x6002);
                records.push(json!({"input_d0":seed,"input_ccr":flags,"step":step,"op":op,
                    "before_state":before.0,"before_visible":before.1,"before_8d2":before.2,
                    "after_state":after.0,"after_visible":after.1,"after_8d2":after.2,
                    "d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-0xf000,
                    "guard":m.read_long(0xf000),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16}));
            }
        }
    }
    Ok(json!({"schema":1,"probe":"cursor-state-runtime-raw-v1","records":records}).to_string())
}
pub fn motion_probe() -> Result<String,String> {
    let input: Value=serde_json::from_str(include_str!("../../probes/cursormotion/movements.json")).map_err(|e|e.to_string())?;
    let mut m=Memory::zeroed(0x10000);let mut cpu=CpuCore::new();let mut records=Vec::new();
    for row in input["movements"].as_array().ok_or("Missing movement inputs")? {
        crate::cursors::dispatch(0xa850,&mut cpu,&mut m)?;
        if row["hidden"]==1 { crate::cursors::dispatch(0xa852,&mut cpu,&mut m)?; }
        crate::cursors::dispatch(0xa856,&mut cpu,&mut m)?;
        let before=state(&mut m);
        if row["start"]!=row["end"] { classic_core::cursor::moved(&mut m); }
        let after=state(&mut m);
        records.push(json!({"hidden":row["hidden"],"start":row["start"],"end":row["end"],
            "before_state":before.0,"before_visible":before.1,"before_8d2":before.2,
            "after_state":after.0,"after_visible":after.1,"after_8d2":after.2}));
    }
    Ok(json!({"schema":1,"probe":"cursor-motion-runtime-raw-v1","records":records}).to_string())
}
