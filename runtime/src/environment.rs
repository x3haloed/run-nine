//! Declared classic environment, independent of the machine hosting this process.
use m68k::{AddressBus, CpuCore};
pub const CPU_TYPE: m68k::CpuType = m68k::CpuType::M68020;
#[derive(Clone, Copy)]
pub struct Environment {
    pub machine: i16,
    pub system: u16,
    pub processor: i16,
    pub fpu_type: u32,
    pub quickdraw: u32,
    pub keyboard: i16,
    pub appletalk: i16,
    pub system_volume: i16,
    /// Zero means there is no Sound Manager backend. This discovery value must
    /// not advertise channel allocation or playback that we do not implement.
    pub sound_version: u32,
}
impl Environment {
    pub fn initialize<B: AddressBus>(&self, memory: &mut B) {
        // This flat-memory runtime starts in 32-bit Memory Manager mode.
        // StripAddress follows the startup mode, not later writes to this byte.
        memory.write_byte(0xcb2, 1);
    }
    pub fn runtime() -> Self {
        Self {
            machine: 0,
            system: 0x0921,
            processor: 3,
            fpu_type: 0,
            quickdraw: 0x230,
            keyboard: 0,
            appletalk: 0,
            system_volume: -1,
            sound_version: 0,
        }
    }
    pub fn dispatch<B: AddressBus>(&self, opcode: u16, cpu: &mut CpuCore, m: &mut B) -> bool {
        if opcode == 0xa05d && matches!(cpu.dar[0] as u8, 0 | 1) {
            // Like the native PowerPC OS9 reference, this declared flat-memory
            // environment remains in 32-bit mode for either legal request.
            // The OS trap takes/returns D0; the pointer-based glue is outside it.
            cpu.dar[0] = 1;
            cpu.set_ccr(if cpu.flag_x() { 16 } else { 0 });
            cpu.pc += 2;
            return true;
        }
        if opcode == 0xa055 {
            // In 32-bit Memory Manager mode the address is already complete.
            // The native OS trap applies a WORD condition test to its result.
            let result = cpu.dar[0] as u16;
            cpu.set_ccr(
                (if cpu.flag_x() { 16 } else { 0 })
                    | if result == 0 {
                        4
                    } else if result & 0x8000 != 0 {
                        8
                    } else {
                        0
                    },
            );
            cpu.pc += 2;
            return true;
        }
        if opcode == 0xa800 && cpu.dar[0] == 0x000c0008 {
            // Pascal NumVersion record return: overwrite the caller's four-byte
            // result slot, without consuming it. A0/D0 are scratch registers;
            // no playback, channel or callback operation is handled here.
            m.write_long(cpu.dar[15], self.sound_version);
            cpu.set_ccr(
                (if cpu.flag_x() { 16 } else { 0 })
                    | if self.sound_version == 0 {
                        4
                    } else if self.sound_version & 0x80000000 != 0 {
                        8
                    } else {
                        0
                    },
            );
            cpu.pc += 2;
            return true;
        }
        if opcode == 0xa1ad {
            let selector=cpu.dar[0];
            let value = match cpu.dar[0].to_be_bytes() {
                [b's', b'y', b's', b'v'] => Some(self.system as u32),
                [b'q', b'd', b' ', b' '] => Some(self.quickdraw),
                [b'p', b'r', b'o', b'c'] => Some(self.processor as u32),
                [b'f', b'p', b'u', b' '] => Some(self.fpu_type),
                // Stereo output and simultaneous channels. Input routines,
                // stereo-to-mono hardware mixing and16-bit guest data are absent.
                [b's', b'n', b'd', b' '] if self.sound_version!=0 => Some((1<<11)|1),
                _ => None,
            };
            cpu.dar[8] = value.unwrap_or(0);
            cpu.dar[0] = if value.is_some() {
                0
            } else {
                -5551i16 as u16 as u32
            };
            cpu.set_ccr(if value.is_some() { 4 } else { 8 });
            if std::env::args().any(|a|a=="--trace-sound") {println!("discovery: gestalt={selector:#010x} response={:?} error={}",value,cpu.dar[0] as i16);}
            cpu.pc += 2;
            return true;
        }
        if opcode != 0xa090 {
            return false;
        }
        let version = cpu.dar[0] as i16;
        if version <= 0 {
            cpu.dar[0] = (cpu.dar[0] & 0xffff0000) | (-5501i16 as u16 as u32);
            cpu.set_ccr(if cpu.flag_x() { 0x18 } else { 8 });
        } else {
            let p = cpu.dar[8];
            for (offset, value) in [
                (0, version as u16),
                (2, self.machine as u16),
                (4, self.system),
                (6, self.processor as u16),
                (10, self.keyboard as u16),
                (12, self.appletalk as u16),
                (14, self.system_volume as u16),
            ] {
                m.write_word(p + offset, value);
            }
            m.write_byte(p + 8, (self.fpu_type != 0) as u8);
            m.write_byte(p + 9, (self.quickdraw >= 0x100) as u8);
            cpu.dar[0] = if version > 2 {
                -5502i16 as u16 as u32
            } else {
                0
            };
            cpu.set_ccr(if version > 2 { 8 } else { 4 });
        }
        cpu.pc += 2;
        true
    }
}

