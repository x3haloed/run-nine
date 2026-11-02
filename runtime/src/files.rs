//! Macintosh fork/position semantics over an owned logical volume.
//! Host filenames are encoded; guest paths never become arbitrary host paths.
use m68k::AddressBus;
use std::{
    cell::Cell,
    collections::{BTreeMap, BTreeSet},
    fs,
    hash::{Hash, Hasher},
    io::Write,
    path::{Path, PathBuf},
};
pub const APPLICATION_VOLUME: i16 = -2;
pub const APPLICATION_RESOURCE_FILE: i16 = 2;
const MAX_FORK: usize = 32 * 1024 * 1024;
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Fork {
    Data,
    Resource,
}
struct File {
    id: u32,
    parent: u32,
    name: Vec<u8>,
    finder: [u8; 16],
    extended: [u8; 16],
    dates: [u32; 3],
    data: Vec<u8>,
    resource: Vec<u8>,
}
struct Open {
    dirty: bool,
    key: Vec<u8>,
    fork: Fork,
    mark: usize,
    permission: u8,
}
pub struct Files {
    catalog: classic_core::catalog::Catalog,
    current_dir: u32,
    files: BTreeMap<Vec<u8>, File>,
    open: BTreeMap<i16, Open>,
    issued: BTreeSet<i16>,
    next_ref: i16,
    root: Option<PathBuf>,
    volume_lock: Option<fs::File>,
    checkpointed: BTreeSet<Vec<u8>>,
    source: Option<Box<Files>>,
    read_only: bool,
    volume_ref: i16,
    volume_name: Vec<u8>,
    current_volume: i16,
    reference_limit: i16,
    allocation_blocks: u32,
    volume_created: u32,
    volume_modified: Cell<u32>,
    volume_backup: u32,
    volume_writes: Cell<u32>,
    committed_state: Cell<Option<u64>>,
}
fn key(name: &[u8]) -> Result<Vec<u8>, i16> {
    if name.is_empty() || name.len() > 31 || name.contains(&0) || name.contains(&b':') {
        return Err(-37);
    }
    Ok(name
        .iter()
        .flat_map(|b| crate::mac_roman::MAC_ROMAN[*b as usize].to_lowercase())
        .collect::<String>()
        .into_bytes())
}
fn encoded(key: &[u8]) -> String {
    key.iter().map(|b| format!("{b:02x}")).collect()
}
impl File {
    fn fork(&self, f: Fork) -> &Vec<u8> {
        match f {
            Fork::Data => &self.data,
            Fork::Resource => &self.resource,
        }
    }
    fn fork_mut(&mut self, f: Fork) -> &mut Vec<u8> {
        match f {
            Fork::Data => &mut self.data,
            Fork::Resource => &mut self.resource,
        }
    }
}
impl Files {
    fn now() -> u32 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|t| t.as_secs().saturating_add(2082844800).min(u32::MAX as u64) as u32)
            .unwrap_or(0)
    }
    /// Only the logical revision counter uses this change detector. Fork and
    /// catalog bytes are always persisted by flush, even on a hash collision.
    fn state_hash(&self) -> u64 {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.allocation_blocks.hash(&mut h);
        self.volume_created.hash(&mut h);
        self.volume_modified.get().hash(&mut h);
        self.volume_backup.hash(&mut h);
        self.catalog.next_id().hash(&mut h);
        for (key, f) in &self.files {
            key.hash(&mut h);
            f.id.hash(&mut h);
            f.parent.hash(&mut h);
            f.name.hash(&mut h);
            f.finder.hash(&mut h);
            f.extended.hash(&mut h);
            f.dates.hash(&mut h);
            f.data.hash(&mut h);
            f.resource.hash(&mut h);
        }
        for (id, d) in self.catalog.entries() {
            id.hash(&mut h);
            d.parent.hash(&mut h);
            d.name.hash(&mut h);
            d.metadata.finder.hash(&mut h);
            d.metadata.extended.hash(&mut h);
            d.metadata.dates.hash(&mut h);
        }
        h.finish()
    }
    fn used_blocks(&self) -> u64 {
        let forks = self
            .files
            .values()
            .map(|f| {
                classic_core::volume::fork_blocks(f.data.len())
                    + classic_core::volume::fork_blocks(f.resource.len())
            })
            .sum();
        classic_core::volume::used_blocks(self.files.len() + self.catalog.entries().count(), forks)
    }
    fn check_growth(&self, old_len: usize, new_len: usize, new_objects: usize) -> Result<(), i16> {
        let n = self.files.len() + self.catalog.entries().count();
        let delta = classic_core::volume::fork_blocks(new_len)
            .saturating_sub(classic_core::volume::fork_blocks(old_len))
            + classic_core::volume::catalog_blocks(n + new_objects)
            - classic_core::volume::catalog_blocks(n);
        if delta != 0
            && self.used_blocks().saturating_add(delta) > u64::from(self.allocation_blocks)
        {
            Err(-34)
        } else {
            Ok(())
        }
    }
    fn volume_record<B: AddressBus>(&self, m: &mut B, p: u32, directory: u32) {
        let free = u64::from(self.allocation_blocks).saturating_sub(self.used_blocks()) as u32;
        let (total, block_bytes, available) =
            classic_core::volume::legacy_geometry(self.allocation_blocks, free);
        let name = m.read_long(p + 18);
        if name != 0 {
            m.write_byte(name, self.volume_name.len() as u8);
            for (i, b) in self.volume_name.iter().enumerate() {
                m.write_byte(name + 1 + i as u32, *b);
            }
        }
        m.write_word(p + 22, self.volume_ref as u16);
        m.write_long(p + 30, self.volume_created);
        m.write_long(p + 34, self.volume_modified.get());
        m.write_word(p + 38, if self.read_only { 0x0080 } else { 0 });
        m.write_word(
            p + 40,
            (self
                .files
                .values()
                .filter(|f| f.parent == directory)
                .count()
                + self
                    .catalog
                    .entries()
                    .filter(|(_, d)| d.parent == directory)
                    .count())
            .min(u16::MAX as usize) as u16,
        );
        m.write_word(p + 42, 0);
        m.write_word(
            p + 44,
            (self.used_blocks() * u64::from(classic_core::volume::BLOCK_BYTES)
                / u64::from(block_bytes))
            .min(u64::from(total.saturating_sub(1))) as u16,
        );
        m.write_word(p + 46, total);
        m.write_long(p + 48, block_bytes);
        m.write_long(p + 52, 65536);
        m.write_word(p + 56, 0);
        m.write_long(p + 58, self.catalog.next_id());
        m.write_word(p + 62, available);
        m.write_word(p + 64, 0x4244);
        m.write_word(
            p + 66,
            if self.volume_ref == APPLICATION_VOLUME {
                2
            } else {
                1
            },
        );
        // Negative references identify the owned virtual storage backends;
        // raw device/control operations are not provided by this File Manager.
        m.write_word(p + 68, self.volume_ref as u16);
        m.write_word(p + 70, 0);
        m.write_long(p + 72, self.volume_backup);
        m.write_word(p + 76, 0);
        m.write_long(p + 78, self.volume_writes.get());
        m.write_long(p + 82, self.files.len() as u32);
        m.write_long(p + 86, self.catalog.entries().count() as u32);
        for i in 0..8 {
            m.write_long(p + 90 + i * 4, 0);
        }
    }
    fn get_volume_info<B: AddressBus>(&self, m: &mut B, p: u32) -> Result<(), i16> {
        let index = m.read_word(p + 28) as i16;
        let target = if index > 0 {
            if index == 1 {
                self.volume_ref
            } else if index == 2 {
                self.source.as_ref().ok_or(-35i16)?.volume_ref
            } else {
                return Err(-35);
            }
        } else {
            let v = m.read_word(p + 22) as i16;
            if v == 0 {
                self.current_volume
            } else {
                v
            }
        };
        let owner = if target == self.volume_ref {
            self
        } else {
            self.source
                .as_ref()
                .filter(|s| s.volume_ref == target)
                .ok_or(-35i16)?
        };
        let directory = if index <= 0 && m.read_word(p + 22) == 0 {
            owner.current_dir
        } else {
            2
        };
        owner.volume_record(m, p, directory);
        Ok(())
    }
    fn catalog_probe_row<B: AddressBus>(
        &mut self,
        m: &mut B,
        index: usize,
        volume: i16,
        dir: u32,
        enumeration: i16,
        path: &[u8],
        nil_name: bool,
        known: i16,
        ids: &[u32; 5],
    ) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let p = 0x4008;
        let np = 0x5000;
        for i in 0..124 {
            m.write_byte(0x4000 + i, 0x55);
        }
        for i in 0..256 {
            m.write_byte(np + i, 0x55);
        }
        m.write_byte(np, path.len() as u8);
        for (i, b) in path.iter().enumerate() {
            m.write_byte(np + 1 + i as u32, *b);
        }
        m.write_long(p + 12, 0);
        m.write_long(p + 18, if nil_name { 0 } else { np });
        m.write_word(p + 22, volume as u16);
        m.write_long(p + 48, dir);
        m.write_word(p + 28, enumeration as u16);
        m.write_byte(p + 31, 0);
        let mut cpu = m68k::CpuCore::new();
        cpu.dar[0] = 9;
        cpu.dar[8] = p;
        cpu.dar[15] = 0x700000;
        cpu.pc = 0x6000;
        cpu.set_ccr(0x10);
        self.dispatch(0xa260, &mut cpu, m)?;
        if cpu.pc != 0x6002 || cpu.dar[15] != 0x700000 || cpu.dar[8] != p || !cpu.flag_x() {
            return Err("Catalog query altered caller ABI".into());
        }
        let object = m.read_long(p + 48);
        let parent = m.read_long(p + 100);
        let reference = m.read_word(p + 24) as i16;
        let role = |id: u32| -> i16 {
            if id <= 2 {
                id as i16
            } else {
                ids.iter()
                    .position(|n| *n == id)
                    .map(|i| i as i16 + 3)
                    .unwrap_or(-1)
            }
        };
        let original_tag = |id: u32| -> i16 {
            let r = role(id);
            if r > 5 {
                -1
            } else {
                r
            }
        };
        let name: String = (0..33)
            .map(|i| format!("{:02x}", m.read_byte(np + i)))
            .collect();
        let body: String = (32..108)
            .map(|i| {
                format!(
                    "{:02x}",
                    if ((48..52).contains(&i) && role(object) >= 3)
                        || ((100..104).contains(&i) && role(parent) >= 3)
                    {
                        0
                    } else {
                        m.read_byte(p + i)
                    }
                )
            })
            .collect();
        let guards =
            (0..8).all(|i| m.read_byte(0x4000 + i) == 0x55 && m.read_byte(p + 108 + i) == 0x55);
        Ok(
            json!({"case":index,"error":cpu.dar[0] as i16,"io_result":m.read_word(p+16) as i16,"volume_same":i16::from(m.read_word(p+22) as i16==volume),"ref":[i16::from(reference==0),i16::from(reference==0x5555),i16::from(known>0&&reference==known)],"version_filler_hex":format!("{:02x}{:02x}",m.read_byte(p+26),m.read_byte(p+27)),"index_out":m.read_word(p+28) as i16,"attributes":m.read_byte(p+30),"access":m.read_byte(p+31),"object_tag":original_tag(object),"object_positive":i16::from(object as i32>0),"parent_tag":original_tag(parent),"identity_roles":[role(object),role(parent)],"guards":i16::from(guards),"name33_hex":name,"body_hex":body}),
        )
    }
    pub fn catalog_probe<B: AddressBus>(m: &mut B) -> Result<String, String> {
        use serde_json::json;
        let mut f = Self::new();
        let volume = f.volume_ref;
        let err = |e: i16| e.to_string();
        let base = f.create_directory(2, b"Catalog-owned").map_err(err)?;
        let child = f.create_directory(base, b"Child").map_err(err)?;
        for (d, n, data, resource) in [
            (base, &b"Alpha"[..], &b"ABCDEF"[..], &b"RS"[..]),
            (base, b"beta", b"B", b""),
            (child, b"Alpha", b"CHLD", b""),
        ] {
            let mut finder = [0; 16];
            finder[..8].copy_from_slice(b"TEXTpPrb");
            f.create_at(d, n, finder).map_err(err)?;
            for (fork, bytes) in [(Fork::Data, data), (Fork::Resource, resource)] {
                let r = f.open_at(d, n, fork, 3).map_err(err)?;
                f.write(r, bytes).map_err(err)?;
                f.close(r).map_err(err)?;
            }
            f.files
                .get_mut(&Self::file_key(d, n).map_err(err)?)
                .unwrap()
                .dates = [0x12345678, 0x12345679, 0x1234567a];
        }
        for id in [base, child] {
            f.catalog.metadata_mut(id).unwrap().dates = [0x12345678, 0x12345679, 0x1234567a];
        }
        let alpha = f.files[&Self::file_key(base, b"Alpha").map_err(err)?].id;
        let beta = f.files[&Self::file_key(base, b"beta").map_err(err)?].id;
        let nested = f.files[&Self::file_key(child, b"Alpha").map_err(err)?].id;
        let ids = [base, child, alpha, beta, nested];
        let pb = 0x4008;
        m.write_word(pb + 22, volume as u16);
        m.write_long(pb + 48, base);
        f.parameter_block(m, 0xa215, pb).map_err(err)?;
        let inputs: Vec<(i16, u32, i16, &[u8], bool)> = vec![
            (0, 0, 0, b"Marathon Preferences", false),
            (volume, base, 0, b"Alpha", false),
            (volume, base, 0, b"Child", false),
            (volume, base, -1, b"IGNORED", false),
            (volume, child, -1, b"IGNORED", false),
            (volume, base, 0, b"aLPHa", false),
            (volume, base, 0, b":Child:Alpha", false),
            (volume, base, 0, b":Missing:Alpha", false),
            (volume, 999999, 0, b"Alpha", false),
            (-123, base, 0, b"Alpha", false),
            (volume, base, 1, b"IGNORED", false),
            (volume, base, 2, b"IGNORED", false),
            (volume, base, 3, b"IGNORED", false),
            (volume, base, 4, b"IGNORED", false),
            (volume, base, 0, b"", false),
            (volume, base, 0, b"IGNORED", true),
            (volume, base, -1, b"IGNORED", true),
            (volume, base, -32768, b"IGNORED", false),
            (volume, alpha, 1, b"IGNORED", false),
        ];
        let mut rows = Vec::new();
        for (i, (v, d, index, name, nil)) in inputs.iter().enumerate() {
            rows.push(f.catalog_probe_row(m, i, *v, *d, *index, name, *nil, 0, &ids)?);
        }
        let r = f.open_at(base, b"Alpha", Fork::Data, 3).map_err(err)?;
        rows.push(f.catalog_probe_row(m, 19, volume, base, 0, b"Alpha", false, r, &ids)?);
        f.close(r).map_err(err)?;
        let r = f.open_at(base, b"Alpha", Fork::Resource, 1).map_err(err)?;
        rows.push(f.catalog_probe_row(m, 20, volume, base, 0, b"Alpha", false, r, &ids)?);
        f.close(r).map_err(err)?;
        for (i, (v, d, index, name)) in [
            (21, (volume, base, 0, &b"Alpha"[..])),
            (22, (volume, base, 0, b":Alpha:")),
            (23, (volume, 0, 0, b"Missing")),
            (24, (volume, child, 0, b"::Alpha")),
            (25, (volume, child, 1, b"IGNORED")),
            (26, (volume, 999999, -1, b"IGNORED")),
            (27, (volume, base, 0, b"::Missing")),
        ] {
            rows.push(f.catalog_probe_row(m, i, v, d, index, name, false, 0, &ids)?);
        }
        Ok(json!({"schema":1,"probe":"catalog-semantics-v1","files":{"cases":rows}}).to_string())
    }
    fn fcb_probe_row<B: AddressBus>(
        &mut self,
        m: &mut B,
        index: usize,
        reference: i16,
        volume: i16,
        enumeration: i16,
        nil_name: bool,
        baseline: &mut u32,
    ) -> Result<serde_json::Value, String> {
        use serde_json::json;
        let p = 0x4008;
        let name = 0x5000;
        for i in 0..78 {
            m.write_byte(0x4000 + i, 0x55);
        }
        for i in 0..256 {
            m.write_byte(name + i, 0x55);
        }
        m.write_long(p + 12, 0);
        m.write_long(p + 18, if nil_name { 0 } else { name });
        m.write_word(p + 22, volume as u16);
        m.write_word(p + 24, reference as u16);
        m.write_word(p + 28, enumeration as u16);
        let mut cpu = m68k::CpuCore::new();
        cpu.dar[0] = 8;
        cpu.dar[8] = p;
        cpu.dar[15] = 0x700000;
        cpu.pc = 0x6000;
        cpu.set_ccr(0x10);
        self.dispatch(0xa260, &mut cpu, m)?;
        if cpu.dar[15] != 0x700000 || cpu.pc != 0x6002 || cpu.dar[8] != p || !cpu.flag_x() {
            return Err("FCB query changed caller ABI".into());
        }
        let error = cpu.dar[0] as i16;
        if index == 0 && error == 0 {
            *baseline = m.read_long(p + 32);
        }
        let id = m.read_long(p + 32);
        let out_ref = m.read_word(p + 24) as i16;
        let out_volume = m.read_word(p + 22) as i16;
        let spelling: String = (0..33)
            .map(|i| format!("{:02x}", m.read_byte(name + i)))
            .collect();
        let guards =
            (0..8).all(|i| m.read_byte(0x4000 + i) == 0x55 && m.read_byte(p + 62 + i) == 0x55);
        Ok(
            json!({"case":index,"error":error,"io_result":m.read_word(p+16) as i16,"ref_same":i16::from(out_ref==reference),"ref_zero":i16::from(out_ref==0),"index_out":m.read_word(p+28) as i16,
            "volume_out":[i16::from(out_volume==volume),i16::from(out_volume==self.volume_ref),i16::from(out_volume==0)],"file_id":[i16::from(id as i32>0),i16::from(id==*baseline)],
            "flags":m.read_word(p+36),"eof":m.read_long(p+40) as i32,"position":m.read_long(p+48) as i32,"file_volume_same":i16::from(m.read_word(p+52) as i16==self.volume_ref),"parent_same":i16::from(m.read_long(p+58)==2),
            "fillers":[m.read_word(p+26),m.read_word(p+30) as i16],"guards":i16::from(guards),"name33_hex":spelling,"raw_physical":[m.read_word(p+38) as i16,m.read_long(p+44) as i32,m.read_long(p+54) as i32]}),
        )
    }
    pub fn fcb_probe<B: AddressBus>(m: &mut B, resource: &[u8]) -> Result<String, String> {
        use serde_json::json;
        let mut f = Self::new();
        let volume = f.volume_ref;
        let map_error = |e: i16| e.to_string();
        f.create(b"FCB-owned.data", [0; 16]).map_err(map_error)?;
        let r = f
            .open(b"FCB-owned.data", Fork::Data, 3)
            .map_err(map_error)?;
        f.write(r, b"ABCDEF").map_err(map_error)?;
        f.close(r).map_err(map_error)?;
        let r = f
            .open(b"FCB-owned.data", Fork::Resource, 3)
            .map_err(map_error)?;
        f.write(r, b"RS").map_err(map_error)?;
        f.close(r).map_err(map_error)?;
        f.create(b"FCBInfoProbe", [0; 16]).map_err(map_error)?;
        f.files
            .get_mut(&key(b"FCBInfoProbe").map_err(map_error)?)
            .unwrap()
            .resource = resource.to_vec();
        let app = f
            .open(b"FCBInfoProbe", Fork::Resource, 3)
            .map_err(map_error)?;
        let process = crate::processes::Process::new(b"FCBInfoProbe", 0, 0, 0x20000, 0x7e0000)?;
        process.initialize_application(m, app);
        m.write_word(0xa5a, app as u16);
        let startup = json!({"global_ref_same":i16::from(m.read_word(0x900)==m.read_word(0xa5a)),"global_name_same":i16::from(Self::name(m,0x910)==b"FCBInfoProbe")});
        let data = f
            .open(b"FCB-owned.data", Fork::Data, 3)
            .map_err(map_error)?;
        let mut rows = Vec::new();
        let mut baseline = 0;
        rows.push(f.fcb_probe_row(m, 0, data, 0, 0, false, &mut baseline)?);
        f.seek(data, 1, 2).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 1, data, volume, 0, false, &mut baseline)?);
        let ro = f
            .open(b"FCB-owned.data", Fork::Data, 1)
            .map_err(map_error)?;
        f.seek(ro, 1, 3).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 2, ro, 0, 0, false, &mut baseline)?);
        let rf = f
            .open(b"FCB-owned.data", Fork::Resource, 3)
            .map_err(map_error)?;
        f.seek(rf, 1, 1).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 3, rf, 0, 0, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 4, data, volume, 0, true, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 5, 32000, 0, 0, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 6, 0, 0, 0, false, &mut baseline)?);
        f.close(ro).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 7, ro, 0, 0, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 8, data, 0, -1, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 9, data, -123, 0, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 10, data, -123, 1, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 11, data, volume, 32767, false, &mut baseline)?);
        f.seek(app, 1, 5).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 12, app, 0, 0, false, &mut baseline)?);
        rows.push(f.fcb_probe_row(m, 13, app, -123, 0, true, &mut baseline)?);
        f.close(rf).map_err(map_error)?;
        f.close(data).map_err(map_error)?;
        let wo = f
            .open(b"FCB-owned.data", Fork::Data, 2)
            .map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 14, wo, 0, 0, false, &mut baseline)?);
        f.seek(wo, 2, 0).map_err(map_error)?;
        f.write(wo, b"!").map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 15, wo, 0, 0, false, &mut baseline)?);
        let p = 0x4008;
        m.write_word(p + 22, volume as u16);
        f.parameter_block(m, 0xa013, p).map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 16, wo, 0, 0, false, &mut baseline)?);
        f.close(wo).map_err(map_error)?;
        let rf = f
            .open(b"FCB-owned.data", Fork::Resource, 3)
            .map_err(map_error)?;
        f.seek(rf, 2, 0).map_err(map_error)?;
        f.write(rf, b"!").map_err(map_error)?;
        rows.push(f.fcb_probe_row(m, 17, rf, 0, 0, false, &mut baseline)?);
        Ok(
            json!({"schema":1,"probe":"fcb-semantics-v1","startup":startup,"files":{"cases":rows}})
                .to_string(),
        )
    }
    /// Owned path/ABI fixtures through the production data-fork open entry.
    pub fn open_df_probe<B: AddressBus>(m: &mut B) -> Result<String, String> {
        use serde_json::json;
        let mut f = Self::new();
        let volume = f.volume_ref;
        let base = f
            .create_directory(2, b"RuntimeOpenDFProbe")
            .map_err(|e| e.to_string())?;
        let child = f
            .create_directory(base, b"Child")
            .map_err(|e| e.to_string())?;
        let other = f
            .create_directory(base, b"Other")
            .map_err(|e| e.to_string())?;
        for (dir, name, bytes) in [
            (base, &b"Map"[..], &b"BASE"[..]),
            (child, b"Map", b"CHLD"),
            (other, b"OnlyOther", b"OTHR"),
        ] {
            f.create_at(dir, name, [0; 16]).map_err(|e| e.to_string())?;
            let r = f
                .open_at(dir, name, Fork::Data, 3)
                .map_err(|e| e.to_string())?;
            f.write(r, bytes).map_err(|e| e.to_string())?;
            f.close(r).map_err(|e| e.to_string())?;
        }
        f.current_dir = base;
        let p = 0x4004;
        let np = 0x2000;
        let read_pb = 0x5000;
        let buffer = 0x6000;
        let mut cpu = m68k::CpuCore::new();
        let mut records = Vec::new();
        for scenario in 0..12 {
            for hierarchical in 0..2 {
                for input_ccr in [16u8, 31, 0] {
                    for i in 0..256 {
                        m.write_byte(np + i, 0);
                    }
                    let path = match scenario {
                        5 => b":Child:Map".to_vec(),
                        7 => b"Missing".to_vec(),
                        8 => b"Child".to_vec(),
                        9 => b"mAp".to_vec(),
                        10 => [
                            f.volume_name.as_slice(),
                            b":RuntimeOpenDFProbe:Other:OnlyOther",
                        ]
                        .concat(),
                        11 => Vec::new(),
                        _ => b"Map".to_vec(),
                    };
                    m.write_byte(np, path.len() as u8);
                    for (i, b) in path.iter().enumerate() {
                        m.write_byte(np + 1 + i as u32, *b);
                    }
                    for i in 0..80 {
                        m.write_byte(p + i, 0);
                    }
                    m.write_long(p - 4, 0x11223344);
                    m.write_long(p + 80, 0x55667788);
                    let v = if scenario == 3 || scenario == 4 {
                        volume
                    } else if scenario == 6 {
                        32000
                    } else {
                        0
                    };
                    let dir = if scenario == 1 || scenario == 4 {
                        child
                    } else if scenario == 2 {
                        777777
                    } else {
                        0
                    };
                    m.write_long(p + 18, np);
                    m.write_word(p + 22, v as u16);
                    m.write_word(p + 24, 0x1234);
                    m.write_byte(p + 27, 1);
                    m.write_long(p + 48, dir);
                    cpu.pc = 0x7000;
                    cpu.dar[0] = 0x1111001a;
                    cpu.dar[8] = p;
                    cpu.dar[15] = 0x700000;
                    cpu.set_ccr(input_ccr);
                    let before = cpu.dar;
                    assert!(f.dispatch(
                        if hierarchical == 1 { 0xa260 } else { 0xa060 },
                        &mut cpu,
                        m
                    )?);
                    let d0 = cpu.dar[0];
                    let ccr = cpu.get_sr() & 31;
                    let delta = 0x700000i64 - cpu.dar[15] as i64;
                    let a0_preserved = i16::from(cpu.dar[8] == p);
                    assert_eq!(&cpu.dar[1..], &before[1..]);
                    assert_eq!(cpu.pc, 0x7002);
                    let error = m.read_word(p + 16) as i16;
                    let reference = m.read_word(p + 24) as i16;
                    let mut read = vec![12345i64, 4, 85, 85, 85, 85];
                    let mut close_error = 12345i16;
                    if error == 0 {
                        for i in 0..80 {
                            m.write_byte(read_pb + i, 0);
                        }
                        for i in 0..4 {
                            m.write_byte(buffer + i, 0x55);
                        }
                        m.write_word(read_pb + 24, reference as u16);
                        m.write_long(read_pb + 32, buffer);
                        m.write_long(read_pb + 36, 4);
                        cpu.dar[8] = read_pb;
                        f.dispatch(0xa002, &mut cpu, m)?;
                        read[0] = cpu.dar[0] as i16 as i64;
                        read[1] = m.read_long(read_pb + 40) as i64;
                        for i in 0..4 {
                            read[2 + i] = m.read_byte(buffer + i as u32) as i64;
                        }
                        f.dispatch(0xa001, &mut cpu, m)?;
                        close_error = cpu.dar[0] as i16;
                    }
                    let name_preserved = i16::from(
                        m.read_byte(np) == path.len() as u8
                            && (0..path.len()).all(|i| m.read_byte(np + 1 + i as u32) == path[i]),
                    );
                    records.push(json!({"case":scenario,"hierarchical":hierarchical,"input_ccr":input_ccr,"d0":d0,"ccr":ccr,"stack_delta":delta,"a0_preserved":a0_preserved,"io_result":error,"raw_ref":reference,"ref_valid_or_preserved":if error==0 {i16::from(reference>0)}else {i16::from(reference==0x1234)},"volume_preserved":i16::from(m.read_word(p+22)==v as u16),"dir_preserved":i16::from(m.read_long(p+48)==dir),"name_preserved":name_preserved,"read":read,"close_error":close_error,"guards":[m.read_long(p-4),m.read_long(p+80)]}));
                }
            }
        }
        Ok(json!({"schema":1,"probe":"open-df-runtime-raw-v2","raw_environment":{"volume":volume,"base":base,"child":child,"other":other},"files":{"records":records}}).to_string())
    }
    /// Fixed native-probe inputs through actual production File Manager ABIs.
    pub fn path_probe<B: AddressBus>(m: &mut B) -> Result<String, String> {
        use serde_json::json;
        let mut f = Self::new();
        let volume = f.volume_ref;
        let base = f
            .create_directory(2, b"RuntimePathProbe")
            .map_err(|e| e.to_string())?;
        let child = f
            .create_directory(base, b"Child")
            .map_err(|e| e.to_string())?;
        let other = f
            .create_directory(base, b"Other")
            .map_err(|e| e.to_string())?;
        for (dir, name, bytes) in [
            (base, &b"Map"[..], &b"BASE"[..]),
            (child, b"Map", b"CHLD"),
            (other, b"OnlyOther", b"OTHR"),
            (base, b"Slash/Name", b"SLSH"),
        ] {
            f.create_at(dir, name, [0; 16]).map_err(|e| e.to_string())?;
            let r = f
                .open_at(dir, name, Fork::Data, 3)
                .map_err(|e| e.to_string())?;
            f.write(r, bytes).map_err(|e| e.to_string())?;
            f.close(r).map_err(|e| e.to_string())?;
        }
        let pb = 0x4000;
        let np = 0x2000;
        let spec = 0x3000;
        let buffer = 0x5000;
        let mut cpu = m68k::CpuCore::new();
        cpu.dar[8] = pb;
        for i in 0..128 {
            m.write_byte(pb + i, 0);
        }
        m.write_word(pb + 22, volume as u16);
        m.write_long(pb + 48, base);
        f.dispatch(0xa215, &mut cpu, m)?;
        if cpu.dar[0] as i16 != 0 {
            return Err("Could not set probe default directory".into());
        }
        let inputs: Vec<(i16, u32, Vec<u8>)> = vec![
            (volume, base, b"Map".to_vec()),
            (0, 0, b"Map".to_vec()),
            (volume, base, b":Map".to_vec()),
            (volume, base, b":Child:Map".to_vec()),
            (volume, base, b"Child:Map".to_vec()),
            (volume, child, b"::Map".to_vec()),
            (volume, child, b"::Other:OnlyOther".to_vec()),
            (volume, base, b":cHiLd:mAp".to_vec()),
            (volume, base, b":Child:Missing".to_vec()),
            (volume, base, b":Missing:Map".to_vec()),
            (volume, base, b":Map:Missing".to_vec()),
            (volume, base, b"".to_vec()),
            (volume, base, b":".to_vec()),
            (volume, base, b":Child".to_vec()),
            (volume, base, b":Child:".to_vec()),
            (volume, child, b"::".to_vec()),
            (volume, base, b":Slash/Name".to_vec()),
            (volume, base, b":Map:".to_vec()),
            (volume, 0, b"Map".to_vec()),
            (volume, 999999, b"Map".to_vec()),
            (-123, 999999, b"Runtime:RuntimePathProbe:Child:Map".to_vec()),
            (
                volume,
                base,
                b"Runtime:RuntimePathProbe:Child:Missing".to_vec(),
            ),
            (volume, base, b"NoSuchVolume:RuntimePathProbe:Map".to_vec()),
            (volume, base, b":Other::Map".to_vec()),
            (volume, base, b":Child::Child:Map".to_vec()),
            (volume, 2, b"::Map".to_vec()),
            (volume, base, b":Map::Map".to_vec()),
            (volume, base, b"Missing".to_vec()),
        ];
        let mut rows = Vec::new();
        for (index, (v, dir, name)) in inputs.iter().enumerate() {
            m.write_byte(np, name.len() as u8);
            for (i, b) in name.iter().enumerate() {
                m.write_byte(np + 1 + i as u32, *b);
            }
            for i in 0..70 {
                m.write_byte(spec + i, 0x55);
            }
            let sp = 0x700000;
            cpu.dar[15] = sp;
            cpu.pc = 0x6000;
            cpu.dar[0] = 1;
            m.write_long(sp, spec);
            m.write_long(sp + 4, np);
            m.write_long(sp + 8, *dir);
            m.write_word(sp + 12, *v as u16);
            f.dispatch(0xaa52, &mut cpu, m)?;
            if cpu.dar[15] != sp + 14 || cpu.pc != 0x6002 {
                return Err("Path spec caller ABI changed".into());
            }
            let se = m.read_word(sp + 14) as i16;
            let sv = m.read_word(spec) as i16;
            let parent = m.read_long(spec + 2);
            let parent = if parent == 0 {
                0
            } else if parent == 2 {
                2
            } else if parent == base {
                3
            } else if parent == child {
                4
            } else if parent == other {
                5
            } else {
                -1
            };
            let n = m.read_byte(spec + 6).min(63) as u32;
            let spelling: String = (0..n)
                .map(|i| format!("{:02x}", m.read_byte(spec + 7 + i)))
                .collect();
            let mut observation = [12345i64, 4, 85, 85, 85, 85];
            // Convenience HOpen's local PB refnum was not initialized by the
            // native glue. The new explicit-sentinel PB observation below is
            // the independent assertion of its actual output behavior.
            for i in 0..128 {
                m.write_byte(pb + i, 0);
            }
            m.write_long(pb + 18, np);
            m.write_word(pb + 22, *v as u16);
            m.write_long(pb + 48, *dir);
            m.write_byte(pb + 27, 1);
            cpu.dar[8] = pb;
            f.dispatch(0xa200, &mut cpu, m)?;
            let oe = cpu.dar[0] as i16;
            let wrapper_ref = m.read_word(pb + 24);
            if oe == 0 {
                for i in 0..4 {
                    m.write_byte(buffer + i, 0x55);
                }
                m.write_long(pb + 32, buffer);
                m.write_long(pb + 36, 4);
                m.write_word(pb + 44, 0);
                m.write_long(pb + 46, 0);
                f.dispatch(0xa002, &mut cpu, m)?;
                observation[0] = cpu.dar[0] as i16 as i64;
                observation[1] = m.read_long(pb + 40) as i64;
                for i in 0..4 {
                    observation[2 + i] = m.read_byte(buffer + i as u32) as i64;
                }
                f.dispatch(0xa001, &mut cpu, m)?;
            }
            for i in 0..128 {
                m.write_byte(pb + i, 0x55);
            }
            m.write_long(pb + 18, np);
            m.write_word(pb + 22, *v as u16);
            m.write_long(pb + 48, *dir);
            m.write_byte(pb + 27, 1);
            m.write_long(pb + 28, 0);
            m.write_long(pb + 12, 0);
            m.write_byte(pb + 26, 0);
            m.write_word(pb + 24, 0x1234);
            f.dispatch(0xa200, &mut cpu, m)?;
            let pe = cpu.dar[0] as i16;
            let pref = m.read_word(pb + 24) as i16;
            let result = m.read_word(pb + 16) as i16;
            if pe == 0 {
                f.dispatch(0xa001, &mut cpu, m)?;
            }
            rows.push(json!({"case":index,"spec_error":se,"spec_volume_same":i16::from(sv==volume),"spec_volume_zero":i16::from(sv==0),"spec_parent":parent,"spec_name_hex":spelling,
                "open_error":oe,"failed_ref_preserved":i16::from(oe!=0 && wrapper_ref==0x1234),"read":observation,
                "pb_error":pe,"pb_io_result":result,"pb_ref":if pe==0 {i16::from(pref>0)} else {pref}}));
        }
        Ok(
            json!({"schema":1,"probe":"path-files-pb-semantics-v1","files":{"cases":rows}})
                .to_string(),
        )
    }
    fn file_key(parent: u32, name: &[u8]) -> Result<Vec<u8>, i16> {
        let leaf = key(name)?;
        if parent == 2 {
            return Ok(leaf);
        }
        let mut result = vec![0];
        result.extend_from_slice(&parent.to_be_bytes());
        result.extend(leaf);
        Ok(result)
    }
    fn path_volume(&self, volume: i16, path: &[u8]) -> Result<i16, i16> {
        if path.first() != Some(&b':') {
            if let Some(at) = path.iter().position(|b| *b == b':') {
                let name = &path[..at];
                if name.eq_ignore_ascii_case(&self.volume_name) {
                    return Ok(self.volume_ref);
                }
                if let Some(source) = &self.source {
                    if name.eq_ignore_ascii_case(&source.volume_name) {
                        return Ok(source.volume_ref);
                    }
                }
                return Err(-35);
            }
        }
        Ok(if volume == 0 {
            self.current_volume
        } else {
            volume
        })
    }
    fn resolve_path(
        &self,
        volume: i16,
        directory: u32,
        path: &[u8],
    ) -> Result<classic_core::catalog::Target, i16> {
        let mut path = path;
        let absolute = path.first() != Some(&b':') && path.contains(&b':');
        let start = if absolute {
            let at = path.iter().position(|b| *b == b':').unwrap();
            if !path[..at].eq_ignore_ascii_case(&self.volume_name) {
                return Err(-35);
            }
            path = &path[at + 1..];
            2
        } else {
            if ![0, self.volume_ref].contains(&volume) {
                return Err(-35);
            }
            if directory != 0 {
                directory
            } else if volume == 0 {
                self.current_dir
            } else {
                2
            }
        };
        self.catalog
            .resolve(start, path, &self.volume_name, |parent, name| {
                Self::file_key(parent, name)
                    .ok()
                    .and_then(|k| self.files.get(&k))
                    .map(|f| f.name.clone())
            })
    }
    pub fn create_directory(&mut self, parent: u32, name: &[u8]) -> Result<u32, i16> {
        if self.read_only {
            return Err(-44);
        }
        if self.files.contains_key(&Self::file_key(parent, name)?) {
            return Err(-48);
        }
        classic_core::catalog::Catalog::valid_name(name)?;
        if !self.catalog.contains(parent) {
            return Err(-120);
        }
        if self.catalog.child(parent, name).is_some() {
            return Err(-48);
        }
        self.check_growth(0, 0, 1)?;
        let id = self.catalog.create(parent, name.to_vec())?;
        if let Err(error) = self.flush() {
            let _ = self.catalog.remove_empty(id);
            return Err(error);
        }
        Ok(id)
    }
    /// Virtual HFS+ allocation profile: allocation granule4096, clump65536.
    /// Logical fields come from the actual open fork and catalog, never host paths.
    fn catalog_info<B: AddressBus>(&self, m: &mut B, p: u32) -> Result<(), i16> {
        let volume = m.read_word(p + 22) as i16;
        let directory = m.read_long(p + 48);
        let index = m.read_word(p + 28) as i16;
        let np = m.read_long(p + 18);
        let name = if np == 0 {
            Vec::new()
        } else {
            Self::name(m, np)
        };
        let target_volume = if index == 0 {
            self.path_volume(volume, &name)?
        } else {
            if volume == 0 {
                self.current_volume
            } else {
                volume
            }
        };
        if let Some(source) = self
            .source
            .as_ref()
            .filter(|s| s.volume_ref == target_volume)
        {
            return source.catalog_info(m, p);
        }
        if target_volume != self.volume_ref {
            return Err(-35);
        }
        let start = if directory != 0 {
            directory
        } else if volume == 0 {
            self.current_dir
        } else {
            2
        };
        let target = if index > 0 {
            if !self.catalog.contains(start) {
                return Err(-43);
            }
            let mut names: Vec<Vec<u8>> = self
                .files
                .values()
                .filter(|f| f.parent == start)
                .map(|f| f.name.clone())
                .chain(
                    self.catalog
                        .entries()
                        .filter(|(_, d)| d.parent == start)
                        .map(|(_, d)| d.name.clone()),
                )
                .collect();
            names.sort_by_cached_key(|n| key(n).unwrap_or_default());
            let leaf = names.get(index as usize - 1).ok_or(-43i16)?;
            self.resolve_path(self.volume_ref, start, leaf)?
        } else if index < 0 {
            if !self.catalog.contains(start) {
                return Err(-43);
            }
            self.resolve_path(self.volume_ref, start, b"")?
        } else {
            match self.resolve_path(volume, directory, &name) {
                Err(-120) if !name.contains(&b':') && !self.catalog.contains(start) => {
                    return Err(-43)
                }
                other => other?,
            }
        };
        if !target.exists {
            return Err(-43);
        }
        if index != 0 && np != 0 {
            m.write_byte(np, target.name.len() as u8);
            for (i, b) in target.name.iter().enumerate() {
                m.write_byte(np + 1 + i as u32, *b);
            }
        }
        if let Some(id) = target.directory {
            let meta = self.catalog.metadata(id).ok_or(-43i16)?;
            m.write_byte(p + 30, 0x10);
            for (i, b) in meta.finder.iter().enumerate() {
                m.write_byte(p + 32 + i as u32, *b);
            }
            m.write_long(p + 48, id);
            let count = self.files.values().filter(|f| f.parent == id).count()
                + self
                    .catalog
                    .entries()
                    .filter(|(_, d)| d.parent == id)
                    .count();
            m.write_word(p + 52, u16::try_from(count).map_err(|_| -36i16)?);
            for (i, t) in meta.dates.iter().enumerate() {
                m.write_long(p + 72 + i as u32 * 4, *t);
            }
            for (i, b) in meta.extended.iter().enumerate() {
                m.write_byte(p + 84 + i as u32, *b);
            }
            m.write_long(
                p + 100,
                if index == 0 && name.is_empty() {
                    start
                } else {
                    target.parent
                },
            );
        } else {
            let k = Self::file_key(target.parent, &target.name)?;
            let file = self.files.get(&k).ok_or(-43i16)?;
            let mut attributes = 2;
            let mut first = 0;
            for (r, open) in &self.open {
                if open.key == k {
                    if first == 0 {
                        first = *r;
                    }
                    attributes |= 0x80 | if open.fork == Fork::Data { 8 } else { 4 };
                }
            }
            m.write_word(p + 24, first as u16);
            m.write_byte(p + 30, attributes);
            for (i, b) in file.finder.iter().enumerate() {
                m.write_byte(p + 32 + i as u32, *b);
            }
            m.write_long(p + 48, file.id);
            m.write_word(p + 52, 0);
            m.write_long(p + 54, file.data.len() as u32);
            m.write_long(p + 58, (file.data.len() as u32 + 4095) / 4096 * 4096);
            m.write_word(p + 62, 0);
            m.write_long(p + 64, file.resource.len() as u32);
            m.write_long(p + 68, (file.resource.len() as u32 + 4095) / 4096 * 4096);
            for (i, t) in file.dates.iter().enumerate() {
                m.write_long(p + 72 + i as u32 * 4, *t);
            }
            for (i, b) in file.extended.iter().enumerate() {
                m.write_byte(p + 84 + i as u32, *b);
            }
            m.write_long(p + 100, file.parent);
            m.write_long(p + 104, 0);
        }
        Ok(())
    }
    fn fcb_info<B: AddressBus>(&self, m: &mut B, p: u32) -> Result<(), i16> {
        let index = m.read_word(p + 28) as i16;
        let volume = m.read_word(p + 22) as i16;
        let reference = m.read_word(p + 24) as i16;
        if index == 0 {
            if self.open.contains_key(&reference) {
                return self.fcb_record(m, p, reference);
            }
            if let Some(source) = &self.source {
                if source.open.contains_key(&reference) {
                    return source.fcb_record(m, p, reference);
                }
            }
            let issued = self.issued.contains(&reference)
                || self
                    .source
                    .as_ref()
                    .is_some_and(|s| s.issued.contains(&reference));
            return Err(if issued { -38 } else { -51 });
        }
        if volume != 0
            && volume != self.volume_ref
            && !self.source.as_ref().is_some_and(|s| s.volume_ref == volume)
        {
            return Err(-35);
        }
        if index < 0 {
            return Err(-38);
        }
        let mut entries: Vec<(i16, &Self)> = Vec::new();
        if volume == 0 || volume == self.volume_ref {
            entries.extend(self.open.keys().map(|r| (*r, self)));
        }
        if let Some(source) = &self.source {
            if volume == 0 || volume == source.volume_ref {
                entries.extend(source.open.keys().map(|r| (*r, source.as_ref())));
            }
        }
        entries.sort_by_key(|(r, _)| *r);
        let (r, owner) = entries.get(index as usize - 1).ok_or(-38i16)?;
        owner.fcb_record(m, p, *r)?;
        m.write_word(p + 24, *r as u16);
        Ok(())
    }
    fn fcb_record<B: AddressBus>(&self, m: &mut B, p: u32, reference: i16) -> Result<(), i16> {
        let open = self.open.get(&reference).ok_or(-38i16)?;
        let file = self.files.get(&open.key).ok_or(-43i16)?;
        let name = m.read_long(p + 18);
        if name != 0 {
            m.write_byte(name, file.name.len() as u8);
            for (i, b) in file.name.iter().enumerate() {
                m.write_byte(name + 1 + i as u32, *b);
            }
        }
        m.write_long(p + 32, file.id);
        m.write_word(
            p + 36,
            (if open.permission == 1 { 0 } else { 0x100 })
                | if open.dirty { 0x8000 } else { 0 }
                | if open.fork == Fork::Resource {
                    0x200
                } else {
                    0
                },
        );
        m.write_word(p + 38, 0);
        let len = file.fork(open.fork).len() as u32;
        m.write_long(p + 40, len);
        m.write_long(p + 44, (len + 4095) / 4096 * 4096);
        m.write_long(p + 48, open.mark as u32);
        m.write_word(p + 52, self.volume_ref as u16);
        m.write_long(p + 54, 65536);
        m.write_long(p + 58, file.parent);
        Ok(())
    }
    fn create_directory_pb<B: AddressBus>(&mut self, m: &mut B, p: u32) -> Result<(), i16> {
        let np = m.read_long(p + 18);
        let name = Self::name(m, np);
        let volume = m.read_word(p + 22) as i16;
        let dir = m.read_long(p + 48);
        let target = self.path_volume(volume, &name)?;
        if let Some(source) = self.source.as_mut().filter(|s| s.volume_ref == target) {
            return source.create_directory_pb(m, p);
        }
        let t = self.resolve_path(volume, dir, &name)?;
        if t.exists {
            return Err(-48);
        }
        let id = self.create_directory(t.parent, &t.name)?;
        m.write_long(p + 48, id);
        Ok(())
    }
    /// Resource Manager storage adapter. No host paths escape this layer.
    pub fn resource_volume(&mut self, volume: i16) -> Result<&mut Self, i16> {
        let target = if volume == 0 {
            self.current_volume
        } else {
            volume
        };
        if target == self.volume_ref {
            return Ok(self);
        }
        self.source
            .as_mut()
            .filter(|s| s.volume_ref == target)
            .map(|s| s.as_mut())
            .ok_or(-35)
    }
    pub fn resource_volume_id(&self) -> i16 {
        self.volume_ref
    }
    pub fn fork_bytes(&self, name: &[u8], fork: Fork) -> Result<&[u8], i16> {
        Ok(self.files.get(&key(name)?).ok_or(-43i16)?.fork(fork))
    }
    pub fn list(&self, kinds: &[[u8; 4]]) -> Vec<Vec<u8>> {
        self.files
            .values()
            .filter(|f| kinds.is_empty() || kinds.iter().any(|k| f.finder[..4] == k[..]))
            .map(|f| f.name.clone())
            .collect()
    }
    pub fn new() -> Self {
        Self {
            catalog: Default::default(),
            current_dir: 2,
            files: BTreeMap::new(),
            open: BTreeMap::new(),
            issued: BTreeSet::new(),
            next_ref: 128,
            root: None,
            volume_lock: None,
            checkpointed: BTreeSet::new(),
            source: None,
            read_only: false,
            volume_ref: -1,
            volume_name: b"Runtime".to_vec(),
            current_volume: -1,
            reference_limit: i16::MAX,
            allocation_blocks: classic_core::volume::DEFAULT_BLOCKS,
            volume_created: Self::now(),
            volume_modified: Cell::new(Self::now()),
            volume_backup: 0,
            volume_writes: Cell::new(0),
            committed_state: Cell::new(None),
        }
    }
    pub fn mount_application(
        &mut self,
        name: &[u8],
        finder: [u8; 16],
        data: &[u8],
        resource: &[u8],
    ) -> Result<(), String> {
        if self.source.is_some() || data.len() > MAX_FORK || resource.len() > MAX_FORK {
            return Err("application source already mounted or exceeds fork limit".into());
        }
        if self.next_ref >= 16000 {
            return Err(
                "application source must be mounted before document reference range is exhausted"
                    .into(),
            );
        }
        self.mount_source(Self::new(), name, finder, data, resource)
    }
    pub fn mount_application_files(
        &mut self,
        root: &Path,
        name: &[u8],
        finder: [u8; 16],
        data: &[u8],
        resource: &[u8],
    ) -> Result<(), String> {
        if self.source.is_some() || data.len() > MAX_FORK || resource.len() > MAX_FORK {
            return Err("invalid or duplicate application source".into());
        }
        let mut source = Self::new();
        source.read_catalog(root)?;
        self.mount_source(source, name, finder, data, resource)?;
        // Explicit application bundle startup directory; ordinary document-only
        // sessions retain their established Runtime default volume.
        self.current_volume = APPLICATION_VOLUME;
        Ok(())
    }
    fn mount_source(
        &mut self,
        mut source: Self,
        name: &[u8],
        finder: [u8; 16],
        data: &[u8],
        resource: &[u8],
    ) -> Result<(), String> {
        if self.next_ref >= 16000 {
            return Err("application reference range exhausted".into());
        }
        match source.files.get(&key(name).map_err(|e| e.to_string())?) {
            Some(file) if file.data != data || file.resource != resource => {
                return Err("application bundle forks differ from executable input".into())
            }
            Some(_) => {}
            None => source.create(name, finder).map_err(|e| e.to_string())?,
        }
        let file = source
            .files
            .get_mut(&key(name).map_err(|e| e.to_string())?)
            .unwrap();
        file.data = data.to_vec();
        file.resource = resource.to_vec();
        source.issued.insert(APPLICATION_RESOURCE_FILE);
        source.open.insert(
            APPLICATION_RESOURCE_FILE,
            Open {
                dirty: false,
                key: key(name).map_err(|e| e.to_string())?,
                fork: Fork::Resource,
                mark: 0,
                permission: 1,
            },
        );
        source.read_only = true;
        source.volume_ref = APPLICATION_VOLUME;
        source.current_volume = APPLICATION_VOLUME;
        source.volume_name = b"Application".to_vec();
        source.next_ref = 16000;
        self.reference_limit = 16000;
        self.source = Some(Box::new(source));
        Ok(())
    }
    fn read_metadata(
        value: Option<&serde_json::Value>,
    ) -> Result<classic_core::catalog::Metadata, String> {
        let mut result = classic_core::catalog::Metadata::default();
        let Some(value) = value else {
            return Ok(result);
        };
        if !value.is_object() {
            return Err("invalid catalog metadata".into());
        }
        for (field, destination) in [
            ("finder", &mut result.finder),
            ("extended", &mut result.extended),
        ] {
            if let Some(v) = value.get(field) {
                let a = v
                    .as_array()
                    .filter(|a| a.len() == 16)
                    .ok_or("invalid metadata bytes")?;
                for (out, n) in destination.iter_mut().zip(a) {
                    *out = n
                        .as_u64()
                        .and_then(|v| u8::try_from(v).ok())
                        .ok_or("invalid metadata byte")?;
                }
            }
        }
        if let Some(v) = value.get("dates") {
            let a = v
                .as_array()
                .filter(|a| a.len() == 3)
                .ok_or("invalid catalog dates")?;
            for (out, n) in result.dates.iter_mut().zip(a) {
                *out = n
                    .as_u64()
                    .and_then(|v| u32::try_from(v).ok())
                    .ok_or("invalid catalog date")?;
            }
        }
        Ok(result)
    }
    fn metadata_value(
        finder: Option<&[u8; 16]>,
        extended: &[u8; 16],
        dates: &[u32; 3],
    ) -> Option<serde_json::Value> {
        if finder.is_none_or(|f| *f == [0; 16]) && *extended == [0; 16] && *dates == [0; 3] {
            return None;
        }
        let mut value = serde_json::json!({"extended":extended,"dates":dates});
        if let Some(finder) = finder {
            value["finder"] = serde_json::json!(finder)
        }
        Some(value)
    }
    fn read_catalog(&mut self, root: &Path) -> Result<(), String> {
        let catalog = root.join("catalog.json");
        let doc: serde_json::Value =
            serde_json::from_slice(&fs::read(&catalog).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        if doc["format"] != "ClassicRuntimeVolume" || doc["version"] != 1 {
            return Err("unrecognized logical volume".into());
        }
        if let Some(v) = doc.get("volume") {
            let value = |key: &str| {
                v[key]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or_else(|| format!("invalid volume {key}"))
            };
            self.allocation_blocks = value("allocation_blocks")?;
            if self.allocation_blocks < classic_core::volume::RESERVED_BLOCKS {
                return Err("volume capacity below metadata reserve".into());
            }
            self.catalog
                .advance_next_id(value("next_catalog_id")?)
                .map_err(|e| e.to_string())?;
            self.volume_created = value("created")?;
            self.volume_modified.set(value("modified")?);
            self.volume_backup = value("backup")?;
            self.volume_writes.set(value("writes")?);
        }
        if doc.get("directories").is_some() && !doc["directories"].is_array() {
            return Err("invalid directory catalog".into());
        }
        if let Some(dirs) = doc["directories"].as_array() {
            for d in dirs {
                let id = d["id"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or("invalid directory ID")?;
                let parent = d["parent"]
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or("invalid parent ID")?;
                let name: Vec<u8> = d["name"]
                    .as_array()
                    .ok_or("invalid directory name")?
                    .iter()
                    .map(|n| {
                        n.as_u64()
                            .and_then(|v| u8::try_from(v).ok())
                            .ok_or("invalid directory byte")
                    })
                    .collect::<Result<_, _>>()?;
                self.catalog
                    .insert(id, parent, name)
                    .map_err(|e| format!("invalid directory catalog: {e}"))?;
                *self.catalog.metadata_mut(id).unwrap() = Self::read_metadata(d.get("metadata"))?;
            }
        }
        for entry in doc["files"].as_array().ok_or("invalid volume catalog")? {
            let bytes = |field: &str| -> Result<Vec<u8>, String> {
                entry[field]
                    .as_array()
                    .ok_or("invalid catalog bytes")?
                    .iter()
                    .map(|v| {
                        v.as_u64()
                            .filter(|n| *n < 256)
                            .map(|n| n as u8)
                            .ok_or("invalid catalog byte".into())
                    })
                    .collect()
            };
            let name = bytes("name")?;
            let parent = match entry.get("parent") {
                None => 2,
                Some(value) => value
                    .as_u64()
                    .and_then(|n| u32::try_from(n).ok())
                    .ok_or("invalid file parent")?,
            };
            if !self.catalog.contains(parent) {
                return Err("catalog parent missing".into());
            }
            if self.catalog.child(parent, &name).is_some() {
                return Err("file conflicts with directory".into());
            }
            let k =
                Self::file_key(parent, &name).map_err(|e| format!("invalid catalog name: {e}"))?;
            let finder: [u8; 16] = bytes("finder")?
                .try_into()
                .map_err(|_| "invalid Finder info")?;
            let directory = root.join("files").join(encoded(&k));
            let data = fs::read(directory.join("data")).map_err(|e| e.to_string())?;
            let resource = fs::read(directory.join("resource")).map_err(|e| e.to_string())?;
            if data.len() > MAX_FORK || resource.len() > MAX_FORK {
                return Err("fork exceeds logical volume limit".into());
            }
            let requested = entry
                .get("id")
                .map(|v| {
                    v.as_u64()
                        .and_then(|n| u32::try_from(n).ok())
                        .ok_or("invalid file identity")
                })
                .transpose()?;
            let id = self
                .catalog
                .file_identity(requested)
                .map_err(|e| format!("invalid file identity: {e}"))?;
            let metadata = Self::read_metadata(entry.get("metadata"))?;
            if self
                .files
                .insert(
                    k,
                    File {
                        id,
                        parent,
                        name,
                        finder,
                        extended: metadata.extended,
                        dates: metadata.dates,
                        data,
                        resource,
                    },
                )
                .is_some()
            {
                return Err("duplicate catalog name".into());
            }
        }
        if doc.get("volume").is_some() {
            self.committed_state.set(Some(self.state_hash()));
        }
        Ok(())
    }
    pub fn volume(root: &Path) -> Result<Self, String> {
        fs::create_dir_all(root).map_err(|e| e.to_string())?;
        let catalog = root.join("catalog.json");
        let mut volume = Self::new();
        if !catalog.exists()
            && fs::read_dir(root)
                .map_err(|e| e.to_string())?
                .next()
                .is_some()
        {
            return Err("volume directory is not empty and has no recognized catalog".into());
        }
        let lock = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(root.join(".runtime.lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("logical volume is already in use or cannot be locked: {e}"))?;
        volume.volume_lock = Some(lock);
        if catalog.exists() {
            volume.read_catalog(root)?;
        }
        if !fs::read(&catalog)
            .ok()
            .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
            .is_some_and(|v| v.get("volume").is_some())
        {
            if let Ok(t) = fs::metadata(root).and_then(|m| m.created()).and_then(|t| {
                t.duration_since(std::time::UNIX_EPOCH)
                    .map_err(std::io::Error::other)
            }) {
                volume.volume_created =
                    t.as_secs().saturating_add(2082844800).min(u32::MAX as u64) as u32;
            }
            volume.allocation_blocks = volume.allocation_blocks.max(
                u32::try_from(volume.used_blocks().saturating_mul(2))
                    .map_err(|_| "legacy volume exceeds supported capacity")?,
            );
        }
        if volume.used_blocks() > u64::from(volume.allocation_blocks) {
            return Err("volume allocation exceeds declared capacity".into());
        }
        volume.root = Some(root.to_path_buf());
        volume
            .flush()
            .map_err(|e| format!("volume initialization: {e}"))?;
        Ok(volume)
    }
    // Preserve the old version before a destructive operation. These host-side
    // checkpoints are outside the guest catalog and do not change classic API
    // results. A failed checkpoint refuses the mutation rather than losing data.
    fn checkpoint(&mut self, k: &[u8]) -> Result<(), i16> {
        let Some(root) = &self.root else {
            return Ok(());
        };
        if self.checkpointed.contains(k) {
            return Ok(());
        }
        let file = self.files.get(k).ok_or(-43i16)?;
        let base = root.join("recovery");
        fs::create_dir_all(&base).map_err(|_| -36i16)?;
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_| -36i16)?
            .as_nanos();
        let mut counter = 0u32;
        let directory = loop {
            let candidate = base.join(format!("{stamp}-{}-{counter}", encoded(k)));
            match fs::create_dir(&candidate) {
                Ok(()) => break candidate,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    counter = counter.checked_add(1).ok_or(-36i16)?
                }
                Err(_) => return Err(-36),
            }
        };
        fn durable(path: &Path, bytes: &[u8]) -> Result<(), i16> {
            let mut f = fs::File::create_new(path).map_err(|_| -36i16)?;
            f.write_all(bytes).map_err(|_| -36i16)?;
            f.sync_all().map_err(|_| -36i16)
        }
        durable(&directory.join("data"), &file.data)?;
        durable(&directory.join("resource"), &file.resource)?;
        let mut record = serde_json::json!({"format":"ClassicRuntimeRecovery","version":1,"id":file.id,"name":file.name,"finder":file.finder,"data_bytes":file.data.len(),"resource_bytes":file.resource.len()});
        if let Some(meta) = Self::metadata_value(None, &file.extended, &file.dates) {
            record["metadata"] = meta;
        }
        let manifest = record.to_string();
        durable(&directory.join("manifest.json"), manifest.as_bytes())?;
        fs::File::open(&directory)
            .and_then(|f| f.sync_all())
            .map_err(|_| -36i16)?;
        fs::File::open(&base)
            .and_then(|f| f.sync_all())
            .map_err(|_| -36i16)?;
        fs::File::open(root)
            .and_then(|f| f.sync_all())
            .map_err(|_| -36i16)?;
        self.checkpointed.insert(k.to_vec());
        Ok(())
    }
    pub fn flush(&self) -> Result<(), i16> {
        if self.read_only {
            return Ok(());
        }
        let state = self.state_hash();
        let changed = self.committed_state.get() != Some(state);
        let modified = self.volume_modified.get();
        let writes = self.volume_writes.get().wrapping_add(u32::from(changed));
        let Some(root) = &self.root else {
            if !self.read_only {
                self.volume_modified.set(modified);
                self.volume_writes.set(writes);
                self.committed_state.set(Some(state));
            }
            return Ok(());
        };
        let mut entries = Vec::new();
        for (k, file) in &self.files {
            let dir = root.join("files").join(encoded(k));
            fs::create_dir_all(&dir).map_err(|_| -36i16)?;
            for (name, bytes) in [("data", &file.data), ("resource", &file.resource)] {
                let temporary = dir.join(format!("{name}.tmp"));
                fs::write(&temporary, bytes).map_err(|_| -36i16)?;
                fs::rename(temporary, dir.join(name)).map_err(|_| -36i16)?;
            }
            let mut entry = serde_json::json!({"id":file.id,"name":file.name,"finder":file.finder});
            if let Some(meta) = Self::metadata_value(None, &file.extended, &file.dates) {
                entry["metadata"] = meta;
            }
            if file.parent != 2 {
                entry["parent"] = file.parent.into();
            }
            entries.push(entry);
        }
        let mut doc =
            serde_json::json!({"format":"ClassicRuntimeVolume","version":1,"files":entries});
        let dirs: Vec<_> = self
            .catalog
            .entries()
            .map(|(id, d)| {
                let mut entry = serde_json::json!({"id":id,"parent":d.parent,"name":d.name});
                if let Some(meta) = Self::metadata_value(
                    Some(&d.metadata.finder),
                    &d.metadata.extended,
                    &d.metadata.dates,
                ) {
                    entry["metadata"] = meta;
                }
                entry
            })
            .collect();
        if !dirs.is_empty() {
            doc["directories"] = dirs.into();
        }
        doc["volume"] = serde_json::json!({"allocation_blocks":self.allocation_blocks,"created":self.volume_created,"modified":modified,"backup":self.volume_backup,"writes":writes,"next_catalog_id":self.catalog.next_id()});
        let bytes = doc.to_string();
        let temporary = root.join("catalog.tmp");
        fs::write(&temporary, bytes).map_err(|_| -36i16)?;
        fs::rename(temporary, root.join("catalog.json")).map_err(|_| -36i16)?;
        self.volume_modified.set(modified);
        self.volume_writes.set(writes);
        self.committed_state.set(Some(state));
        Ok(())
    }
    pub fn create(&mut self, name: &[u8], finder: [u8; 16]) -> Result<(), i16> {
        self.create_at(2, name, finder)
    }
    pub fn create_at(&mut self, parent: u32, name: &[u8], finder: [u8; 16]) -> Result<(), i16> {
        if self.read_only {
            return Err(-44);
        }
        if !self.catalog.contains(parent) {
            return Err(-120);
        }
        let k = Self::file_key(parent, name)?;
        if self.catalog.child(parent, name).is_some() || self.files.contains_key(&k) {
            return Err(-48);
        }
        self.check_growth(0, 0, 1)?;
        let id = self.catalog.file_identity(None)?;
        self.checkpointed.insert(k.clone());
        self.files.insert(
            k,
            File {
                id,
                parent,
                name: name.to_vec(),
                finder,
                extended: [0; 16],
                dates: [0; 3],
                data: Vec::new(),
                resource: Vec::new(),
            },
        );
        self.flush()
    }
    pub fn open(&mut self, name: &[u8], fork: Fork, permission: u8) -> Result<i16, i16> {
        self.open_at(2, name, fork, permission)
    }
    fn open_at(
        &mut self,
        parent: u32,
        name: &[u8],
        fork: Fork,
        permission: u8,
    ) -> Result<i16, i16> {
        let k = Self::file_key(parent, name)?;
        if !self.files.contains_key(&k) {
            return Err(-43);
        }
        if permission > 4 {
            return Err(-50);
        }
        let permission = if self.read_only {
            if permission <= 1 {
                1
            } else {
                return Err(-44);
            }
        } else {
            permission
        };
        if permission != 1
            && permission != 4
            && self
                .open
                .values()
                .any(|o| o.key == k && o.fork == fork && o.permission != 1)
        {
            return Err(-49);
        }
        let r = self.next_ref;
        if r >= self.reference_limit {
            return Err(-42);
        }
        self.next_ref += 1;
        self.issued.insert(r);
        self.open.insert(
            r,
            Open {
                dirty: false,
                key: k,
                fork,
                mark: 0,
                permission,
            },
        );
        Ok(r)
    }
    pub fn close(&mut self, r: i16) -> Result<(), i16> {
        let opened = self.open.remove(&r).ok_or(-51i16)?;
        self.flush()?;
        if !self.open.values().any(|other| other.key == opened.key) {
            self.checkpointed.remove(&opened.key);
        }
        Ok(())
    }
    /// Commit file data without changing its open reference or position.
    pub fn flush_file(&mut self, r: i16) -> Result<bool, i16> {
        if let Some(source) = self.source.as_mut() {
            if source.open.contains_key(&r) || source.issued.contains(&r) {
                return source.flush_file(r);
            }
        }
        let dirty = self
            .open
            .get(&r)
            .ok_or_else(|| self.reference_error(r))?
            .dirty;
        // The application source volume is immutable. Read-only references
        // still flush successfully, without rewriting its pinned artifacts.
        if !self.read_only {
            self.flush()?;
        }
        self.open.get_mut(&r).unwrap().dirty = false;
        Ok(dirty)
    }
    fn reference_error(&self, r: i16) -> i16 {
        if self.issued.contains(&r) {
            -38
        } else {
            -51
        }
    }
    pub fn eof(&self, r: i16) -> Result<usize, i16> {
        let o = self.open.get(&r).ok_or_else(|| self.reference_error(r))?;
        Ok(self.files[&o.key].fork(o.fork).len())
    }
    pub fn position(&self, r: i16) -> Result<usize, i16> {
        Ok(self
            .open
            .get(&r)
            .ok_or_else(|| self.reference_error(r))?
            .mark)
    }
    pub fn seek(&mut self, r: i16, mode: u16, offset: i32) -> Result<(), i16> {
        let len = self.eof(r)?;
        let o = self.open.get_mut(&r).ok_or(-51i16)?;
        let base = match mode {
            0 => return Ok(()),
            1 => 0,
            2 => len,
            3 => o.mark,
            _ => return Err(-50),
        };
        let at = base as i64 + offset as i64;
        if at < 0 {
            return Err(-40);
        }
        if at > i32::MAX as i64 {
            return Err(-50);
        }
        if at as usize > len {
            o.mark = len;
            return Err(-39);
        }
        o.mark = at as usize;
        Ok(())
    }
    pub fn set_eof(&mut self, r: i16, len: usize) -> Result<(), i16> {
        if len > MAX_FORK {
            return Err(-34);
        }
        let o = self.open.get(&r).ok_or(-51i16)?;
        if o.permission == 1 {
            return Err(-61);
        }
        let k = o.key.clone();
        let fork = o.fork;
        self.check_growth(self.files[&k].fork(fork).len(), len, 0)?;
        if self.files[&k].fork(fork).len() != len {
            self.checkpoint(&k)?;
            self.open.get_mut(&r).unwrap().dirty = true;
        }
        self.files
            .get_mut(&k)
            .unwrap()
            .fork_mut(fork)
            .resize(len, 0);
        self.open.get_mut(&r).unwrap().mark = self.open[&r].mark.min(len);
        self.flush()
    }
    pub fn read(&mut self, r: i16, count: usize) -> Result<(Vec<u8>, i16), i16> {
        let o = self.open.get_mut(&r).ok_or(-51i16)?;
        if o.permission == 2 {
            return Err(-54);
        }
        let bytes = self.files[&o.key].fork(o.fork);
        let available = bytes.len().saturating_sub(o.mark);
        let n = count.min(available);
        let data = if n == 0 {
            Vec::new()
        } else {
            bytes[o.mark..o.mark + n].to_vec()
        };
        o.mark += n;
        Ok((data, if n < count { -39 } else { 0 }))
    }
    pub fn write(&mut self, r: i16, bytes: &[u8]) -> Result<usize, i16> {
        let o = self.open.get(&r).ok_or(-51i16)?;
        if o.permission == 1 {
            return Err(-61);
        }
        let end = o.mark.checked_add(bytes.len()).ok_or(-34i16)?;
        if end > MAX_FORK {
            return Err(-34);
        }
        let k = o.key.clone();
        let fork = o.fork;
        let mark = o.mark;
        self.check_growth(
            self.files[&k].fork(fork).len(),
            end.max(self.files[&k].fork(fork).len()),
            0,
        )?;
        if !bytes.is_empty() {
            self.checkpoint(&k)?;
        }
        let data = self.files.get_mut(&k).unwrap().fork_mut(fork);
        if end > data.len() {
            data.resize(end, 0);
        }
        data[mark..end].copy_from_slice(bytes);
        if !bytes.is_empty() {
            self.open.get_mut(&r).unwrap().dirty = true;
        }
        self.open.get_mut(&r).unwrap().mark = end;
        self.flush()?;
        Ok(bytes.len())
    }
    pub fn finder(&self, name: &[u8]) -> Result<[u8; 16], i16> {
        self.finder_at(2, name)
    }
    fn finder_at(&self, parent: u32, name: &[u8]) -> Result<[u8; 16], i16> {
        Ok(self
            .files
            .get(&Self::file_key(parent, name)?)
            .ok_or(-43i16)?
            .finder)
    }
    pub fn set_finder(&mut self, name: &[u8], info: [u8; 16]) -> Result<(), i16> {
        self.set_finder_at(2, name, info)
    }
    fn set_finder_at(&mut self, parent: u32, name: &[u8], info: [u8; 16]) -> Result<(), i16> {
        if self.read_only {
            return Err(-44);
        }
        let k = Self::file_key(parent, name)?;
        let current = self.files.get(&k).ok_or(-43i16)?.finder;
        if current != info {
            self.checkpoint(&k)?;
        }
        self.files.get_mut(&k).unwrap().finder = info;
        self.flush()
    }
    pub fn delete(&mut self, name: &[u8]) -> Result<(), i16> {
        self.delete_at(2, name)
    }
    fn delete_at(&mut self, parent: u32, name: &[u8]) -> Result<(), i16> {
        if self.read_only {
            return Err(-44);
        }
        let k = Self::file_key(parent, name)?;
        if self.open.values().any(|o| o.key == k) {
            return Err(-47);
        }
        self.checkpoint(&k)?;
        let file = self.files.remove(&k).ok_or(-43i16)?;
        if let Err(error) = self.flush() {
            self.files.insert(k, file);
            return Err(error);
        }
        if let Some(root) = &self.root {
            fs::remove_dir_all(root.join("files").join(encoded(&k))).map_err(|_| -36i16)?;
        }
        Ok(())
    }
    fn spec<B: AddressBus>(&self, m: &mut B, p: u32) -> Result<classic_core::catalog::Target, i16> {
        self.resolve_path(
            m.read_word(p) as i16,
            m.read_long(p + 2),
            &Self::name(m, p + 6),
        )
    }
    /// Create the resource map without truncating either existing fork. A
    /// nonempty resource fork (even corrupt) is a duplicate, as on OS9.
    fn create_resource_file<B: AddressBus>(
        &mut self,
        m: &mut B,
        spec: u32,
        creator: u32,
        kind: u32,
        script: i16,
    ) -> Result<i16, String> {
        if ![-1, 0].contains(&script) {
            return Err("FSpCreateResFile non-Roman script tags are not implemented".into());
        }
        let volume = m.read_word(spec) as i16;
        let target = if volume == 0 {
            self.current_volume
        } else {
            volume
        };
        if let Some(source) = self.source.as_mut().filter(|s| s.volume_ref == target) {
            return source.create_resource_file(m, spec, creator, kind, script);
        }
        let result = self.spec(m, spec).and_then(|target| {
            if target.parent != 2 {
                return Err(-120);
            }
            self.create_resource_map(&target.name, creator, kind)
        });
        Ok(result.err().unwrap_or(0))
    }
    fn create_resource_map(&mut self, name: &[u8], creator: u32, kind: u32) -> Result<(), i16> {
        if self.read_only {
            return Err(-44);
        }
        let mut finder = match self.finder(&name) {
            Ok(info) => info,
            Err(-43) => [0; 16],
            Err(error) => return Err(error),
        };
        match self.fork_bytes(&name, Fork::Resource) {
            Ok(bytes) if !bytes.is_empty() => return Err(-48),
            Ok(_) | Err(-43) => {}
            Err(error) => return Err(error),
        }
        finder[..4].copy_from_slice(&kind.to_be_bytes());
        finder[4..8].copy_from_slice(&creator.to_be_bytes());
        match self.create(&name, finder) {
            Ok(()) | Err(-48) => {}
            Err(error) => return Err(error),
        }
        let reference = self.open(&name, Fork::Resource, 3)?;
        let result = (|| {
            self.write(reference, &crate::resource::empty_fork())?;
            self.set_finder(name, finder)
        })();
        let close = self.close(reference);
        result?;
        close
    }
    fn high_level<B: AddressBus>(
        &mut self,
        selector: u16,
        m: &mut B,
        sp: u32,
    ) -> Result<(u32, i16), String> {
        let volume = match selector {
            1 => Some(m.read_word(sp + 12) as i16),
            2 | 3 => {
                let p = m.read_long(sp + 6);
                Some(m.read_word(p) as i16)
            }
            4 => {
                let p = m.read_long(sp + 10);
                Some(m.read_word(p) as i16)
            }
            6 => {
                let p = m.read_long(sp);
                Some(m.read_word(p) as i16)
            }
            7 | 8 => {
                let p = m.read_long(sp + 4);
                Some(m.read_word(p) as i16)
            }
            _ => None,
        };
        if let Some(volume) = volume {
            let path = if selector == 1 {
                {
                    let p = m.read_long(sp + 4);
                    Self::name(m, p)
                }
            } else {
                Vec::new()
            };
            let target = self.path_volume(volume, &path).unwrap_or(volume);
            if let Some(source) = self.source.as_mut() {
                if target == source.volume_ref {
                    return source.high_level(selector, m, sp);
                }
            }
        }
        let (pop, error) = match selector {
            1 => {
                let output = m.read_long(sp);
                let name = m.read_long(sp + 4);
                let dir = m.read_long(sp + 8);
                let volume = m.read_word(sp + 12) as i16;
                let name = Self::name(m, name);
                // Native clears the complete FSSpec on structural errors, but
                // preserves a usable missing-final-leaf spec with fnfErr.
                for i in 0..70 {
                    m.write_byte(output + i, 0);
                }
                let result = (|| -> Result<(), i16> {
                    let t = self.resolve_path(volume, dir, &name)?;
                    m.write_word(output, self.volume_ref as u16);
                    m.write_long(output + 2, t.parent);
                    m.write_byte(output + 6, t.name.len() as u8);
                    for (i, b) in t.name.iter().enumerate() {
                        m.write_byte(output + 7 + i as u32, *b);
                    }
                    if t.exists {
                        Ok(())
                    } else {
                        Err(-43)
                    }
                })();
                (14, result.err().unwrap_or(0))
            }
            2 | 3 => {
                let destination = m.read_long(sp);
                let permission = m.read_byte(sp + 4);
                let spec = m.read_long(sp + 6);
                let result = self.spec(m, spec).and_then(|target| {
                    self.open_at(
                        target.parent,
                        &target.name,
                        if selector == 2 {
                            Fork::Data
                        } else {
                            Fork::Resource
                        },
                        permission,
                    )
                });
                let error = match result {
                    Ok(reference) => {
                        m.write_word(destination, reference as u16);
                        0
                    }
                    Err(error) => error,
                };
                (10, error)
            }
            4 => {
                let kind = m.read_long(sp + 2);
                let creator = m.read_long(sp + 6);
                let spec = m.read_long(sp + 10);
                let mut info = [0; 16];
                info[..4].copy_from_slice(&kind.to_be_bytes());
                info[4..8].copy_from_slice(&creator.to_be_bytes());
                let error = self
                    .spec(m, spec)
                    .and_then(|target| self.create_at(target.parent, &target.name, info))
                    .err()
                    .unwrap_or(0);
                (14, error)
            }
            6 => {
                let spec = m.read_long(sp);
                let error = self
                    .spec(m, spec)
                    .and_then(|target| self.delete_at(target.parent, &target.name))
                    .err()
                    .unwrap_or(0);
                (4, error)
            }
            7 | 8 => {
                let info = m.read_long(sp);
                let spec = m.read_long(sp + 4);
                let result = self.spec(m, spec).and_then(|target| {
                    if selector == 7 {
                        let value = self.finder_at(target.parent, &target.name)?;
                        for (i, b) in value.iter().enumerate() {
                            m.write_byte(info + i as u32, *b);
                        }
                        Ok(())
                    } else {
                        let mut value = [0; 16];
                        for (i, b) in value.iter_mut().enumerate() {
                            *b = m.read_byte(info + i as u32);
                        }
                        self.set_finder_at(target.parent, &target.name, value)
                    }
                });
                (8, result.err().unwrap_or(0))
            }
            _ => {
                return Err(format!(
                    "HighLevelFSDispatch selector {selector:#x} is not implemented"
                ))
            }
        };
        Ok((pop, error))
    }
    fn name<B: AddressBus>(m: &mut B, p: u32) -> Vec<u8> {
        let n = m.read_byte(p) as u32;
        (0..n).map(|i| m.read_byte(p + 1 + i)).collect()
    }
    pub fn parameter_block<B: AddressBus>(
        &mut self,
        m: &mut B,
        opcode: u16,
        p: u32,
    ) -> Result<(), i16> {
        let code = opcode & !0x200;
        let reference = m.read_word(p + 24) as i16;
        if [0xa000, 0xa00a].contains(&code) && opcode & 0x200 != 0 {
            m.write_word(p + 24, 0);
        }
        let volume = m.read_word(p + 22) as i16;
        let named = [0xa000, 0xa008, 0xa009, 0xa00a, 0xa00c, 0xa00d].contains(&code);
        let path = if named {
            {
                let n = m.read_long(p + 18);
                Self::name(m, n)
            }
        } else {
            Vec::new()
        };
        let target = if named {
            self.path_volume(volume, &path)?
        } else {
            if volume == 0 {
                self.current_volume
            } else {
                volume
            }
        };
        if code == 0xa015
            && self
                .source
                .as_ref()
                .is_some_and(|source| target == source.volume_ref)
        {
            let source = self.source.as_mut().unwrap();
            source.parameter_block(m, opcode, p)?;
            self.current_volume = target;
            return Ok(());
        }
        if let Some(source) = self.source.as_mut() {
            let named = [0xa000, 0xa008, 0xa009, 0xa00a, 0xa00c, 0xa00d, 0xa013].contains(&code)
                && target == source.volume_ref;
            let current = code == 0xa014 && self.current_volume == source.volume_ref;
            let by_ref = [0xa001, 0xa002, 0xa003, 0xa011, 0xa012, 0xa018, 0xa044].contains(&code)
                && source.open.contains_key(&reference);
            if named || current || by_ref {
                return source.parameter_block(m, opcode, p);
            }
        }

        if code == 0xa000 {
            let name_ptr = m.read_long(p + 18);
            let name = Self::name(m, name_ptr);
            if name.first() == Some(&b'.') {
                if std::env::args().any(|a| a == "--trace-files") {
                    println!("device open unsupported: name={:?} error=-43", String::from_utf8_lossy(&name));
                }
                return Err(-43);
            }
        }
        let resolved = if named {
            let dir = if opcode & 0x200 != 0 {
                m.read_long(p + 48)
            } else {
                0
            };
            let result = self.resolve_path(volume, dir, &path);
            Some(match result {
                Err(-120)
                    if [0xa000, 0xa009, 0xa00a].contains(&code)
                        && !self.catalog.contains(dir)
                        && !path.contains(&b':') =>
                {
                    return Err(-43)
                }
                other => other?,
            })
        } else {
            None
        };
        match code {
            0xa014 => {
                let name = m.read_long(p + 18);
                if name != 0 {
                    m.write_byte(name, self.volume_name.len() as u8);
                    for (i, b) in self.volume_name.iter().enumerate() {
                        m.write_byte(name + 1 + i as u32, *b);
                    }
                }
                m.write_word(p + 22, self.volume_ref as u16);
                if opcode & 0x200 != 0 {
                    m.write_long(p + 28, 0);
                    m.write_word(p + 32, self.volume_ref as u16);
                    m.write_long(p + 48, self.current_dir);
                }
                Ok(())
            }
            0xa015 => {
                if ![0, self.volume_ref].contains(&volume) {
                    return Err(-35);
                }
                let dir = if opcode & 0x200 != 0 {
                    m.read_long(p + 48)
                } else {
                    0
                };
                let next = if dir != 0 {
                    dir
                } else if volume == 0 {
                    self.current_dir
                } else {
                    2
                };
                if !self.catalog.contains(next) {
                    return Err(-120);
                }
                self.current_dir = next;
                self.current_volume = self.volume_ref;
                Ok(())
            }
            0xa000 | 0xa00a => {
                let name_ptr = m.read_long(p + 18);
                let name = Self::name(m, name_ptr);
                if std::env::args().any(|a| a == "--trace-files") {
                    println!("file open name={:?}", String::from_utf8_lossy(&name));
                }
                if name.first() == Some(&b'.') {
                    return Err(-43);
                }
                let t = resolved.as_ref().unwrap();
                let r = self.open_at(
                    t.parent,
                    &t.name,
                    if code == 0xa00a {
                        Fork::Resource
                    } else {
                        Fork::Data
                    },
                    m.read_byte(p + 27),
                )?;
                m.write_word(p + 24, r as u16);
                Ok(())
            }
            0xa001 => self.close(reference),
            0xa008 => {
                let t = resolved.as_ref().unwrap();
                self.create_at(t.parent, &t.name, [0; 16])
            }
            0xa009 => {
                let t = resolved.as_ref().unwrap();
                if t.parent == 2 {
                    self.delete(&t.name)
                } else {
                    self.delete_at(t.parent, &t.name)
                }
            }
            0xa002 | 0xa003 => {
                // OS 9 rejects a read-only write before touching ioActCount
                // or the file position (AppSourceProbe's initialized PB).
                if code == 0xa003 && self.open.get(&reference).is_some_and(|o| o.permission == 1) {
                    return Err(-61);
                }
                m.write_long(p + 40, 0);
                let count = m.read_long(p + 36) as i32;
                if count < 0 || count as usize > MAX_FORK {
                    return Err(-50);
                }
                let mode = m.read_word(p + 44);
                let offset = m.read_long(p + 46) as i32;
                self.seek(reference, mode, offset)?;
                let buffer = m.read_long(p + 32);
                let error = if code == 0xa002 {
                    let (bytes, error) = self.read(reference, count as usize)?;
                    for (i, b) in bytes.iter().enumerate() {
                        m.write_byte(buffer + i as u32, *b);
                    }
                    m.write_long(p + 40, bytes.len() as u32);
                    error
                } else {
                    let bytes: Vec<u8> =
                        (0..count as u32).map(|i| m.read_byte(buffer + i)).collect();
                    let n = self.write(reference, &bytes)?;
                    m.write_long(p + 40, n as u32);
                    0
                };
                m.write_long(p + 46, self.position(reference)? as u32);
                if error == 0 {
                    Ok(())
                } else {
                    Err(error)
                }
            }
            0xa011 => {
                m.write_long(p + 28, self.eof(reference)? as u32);
                Ok(())
            }
            0xa012 => {
                let len = m.read_long(p + 28) as i32;
                if len < 0 {
                    return Err(-50);
                }
                self.set_eof(reference, len as usize)
            }
            0xa018 => {
                m.write_long(p + 46, self.position(reference)? as u32);
                Ok(())
            }
            0xa044 => {
                let mode = m.read_word(p + 44);
                let at = m.read_long(p + 46) as i32;
                let result = self.seek(reference, mode, at);
                if let Ok(mark) = self.position(reference) {
                    m.write_long(p + 46, mark as u32);
                }
                result
            }
            0xa00c | 0xa00d => {
                if m.read_word(p + 28) != 0 {
                    return Err(-50);
                }
                if code == 0xa00c {
                    let t = resolved.as_ref().unwrap();
                    let info = self.finder_at(t.parent, &t.name)?;
                    for (i, b) in info.iter().enumerate() {
                        m.write_byte(p + 32 + i as u32, *b);
                    }
                    let file = &self.files[&Self::file_key(t.parent, &t.name)?];
                    m.write_long(p + 54, file.data.len() as u32);
                    m.write_long(p + 64, file.resource.len() as u32);
                    Ok(())
                } else {
                    let mut info = [0; 16];
                    for (i, b) in info.iter_mut().enumerate() {
                        *b = m.read_byte(p + 32 + i as u32);
                    }
                    let t = resolved.as_ref().unwrap();
                    self.set_finder_at(t.parent, &t.name, info)
                }
            }
            0xa013 => {
                self.flush()?;
                for open in self.open.values_mut() {
                    open.dirty = false;
                }
                Ok(())
            }
            _ => Err(-50),
        }
    }
    pub fn dispatch<B: AddressBus>(
        &mut self,
        opcode: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
    ) -> Result<bool, String> {
        if opcode == 0xa207 {
            let p = cpu.dar[8];
            let index = m.read_word(p + 28) as i16;
            let volume = m.read_word(p + 22) as i16;
            let error = self.get_volume_info(m, p).err().unwrap_or(0);
            m.write_long(p, 0);
            m.write_word(p + 4, 5);
            m.write_word(p + 6, opcode);
            m.write_long(p + 8, 0);
            m.write_word(p + 16, error as u16);
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr(if error == 0 {
                20
            } else if index > 0 {
                8
            } else {
                24
            });
            cpu.pc += 2;
            if std::env::args().any(|a| a == "--trace-files") {
                println!("file HGetVInfo index={index} volume={volume} error={error} free_blocks={} block_bytes={}",m.read_word(p+62),m.read_long(p+48));
            }
            return Ok(true);
        }
        if opcode == 0xa045 {
            let p = cpu.dar[8];
            let reference = m.read_word(p + 24) as i16;
            let result = self.flush_file(reference);
            let error = result.as_ref().err().copied().unwrap_or(0);
            // Public synchronous File Manager queue/header fields. The
            // owner-private native ROM command pointer has no hosted analog.
            m.write_long(p, 0);
            m.write_word(p + 4, 5);
            m.write_word(p + 6, opcode);
            m.write_long(p + 8, 0);
            m.write_word(p + 16, error as u16);
            cpu.dar[0] = error as i32 as u32;
            let x = match result {
                Ok(dirty) => {
                    if dirty {
                        0
                    } else {
                        16
                    }
                }
                Err(-38) => 16,
                Err(_) => {
                    if cpu.flag_x() {
                        16
                    } else {
                        0
                    }
                }
            };
            cpu.set_ccr(x | if error == 0 { 4 } else { 8 });
            cpu.pc += 2;
            return Ok(true);
        }
        if matches!(opcode, 0xa060 | 0xa260) && cpu.dar[0] as u16 == 0x1a {
            let p = cpu.dar[8];
            let hierarchical = opcode & 0x200 != 0;
            let volume = m.read_word(p + 22) as i16;
            let name_pointer = m.read_long(p + 18);
            let path = Self::name(m, name_pointer);
            let directory = if hierarchical { m.read_long(p + 48) } else { 0 };
            // A failure before the final catalog lookup has different native
            // X status from an ordinary file-not-found result. Resolve the
            // directory identity in the same logical volume as the open.
            let invalid_directory = directory != 0
                && !path.contains(&b':')
                && self.path_volume(volume, &path).is_ok_and(|target| {
                    let catalog = self
                        .source
                        .as_ref()
                        .filter(|s| s.volume_ref == target)
                        .map_or(&self.catalog, |s| &s.catalog);
                    !catalog.contains(directory)
                });
            m.write_word(p + 24, 0);
            let error = self
                .parameter_block(m, if hierarchical { 0xa200 } else { 0xa000 }, p)
                .err()
                .unwrap_or(0);
            m.write_word(p + 16, error as u16);
            cpu.dar[0] = error as i32 as u32;
            cpu.set_ccr(if error == 0 {
                20
            } else if error != -43 || invalid_directory {
                24
            } else {
                8
            });
            if std::env::args().any(|a| a == "--trace-files") {
                println!("file OpenDF hierarchical={hierarchical} volume={volume} directory={directory} name={:?} error={error}", String::from_utf8_lossy(&path));
            }
            cpu.pc += 2;
            return Ok(true);
        }
        if opcode == 0xa260 {
            let selector = cpu.dar[0] as u16;
            let p = cpu.dar[8];
            if selector == 8 && std::env::args().any(|a| a == "--trace-files") {
                println!(
                    "FCB input: ref={} index={} volume={} current_app_ref={} current_resource={}",
                    m.read_word(p + 24) as i16,
                    m.read_word(p + 28) as i16,
                    m.read_word(p + 22) as i16,
                    m.read_word(0x900) as i16,
                    m.read_word(0xa5a) as i16
                );
            }
            if selector == 9 && std::env::args().any(|a| a == "--trace-files") {
                let name = m.read_long(p + 18);
                let bytes = if name == 0 {
                    Vec::new()
                } else {
                    Self::name(m, name)
                };
                println!("catalog input: volume={} directory={} index={} name_ptr={name:#x} name={bytes:?}",m.read_word(p+22) as i16,m.read_long(p+48),m.read_word(p+28) as i16);
            }
            let result = match selector {
                6 => self.create_directory_pb(m, p),
                8 => self.fcb_info(m, p),
                9 => self.catalog_info(m, p),
                7 => {
                    let volume = m.read_word(p + 22) as i16;
                    let target = if volume == 0 {
                        self.current_volume
                    } else {
                        volume
                    };
                    if let Some(source) = self.source.as_mut().filter(|s| s.volume_ref == target) {
                        source.parameter_block(m, 0xa214, p)
                    } else if target == self.volume_ref {
                        let old = self.current_volume;
                        self.current_volume = self.volume_ref;
                        let result = self.parameter_block(m, 0xa214, p);
                        self.current_volume = old;
                        result
                    } else {
                        Err(-35)
                    }
                }
                _ => {
                    return Err(format!(
                        "FSDispatch selector {selector:#x} at PC {:#x} is not implemented",
                        cpu.pc
                    ))
                }
            };
            let error = result.err().unwrap_or(0);
            if std::env::args().any(|a| a == "--trace-files") {
                println!("file FSDispatch selector={selector:#x} error={error}");
            }
            m.write_word(p + 16, error as u16);
            cpu.dar[0] = error as i32 as u32;
            let x = if cpu.flag_x() { 16 } else { 0 };
            cpu.set_ccr(x | if error == 0 { 4 } else { 8 });
            cpu.pc += 2;
            return Ok(true);
        }
        if opcode == 0xaa52 {
            let sp = cpu.dar[15];
            if std::env::args().any(|a| a == "--trace-files") {
                let selector = cpu.dar[0] as u16;
                let offset = match selector {
                    1 => None,
                    2 | 3 => Some(6),
                    4 | 14 => Some(10),
                    6 => Some(0),
                    7 | 8 => Some(4),
                    _ => None,
                };
                if let Some(offset) = offset {
                    let spec = m.read_long(sp + offset);
                    let name = Self::name(m, spec + 6);
                    println!("file FSSpec input: selector={selector} spec={spec:#x} volume={} parent={} name={name:?}",m.read_word(spec) as i16,m.read_long(spec+2));
                }
            }

            if cpu.dar[0] as u16 == 14 {
                let spec = m.read_long(sp + 10);
                let creator = m.read_long(sp + 6);
                let kind = m.read_long(sp + 2);
                let script = m.read_word(sp) as i16;
                let error = self.create_resource_file(m, spec, creator, kind, script)?;
                m.write_word(0xa60, error as u16);
                cpu.dar[15] += 14;
                cpu.pc += 2;
                if std::env::args().any(|a| a == "--trace-files") {
                    println!("FSpCreateResFile error={error}");
                }
                return Ok(true);
            }

            let (pop, error) = self.high_level(cpu.dar[0] as u16, m, sp)?;
            if std::env::args().any(|a| a == "--trace-files") {
                println!("file FSSpec selector={} error={error}", cpu.dar[0] & 15);
            }
            m.write_word(sp + pop, error as u16);
            cpu.dar[15] += pop;
            cpu.pc += 2;
            return Ok(true);
        }
        if ![
            0xa000, 0xa200, 0xa001, 0xa002, 0xa003, 0xa008, 0xa208, 0xa009, 0xa209, 0xa00a, 0xa20a,
            0xa00c, 0xa20c, 0xa00d, 0xa20d, 0xa011, 0xa012, 0xa018, 0xa044, 0xa013, 0xa014, 0xa015,
            0xa214, 0xa215,
        ]
        .contains(&opcode)
        {
            return Ok(false);
        }
        let p = cpu.dar[8];
        let error = self.parameter_block(m, opcode, p).err().unwrap_or(0);
        if std::env::args().any(|a| a == "--trace-files") {
            println!("file PB trap={opcode:#x} error={error} ref={} volume={} mode={} offset={} requested={} actual={}",m.read_word(p+24) as i16,m.read_word(p+22) as i16,m.read_word(p+44),m.read_long(p+46),m.read_long(p+36),m.read_long(p+40));
        }
        m.write_word(p + 16, error as u16);
        cpu.dar[0] = error as i32 as u32;
        let x = if cpu.flag_x() { 16 } else { 0 };
        cpu.set_ccr(
            x | if error == 0 {
                4
            } else if error < 0 {
                8
            } else {
                0
            },
        );
        cpu.pc += 2;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    static NEXT_DIRECTORY: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    fn directory() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .join("work/test-volumes")
            .join(format!(
                "{}-{stamp}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ))
    }
    #[test]
    fn catalog_metadata_persists_and_checkpoint_retains_prior_values() {
        let root = directory();
        let folder;
        {
            let mut f = Files::volume(&root).unwrap();
            folder = f.create_directory(2, b"Folder").unwrap();
            f.catalog.metadata_mut(folder).unwrap().dates = [1, 2, 3];
            f.create(b"Original", [0; 16]).unwrap();
            let entry = f.files.get_mut(&key(b"Original").unwrap()).unwrap();
            entry.dates = [4, 5, 6];
            entry.extended = [7; 16];
            f.flush().unwrap();
        }
        {
            let mut f = Files::volume(&root).unwrap();
            assert_eq!(f.catalog.metadata(folder).unwrap().dates, [1, 2, 3]);
            assert_eq!(f.files[&key(b"Original").unwrap()].dates, [4, 5, 6]);
            assert_eq!(f.files[&key(b"Original").unwrap()].extended, [7; 16]);
            let r = f.open(b"Original", Fork::Data, 3).unwrap();
            f.write(r, b"changed").unwrap();
            f.close(r).unwrap();
        }
        let record = fs::read_dir(root.join("recovery"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
            .join("manifest.json");
        let record: serde_json::Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
        assert_eq!(record["metadata"]["dates"], serde_json::json!([4, 5, 6]));
        assert_eq!(
            record["metadata"]["extended"],
            serde_json::json!(vec![7u8; 16])
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn file_identity_is_stable_across_cold_load_and_fcb_queries_real_source_handle() {
        let root = directory();
        let id;
        {
            let mut f = Files::volume(&root).unwrap();
            f.create(b"Original", [0; 16]).unwrap();
            id = f.files[&key(b"Original").unwrap()].id;
        }
        {
            let mut f = Files::volume(&root).unwrap();
            assert_eq!(f.files[&key(b"Original").unwrap()].id, id);
            f.create_directory(2, b"Folder").unwrap();
            assert_ne!(f.catalog.child(2, b"Folder").unwrap().0, id);
        }
        let mut f = Files::new();
        f.mount_application(b"OwnApp", [0; 16], b"", b"resource")
            .unwrap();
        let mut memory = m68k::LinearMemoryBus::new(65536);
        let p = 0x2000;
        memory.write_word(p + 24, APPLICATION_RESOURCE_FILE as u16);
        memory.write_word(p + 28, 0);
        memory.write_word(p + 22, (-123i16) as u16);
        f.fcb_info(&mut memory, p).unwrap();
        assert_eq!(memory.read_word(p + 36), 0x200);
        assert_eq!(memory.read_long(p + 40), 8);
        assert_eq!(memory.read_word(p + 52) as i16, APPLICATION_VOLUME);
        assert_eq!(memory.read_long(p + 58), 2);
        f.source
            .as_mut()
            .unwrap()
            .close(APPLICATION_RESOURCE_FILE)
            .unwrap();
        assert_eq!(f.fcb_info(&mut memory, p), Err(-38));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn nested_catalog_persists_equal_leaf_names_and_source_mount_does_not_mutate_it() {
        let root = directory();
        let (left, right);
        {
            let mut f = Files::volume(&root).unwrap();
            left = f.create_directory(2, b"Left").unwrap();
            right = f.create_directory(2, b"Right").unwrap();
            for (parent, data) in [(left, b"LEFT"), (right, b"RGHT")] {
                f.create_at(parent, b"Map", [0; 16]).unwrap();
                let r = f.open_at(parent, b"Map", Fork::Data, 3).unwrap();
                f.write(r, data).unwrap();
                f.close(r).unwrap();
            }
        }
        let catalog = fs::read(root.join("catalog.json")).unwrap();
        {
            let mut f = Files::volume(&root).unwrap();
            assert_eq!(f.resolve_path(-1, 2, b":left:map").unwrap().parent, left);
            for (parent, data) in [(left, b"LEFT"), (right, b"RGHT")] {
                let r = f.open_at(parent, b"Map", Fork::Data, 1).unwrap();
                assert_eq!(f.read(r, 4).unwrap(), (data.to_vec(), 0));
                f.close(r).unwrap();
            }
            assert_eq!(f.create_at(2, b"Left", [0; 16]), Err(-48));
        }
        assert_eq!(fs::read(root.join("catalog.json")).unwrap(), catalog);
        let mut f = Files::new();
        f.mount_application_files(&root, b"OwnedApp", [0; 16], b"data", b"resource")
            .unwrap();
        assert_eq!(fs::read(root.join("catalog.json")).unwrap(), catalog);
        let source = f.source.as_mut().unwrap();
        assert_eq!(source.create_directory(2, b"New"), Err(-44));
        assert_eq!(source.delete_at(left, b"Map"), Err(-44));
        let r = source.open_at(left, b"Map", Fork::Data, 0).unwrap();
        assert_eq!(source.read(r, 4).unwrap().0, b"LEFT");
        assert_eq!(source.write(r, b"oops"), Err(-61));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn application_source_is_readonly_and_absent_from_document_catalog() {
        let root = directory();
        let mut files = Files::volume(&root).unwrap();
        files.create(b"Document", [0; 16]).unwrap();
        files
            .mount_application(b"Application", *b"APPLtest12345678", b"data", b"resources")
            .unwrap();
        let source = files.source.as_mut().unwrap();
        assert_eq!(source.create(b"New", [0; 16]), Err(-44));
        assert_eq!(source.delete(b"Application"), Err(-44));
        assert_eq!(source.set_finder(b"Application", [0; 16]), Err(-44));
        for permission in [2, 3, 4] {
            assert_eq!(
                source.open(b"Application", Fork::Data, permission),
                Err(-44)
            );
        }
        let r = source.open(b"Application", Fork::Data, 0).unwrap();
        assert_eq!(source.write(r, b"change"), Err(-61));
        assert_eq!(source.set_eof(r, 0), Err(-61));
        assert_eq!(
            source.fork_bytes(b"Application", Fork::Data).unwrap(),
            b"data"
        );
        assert_eq!(
            source.fork_bytes(b"Application", Fork::Resource).unwrap(),
            b"resources"
        );
        drop(files);
        let reopened = Files::volume(&root).unwrap();
        assert_eq!(reopened.list(&[]), vec![b"Document".to_vec()]);
        assert_eq!(reopened.finder(b"Application"), Err(-43));
        drop(reopened);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn host_volume_preserves_both_forks_and_finder_info_across_reload() {
        let root = directory();
        let mut info = [0; 16];
        info[..4].copy_from_slice(b"TEXT");
        info[4..8].copy_from_slice(b"ttxt");
        info[8] = 0x40;
        {
            let mut files = Files::volume(&root).unwrap();
            files.create(b"Original", info).unwrap();
            let data = files.open(b"ORIGINAL", Fork::Data, 0).unwrap();
            files.write(data, b"a\0b\xff").unwrap();
            files.close(data).unwrap();
            let resource = files.open(b"original", Fork::Resource, 0).unwrap();
            files.write(resource, b"resource\0bytes").unwrap();
            files.close(resource).unwrap();
        }
        let mut reopened = Files::volume(&root).unwrap();
        assert_eq!(reopened.finder(b"Original").unwrap(), info);
        let data = reopened.open(b"Original", Fork::Data, 1).unwrap();
        assert_eq!(reopened.read(data, 4).unwrap(), (b"a\0b\xff".to_vec(), 0));
        assert_eq!(reopened.write(data, b"overwrite"), Err(-61));
        assert_eq!(reopened.position(data).unwrap(), 4);
        reopened.close(data).unwrap();
        let resource = reopened.open(b"Original", Fork::Resource, 1).unwrap();
        assert_eq!(
            reopened.read(resource, 14).unwrap(),
            (b"resource\0bytes".to_vec(), 0)
        );
        reopened.close(resource).unwrap();
        reopened.delete(b"Original").unwrap();
        drop(reopened);
        let empty = Files::volume(&root).unwrap();
        assert_eq!(empty.finder(b"Original"), Err(-43));
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn logical_volume_lock_prevents_two_writers_and_releases_on_drop() {
        let root = directory();
        let first = Files::volume(&root).unwrap();
        assert!(Files::volume(&root).is_err());
        drop(first);
        drop(Files::volume(&root).unwrap());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn recovery_retains_both_forks_and_metadata_across_interrupted_replacement() {
        let root = directory();
        let mut info = [0; 16];
        info[..8].copy_from_slice(b"TEXTttxt");
        info[8] = 0x40;
        {
            let mut f = Files::volume(&root).unwrap();
            f.create(b"Original", info).unwrap();
            let r = f.open(b"Original", Fork::Data, 0).unwrap();
            f.write(r, b"original data").unwrap();
            f.close(r).unwrap();
            let r = f.open(b"Original", Fork::Resource, 0).unwrap();
            f.write(r, b"original resources").unwrap();
            f.close(r).unwrap();
        }
        {
            let mut f = Files::volume(&root).unwrap();
            f.delete(b"Original").unwrap();
            f.create(b"Original", [0; 16]).unwrap();
            let r = f.open(b"Original", Fork::Data, 0).unwrap();
            f.write(r, b"partial").unwrap();
        }
        let current = Files::volume(&root).unwrap();
        assert_eq!(
            current.fork_bytes(b"Original", Fork::Data).unwrap(),
            b"partial"
        );
        let snapshots: Vec<_> = fs::read_dir(root.join("recovery"))
            .unwrap()
            .map(|d| d.unwrap().path())
            .filter(|d| {
                fs::read(d.join("data")).unwrap() == b"original data"
                    && fs::read(d.join("resource")).unwrap() == b"original resources"
            })
            .collect();
        assert!(!snapshots.is_empty());
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(snapshots[0].join("manifest.json")).unwrap()).unwrap();
        assert_eq!(manifest["name"], serde_json::json!(b"Original"));
        assert_eq!(manifest["finder"], serde_json::json!(info));
        drop(current);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn failed_recovery_checkpoint_refuses_changes_and_preserves_original() {
        let root = directory();
        let info = [7; 16];
        {
            let mut f = Files::volume(&root).unwrap();
            f.create(b"Keep", info).unwrap();
            let r = f.open(b"Keep", Fork::Data, 0).unwrap();
            f.write(r, b"keep me").unwrap();
            f.close(r).unwrap();
        }
        fs::write(root.join("recovery"), b"not a directory").unwrap();
        {
            let mut f = Files::volume(&root).unwrap();
            let r = f.open(b"Keep", Fork::Data, 0).unwrap();
            assert_eq!(f.set_eof(r, 0), Err(-36));
            assert_eq!(f.write(r, b"damage"), Err(-36));
            assert_eq!(f.set_finder(b"Keep", [0; 16]), Err(-36));
            f.close(r).unwrap();
            assert_eq!(f.delete(b"Keep"), Err(-36));
            assert_eq!(f.fork_bytes(b"Keep", Fork::Data).unwrap(), b"keep me");
            assert_eq!(f.finder(b"Keep").unwrap(), info);
        }
        let current = Files::volume(&root).unwrap();
        assert_eq!(current.fork_bytes(b"Keep", Fork::Data).unwrap(), b"keep me");
        assert_eq!(current.finder(b"Keep").unwrap(), info);
        drop(current);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unrecognized_nonempty_host_directory_is_not_converted() {
        let root = directory();
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("existing.txt"), b"preserve").unwrap();
        assert!(Files::volume(&root).is_err());
        assert_eq!(fs::read(root.join("existing.txt")).unwrap(), b"preserve");
        assert!(!root.join("catalog.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn resource_map_creation_is_durable_preserves_data_and_checkpoints_old_metadata() {
        let root = directory();
        let old = *b"BINAOld!12345678";
        let creator = u32::from_be_bytes(*b"CrPr");
        let kind = u32::from_be_bytes(*b"TEXT");
        {
            let mut files = Files::volume(&root).unwrap();
            files.create(b"Document", old).unwrap();
            let r = files.open(b"Document", Fork::Data, 3).unwrap();
            files.write(r, b"keep").unwrap();
            files.close(r).unwrap();
            files
                .create_resource_map(b"Document", creator, kind)
                .unwrap();
            assert_eq!(files.create_resource_map(b"Document", 0, 0), Err(-48));
        }
        {
            let files = Files::volume(&root).unwrap();
            assert_eq!(files.fork_bytes(b"Document", Fork::Data).unwrap(), b"keep");
            let fork = files.fork_bytes(b"Document", Fork::Resource).unwrap();
            assert_eq!(fork, crate::resource::empty_fork());
            assert!(crate::resource::parse(fork).unwrap().is_empty());
            let mut expected = old;
            expected[..8].copy_from_slice(b"TEXTCrPr");
            assert_eq!(files.finder(b"Document").unwrap(), expected);
        }
        let checkpoints: Vec<_> = fs::read_dir(root.join("recovery"))
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        assert_eq!(checkpoints.len(), 1);
        let prior = &checkpoints[0];
        assert_eq!(fs::read(prior.join("data")).unwrap(), b"keep");
        assert!(fs::read(prior.join("resource")).unwrap().is_empty());
        let metadata: serde_json::Value =
            serde_json::from_slice(&fs::read(prior.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(metadata["finder"], serde_json::json!(old.to_vec()));
        fs::remove_dir_all(root).unwrap();
    }
}

#[cfg(test)]
mod volume_accounting_tests {
    use super::*;
    use crate::machine::Memory;
    fn query(f: &mut Files, m: &mut Memory, reference: i16, index: i16) -> (u16, u32, u32, u32) {
        let p = 0x4000;
        m.write_long(p + 18, 0);
        m.write_word(p + 22, reference as u16);
        m.write_word(p + 28, index as u16);
        let mut cpu = m68k::CpuCore::new();
        cpu.dar[8] = p;
        cpu.pc = 0x6000;
        assert!(f.dispatch(0xa207, &mut cpu, m).unwrap());
        assert_eq!(cpu.dar[0], 0);
        (
            m.read_word(p + 62),
            m.read_long(p + 48),
            m.read_long(p + 82),
            m.read_long(p + 86),
        )
    }
    #[test]
    fn both_forks_catalog_and_persisted_capacity_drive_reported_free_space() {
        let root = std::env::temp_dir().join(format!(
            "classic-volume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut f = Files::volume(&root).unwrap();
        let mut m = Memory::zeroed(0x10000);
        f.volume_modified.set(0x12345678);
        let (initial, bytes, _, _) = query(&mut f, &mut m, 0, 0);
        assert_eq!(bytes, 4096);
        f.create(b"Both", [0; 16]).unwrap();
        let r = f.open(b"Both", Fork::Data, 3).unwrap();
        let s = f.open(b"Both", Fork::Resource, 3).unwrap();
        f.write(r, b"D").unwrap();
        f.write(s, &vec![0x5a; 4097]).unwrap();
        let (free, _, files, dirs) = query(&mut f, &mut m, 0, 0);
        assert_eq!(initial - free, 4);
        assert_eq!((files, dirs), (1, 0));
        f.set_eof(s, 0).unwrap();
        assert_eq!(query(&mut f, &mut m, 0, 0).0, free + 2);
        assert_eq!(m.read_long(0x4000 + 34), 0x12345678);
        let allocated_id = f.catalog.next_id();
        f.close(r).unwrap();
        f.close(s).unwrap();
        f.delete(b"Both").unwrap();
        assert_eq!(query(&mut f, &mut m, 0, 0).0, initial);
        let before = f.allocation_blocks;
        drop(f);
        let mut f = Files::volume(&root).unwrap();
        assert_eq!(f.allocation_blocks, before);
        assert_eq!(f.volume_modified.get(), 0x12345678);
        assert_eq!(query(&mut f, &mut m, 0, 0).0, initial);
        assert!(f.catalog.next_id() >= allocated_id);
        f.create(b"Next", [0; 16]).unwrap();
        assert!(f.files[&key(b"Next").unwrap()].id >= allocated_id);
        drop(f);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn disk_full_rejects_growth_before_data_position_and_catalog_mutation() {
        let mut f = Files::new();
        f.allocation_blocks = 18;
        let mut m = Memory::zeroed(0x10000);
        f.create(b"Limit", [0; 16]).unwrap();
        let r = f.open(b"Limit", Fork::Data, 3).unwrap();
        f.write(r, b"safe").unwrap();
        assert_eq!(query(&mut f, &mut m, 0, 0).0, 0);
        let position = f.position(r);
        let count = f.volume_writes.get();
        assert_eq!(f.set_eof(r, 4097), Err(-34));
        assert_eq!(f.position(r), position);
        assert_eq!(f.volume_writes.get(), count);
        assert_eq!(f.fork_bytes(b"Limit", Fork::Data).unwrap(), b"safe");
        assert_eq!(f.write(r, &vec![b'X'; 4093]), Err(-34));
        assert_eq!(f.position(r), position);
        assert_eq!(f.eof(r), Ok(4));
        f.set_eof(r, 0).unwrap();
        assert_eq!(query(&mut f, &mut m, 0, 0).0, 1);
    }
    #[test]
    fn full_catalog_reserves_no_identity_and_preserves_validation_errors() {
        let mut f = Files::new();
        f.allocation_blocks = 17;
        for i in 0..16 {
            f.create(format!("File{i}").as_bytes(), [0; 16]).unwrap();
        }
        let next = f.catalog.next_id();
        let writes = f.volume_writes.get();
        assert_eq!(f.create(b"NoSpace", [0; 16]), Err(-34));
        assert_eq!(f.create_directory(2, b"NoSpace"), Err(-34));
        assert_eq!(f.catalog.next_id(), next);
        assert_eq!(f.files.len(), 16);
        assert_eq!(f.catalog.entries().count(), 0);
        assert_eq!(f.volume_writes.get(), writes);
        assert_eq!(f.create_directory(12345, b"NoParent"), Err(-120));
        assert_eq!(f.create_directory(2, b""), Err(-37));
        assert_eq!(f.create(b"File0", [0; 16]), Err(-48));
    }
    #[test]
    fn legacy_directory_valence_follows_selection_while_volume_counts_stay_global() {
        let mut f = Files::new();
        let mut m = Memory::zeroed(0x10000);
        let child = f.create_directory(2, b"Child").unwrap();
        f.create(b"RootFile", [0; 16]).unwrap();
        f.create_at(child, b"A", [0; 16]).unwrap();
        f.create_at(child, b"B", [0; 16]).unwrap();
        f.create_directory(child, b"Nested").unwrap();
        f.current_dir = child;
        let totals = query(&mut f, &mut m, 0, 0);
        assert_eq!(m.read_word(0x4000 + 40), 3);
        assert_eq!((totals.2, totals.3), (3, 2));
        assert_eq!(query(&mut f, &mut m, 0, 1), totals);
        assert_eq!(m.read_word(0x4000 + 40), 2);
    }
    #[test]
    fn volume_enumeration_current_selection_and_locked_source_are_independent() {
        let mut f = Files::new();
        f.mount_application(b"Owned", [0; 16], b"DF", b"RF")
            .unwrap();
        let mut m = Memory::zeroed(0x10000);
        let own = query(&mut f, &mut m, 0, 0);
        assert_eq!(m.read_word(0x4000 + 22) as i16, -1);
        assert_eq!(own.2, 0);
        let source = query(&mut f, &mut m, 0, 2);
        assert_eq!(source.2, 1);
        assert_eq!(m.read_word(0x4000 + 22) as i16, APPLICATION_VOLUME);
        assert_eq!(m.read_word(0x4000 + 38), 0x0080);
        assert_eq!(query(&mut f, &mut m, 0, 0), own);
        f.current_volume = APPLICATION_VOLUME;
        assert_eq!(query(&mut f, &mut m, 0, 0), source);
        assert_eq!(query(&mut f, &mut m, 0, 1), own);
    }
}
