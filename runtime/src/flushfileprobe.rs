//! Real PBFlushFile dispatch over owned open/closed data and resource forks.
use crate::{
    files::{Files, Fork},
    machine::Memory,
};
use m68k::{AddressBus, CpuCore};
use serde_json::json;
const PB: u32 = 0x19000;
const STACK: u32 = 0x700000;
fn hex(m: &mut Memory) -> String {
    (0..96)
        .map(|i| format!("{:02x}", m.read_byte(PB + i)))
        .collect()
}
fn make(
    files: &mut Files,
    name: &[u8],
    fork: Fork,
    readonly: bool,
    empty: bool,
) -> Result<i16, String> {
    files.create(name, [0; 16]).map_err(|e| e.to_string())?;
    let mut r = files.open(name, fork, 3).map_err(|e| e.to_string())?;
    if !empty {
        files.write(r, b"ABCDEFGH").map_err(|e| e.to_string())?;
    }
    if readonly {
        files.close(r).map_err(|e| e.to_string())?;
        r = files.open(name, fork, 1).map_err(|e| e.to_string())?;
    }
    if !empty {
        files
            .seek(r, 1, if fork == Fork::Resource { 4 } else { 3 })
            .map_err(|e| e.to_string())?;
    }
    Ok(r)
}
pub fn probe() -> Result<String, String> {
    let mut m = Memory::zeroed(0x800000);
    let mut cpu = CpuCore::new();
    let mut files = Files::new();
    let refs = [
        make(&mut files, b"FlushDFWrite", Fork::Data, false, false)?,
        make(&mut files, b"FlushDFRead", Fork::Data, true, false)?,
        make(&mut files, b"FlushRFWrite", Fork::Resource, false, false)?,
        make(&mut files, b"FlushRFRead", Fork::Resource, true, false)?,
        make(&mut files, b"FlushEmpty", Fork::Data, false, true)?,
        make(&mut files, b"FlushClosed", Fork::Data, false, false)?,
        0,
        -1,
        12345,
    ];
    files.close(refs[5]).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    for (kind, reference) in refs.into_iter().enumerate() {
        for volume in 0..2 {
            for seed in [0x12345678u32, 0] {
                for flags in [16, 31, 0] {
                    let mb = files.position(reference);
                    let eb = files.eof(reference);
                    for i in 0..96 {
                        m.write_byte(PB + i, 0xa5);
                    }
                    m.write_long(PB + 12, 0);
                    m.write_long(PB + 18, 0);
                    m.write_word(PB + 22, if volume == 0 { 0 } else { 0x1357 });
                    m.write_word(PB + 24, reference as u16);
                    m.write_word(PB + 16, 0x7b7b);
                    let before = hex(&mut m);
                    cpu.pc = 0x6000;
                    cpu.dar[15] = STACK;
                    cpu.dar[8] = PB;
                    cpu.dar[0] = seed;
                    cpu.set_ccr(flags);
                    m.write_long(STACK, 0x55667788);
                    m.write_word(0x220, (-123i16) as u16);
                    m.write_word(0xa60, (-192i16) as u16);
                    assert!(files.dispatch(0xa045, &mut cpu, &mut m)?);
                    assert_eq!(cpu.pc, 0x6002);
                    let ma = files.position(reference);
                    let ea = files.eof(reference);
                    records.push(json!({"kind":kind,"volume_input":volume,"reference":reference,"input_d0":seed,"input_ccr":flags,"d0":cpu.dar[0],"ccr":cpu.get_sr()&31,"stack_delta":i64::from(cpu.dar[15])-i64::from(STACK),"guard":m.read_long(STACK),"a0_preserved":i32::from(cpu.dar[8]==PB),"mem_error":m.read_word(0x220)as i16,"res_error":m.read_word(0xa60)as i16,"mark_before_error":mb.as_ref().err().copied().unwrap_or(0),"mark_after_error":ma.as_ref().err().copied().unwrap_or(0),"mark_before":mb.unwrap_or(0)as i32,"mark_after":ma.unwrap_or(0)as i32,"eof_before_error":eb.as_ref().err().copied().unwrap_or(0),"eof_after_error":ea.as_ref().err().copied().unwrap_or(0),"eof_before":eb.unwrap_or(0)as i32,"eof_after":ea.unwrap_or(0)as i32,"before_hex":before,"after_hex":hex(&mut m)}));
                }
            }
        }
    }
    Ok(json!({"schema":1,"probe":"flush-file-runtime-raw-v1","records":records}).to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flush_commits_both_forks_while_reference_and_position_stay_open() {
        let root = std::env::temp_dir().join(format!(
            "classic-flush-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = Files::volume(&root).unwrap();
        f.create(b"Owned", [0; 16]).unwrap();
        let data = f.open(b"Owned", Fork::Data, 3).unwrap();
        let res = f.open(b"Owned", Fork::Resource, 3).unwrap();
        f.write(data, b"new data").unwrap();
        f.write(res, b"resource bytes").unwrap();
        f.seek(data, 1, 2).unwrap();
        f.seek(res, 1, 4).unwrap();
        assert!(f.flush_file(data).unwrap());
        assert_eq!(
            std::fs::read(root.join("files/6f776e6564/data")).unwrap(),
            b"new data"
        );
        assert_eq!(
            std::fs::read(root.join("files/6f776e6564/resource")).unwrap(),
            b"resource bytes"
        );
        assert_eq!(f.position(data), Ok(2));
        assert_eq!(f.position(res), Ok(4));
        assert_eq!(f.eof(data), Ok(8));
        assert_eq!(f.eof(res), Ok(14));
        assert!(!f.flush_file(data).unwrap());
        f.write(data, b"X").unwrap();
        assert!(f.flush_file(data).unwrap());
        assert_eq!(
            std::fs::read(root.join("files/6f776e6564/data")).unwrap(),
            b"neX data"
        );
        f.close(data).unwrap();
        assert_eq!(f.flush_file(data), Err(-38));
        assert_eq!(f.flush_file(-1), Err(-51));
        drop(f);
        std::fs::remove_dir_all(root).unwrap();
    }
}
