//! Process Manager for the runtime's single classic application context.
//! Serial numbers describe this context, not the surrounding host's processes.
use crate::{managers::Heap, resource::Resource};
use m68k::{AddressBus, CpuCore};

pub struct Process {
    name: Vec<u8>,
    signature: u32,
    mode: u32,
    location: u32,
    size: u32,
}
impl Process {
    pub fn new(
        name: &[u8],
        signature: u32,
        mode: u32,
        location: u32,
        size: u32,
    ) -> Result<Self, String> {
        if name.len() > 31 || name.is_empty() || name.contains(&b':') {
            return Err("invalid classic process name".into());
        }
        Ok(Self {
            name: name.to_vec(),
            signature,
            mode,
            location,
            size,
        })
    }
    pub fn from_resources(
        name: &[u8],
        resources: &[Resource<'_>],
        location: u32,
        size: u32,
    ) -> Result<Self, String> {
        let signature = resources
            .iter()
            .find(|r| r.kind == *b"BNDL")
            .and_then(|r| r.data.get(..4))
            .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
            .unwrap_or(0);
        let flags = resources
            .iter()
            .find(|r| r.kind == *b"SIZE" && r.id == -1)
            .and_then(|r| r.data.get(..2))
            .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
            .unwrap_or(0);
        // Process mode exposes documented SIZE capability bits. Reserved and
        // save-screen bits are not process capabilities in this profile.
        Self::new(name, signature, (flags & 0x5ff8) as u32, location, size)
    }
    pub fn initialize_application<B: AddressBus>(&self, m: &mut B, resource_reference: i16) {
        m.write_word(0x900, resource_reference as u16);
        Self::pstring(m, 0x910, &self.name);
    }
    pub fn set_signature(&mut self, signature: u32) {
        self.signature = signature;
    }
    pub fn signature(&self) -> u32 {
        self.signature
    }
    fn serial<B: AddressBus>(m: &mut B, p: u32) -> (u32, u32) {
        (m.read_long(p), m.read_long(p + 4))
    }
    fn resolve(id: (u32, u32)) -> (u32, u32) {
        if id == (0, 2) {
            (0, 3)
        } else {
            id
        }
    }
    fn write_serial<B: AddressBus>(m: &mut B, p: u32, id: (u32, u32)) {
        m.write_long(p, id.0);
        m.write_long(p + 4, id.1);
    }
    fn pstring<B: AddressBus>(m: &mut B, p: u32, bytes: &[u8]) {
        m.write_byte(p, bytes.len() as u8);
        for (i, b) in bytes.iter().enumerate() {
            m.write_byte(p + 1 + i as u32, *b);
        }
    }
    fn information<B: AddressBus>(&self, m: &mut B, heap: &Heap, id: (u32, u32), p: u32) -> i16 {
        if Self::resolve(id) != (0, 3) {
            return -600;
        }
        if m.read_long(p) < 60 {
            return -50;
        }
        let name = m.read_long(p + 4);
        let spec = m.read_long(p + 56);
        if name != 0 {
            Self::pstring(m, name, &self.name);
        }
        Self::write_serial(m, p + 8, (0, 3));
        m.write_long(p + 16, u32::from_be_bytes(*b"APPL"));
        m.write_long(p + 20, self.signature);
        m.write_long(p + 24, self.mode);
        m.write_long(p + 28, self.location);
        m.write_long(p + 32, self.size);
        m.write_long(p + 36, heap.free_bytes());
        Self::write_serial(m, p + 40, (0, 0)); // No launching classic parent in this context.
        m.write_long(p + 48, 0); // Context boot is the launch time origin.
        let ticks = m.read_long(0x16a);
        m.write_long(p + 52, ticks);
        if spec != 0 {
            m.write_word(spec, crate::files::APPLICATION_VOLUME as u16);
            m.write_long(spec + 2, 2);
            Self::pstring(m, spec + 6, &self.name);
        }
        0
    }
    pub fn dispatch<B: AddressBus>(
        &self,
        op: u16,
        cpu: &mut CpuCore,
        m: &mut B,
        heap: &Heap,
    ) -> Result<bool, String> {
        if op != 0xa88f {
            return Ok(false);
        }
        let sp = cpu.dar[15];
        let selector = m.read_word(sp);
        let (pop, error) = match selector {
            0x37 => {
                let p = m.read_long(sp + 2);
                Self::write_serial(m, p, (0, 3));
                (6, 0)
            }
            0x38 => {
                let p = m.read_long(sp + 2);
                let id = Self::serial(m, p);
                if id == (0, 3) {
                    (6, -600)
                } else {
                    Self::write_serial(m, p, (0, 3));
                    (6, 0)
                }
            }
            0x3a => {
                let info = m.read_long(sp + 2);
                let p = m.read_long(sp + 6);
                let id = Self::serial(m, p);
                (10, self.information(m, heap, id, info))
            }
            0x3d => {
                let out = m.read_long(sp + 2);
                let b = m.read_long(sp + 6);
                let a = m.read_long(sp + 10);
                let a = Self::resolve(Self::serial(m, a));
                let b = Self::resolve(Self::serial(m, b));
                m.write_byte(out, (a == b) as u8);
                (14, 0)
            }
            _ => {
                return Err(format!(
                    "OSDispatch selector {selector:#x} is not implemented"
                ))
            }
        };
        m.write_word(sp + pop, error as u16);
        cpu.dar[15] += pop;
        cpu.pc += 2;
        Ok(true)
    }
}