pub fn addressing_probe() -> Result<String, String> {
    let environment = Environment::runtime();
    let mut memory = m68k::LinearMemoryBus::new(4096);
    environment.initialize(&mut memory);
    let mut cases = Vec::new();
    for input in [0u32, 0x00123456, 0xff123456, 0xffffffff] {
        for input_ccr in [16, 31, 0] {
            let mut cpu = CpuCore::new();
            cpu.pc = 0x100;
            cpu.dar[0] = input;
            cpu.dar[8] = 0x12345678;
            cpu.dar[15] = 0x300;
            cpu.set_ccr(input_ccr);
            if !environment.dispatch(0xa055, &mut cpu, &mut memory) || cpu.pc != 0x102 {
                return Err("StripAddress dispatch/PC failure".into());
            }
            let ccr = (u32::from(cpu.flag_x()) << 4)
                | (u32::from(cpu.flag_n()) << 3)
                | (u32::from(cpu.flag_z()) << 2)
                | (u32::from(cpu.flag_v()) << 1)
                | u32::from(cpu.flag_c());
            cases.push(serde_json::json!({"input":input,"input_ccr":input_ccr,
                "output":cpu.dar[0],"ccr":ccr,"a0":cpu.dar[8],"stack_delta":0x300i32-cpu.dar[15] as i32}));
        }
    }
    Ok(
        serde_json::json!({"schema":1,"probe":"addressing-v1","addressing":{
        "mmu32bit":memory.read_byte(0xcb2),"cases":cases}})
        .to_string(),
    )
}

/// ABI replay with the explicitly declared reference version, independently of
/// the production environment's absent sound backend. No observation file is
/// read by the runtime. A0/D0 are caller-clobbered trap scratch registers.
pub fn sound_version_probe() -> Result<String, String> {
    use m68k::LinearMemoryBus;
    let mut environment = Environment::runtime();
    environment.sound_version = 0x03678000;
    let mut cases = Vec::new();
    for input_ccr in [16, 31, 0] {
        let mut memory = LinearMemoryBus::new(4096);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x100;
        cpu.dar[0] = 0x000c0008;
        cpu.dar[8] = 0x204;
        cpu.dar[15] = 0x300;
        for p in 0x200..0x210 {
            memory.write_byte(p, 0x55);
        }
        memory.write_long(0x300, 0x204);
        memory.write_long(0x304, 0x55555555);
        memory.write_long(0x308, 0x66666666);
        cpu.set_ccr(input_ccr);
        if !environment.dispatch(0xa800, &mut cpu, &mut memory) || cpu.pc != 0x102 {
            return Err("Sound version dispatch/PC failure".into());
        }
        let ccr = (u32::from(cpu.flag_x()) << 4)
            | (u32::from(cpu.flag_n()) << 3)
            | (u32::from(cpu.flag_z()) << 2)
            | (u32::from(cpu.flag_v()) << 1)
            | u32::from(cpu.flag_c());
        let record_hex = (0x200..0x210)
            .map(|p| format!("{:02x}", memory.read_byte(p)))
            .collect::<String>();
        cases.push(serde_json::json!({"input_ccr":input_ccr,"sp_delta":cpu.dar[15] as i32-0x30c,
            "stack_value":memory.read_long(0x300),"stack_tail":[memory.read_long(0x304),memory.read_long(0x308)],
            "ccr":ccr,"record_hex":record_hex}));
    }
    Ok(
        serde_json::json!({"schema":1,"probe":"sound-version-abi-v1",
        "sound":{"reference_profile_version":environment.sound_version,"cases":cases}})
        .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use m68k::LinearMemoryBus;
    #[test]
    fn absent_backend_discovery_does_not_advertise_playback() {
        let environment = Environment::runtime();
        let mut memory = LinearMemoryBus::new(4096);
        let mut cpu = CpuCore::new();
        cpu.pc = 0x100;
        cpu.dar[15] = 0x300;
        cpu.dar[0] = 0x000c0008;
        memory.write_long(0x2fc, 0x12345678);
        memory.write_long(0x300, 0x55555555);
        memory.write_long(0x304, 0x87654321);
        cpu.set_ccr(0x1f);
        assert!(environment.dispatch(0xa800, &mut cpu, &mut memory));
        assert_eq!(memory.read_long(0x300), 0);
        assert_eq!(memory.read_long(0x2fc), 0x12345678);
        assert_eq!(memory.read_long(0x304), 0x87654321);
        assert_eq!((cpu.dar[15], cpu.pc), (0x300, 0x102));
        assert!(cpu.flag_x() && cpu.flag_z());
        assert!(!cpu.flag_n() && !cpu.flag_v() && !cpu.flag_c());
        cpu.dar[0] = u32::from_be_bytes(*b"snd ");
        assert!(environment.dispatch(0xa1ad, &mut cpu, &mut memory));
        assert_eq!(cpu.dar[0] as i16, -5551);
        // Playback/status/other SoundDispatch selectors still fail visibly.
        for (trap, selector) in [
            (0xa807, 0),
            (0xa805, 0),
            (0xa800, 0x03140008),
            (0xa800, 0x000c0018),
        ] {
            cpu.dar[0] = selector;
            let pc = cpu.pc;
            assert!(!environment.dispatch(trap, &mut cpu, &mut memory));
            assert_eq!((cpu.pc, cpu.dar[15], cpu.dar[0]), (pc, 0x300, selector));
        }
    }
}
