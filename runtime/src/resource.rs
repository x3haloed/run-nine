//! Bounds-checked Macintosh resource-fork reader. No ROM or System dependency.
#[derive(Debug)]
pub struct Resource<'a> {
    pub kind: [u8; 4],
    pub id: i16,
    pub name: Option<&'a [u8]>,
    pub attributes: u8,
    pub data: &'a [u8],
    pub map: &'a [u8],
    pub reference_offset: usize,
}
#[derive(Clone)]
pub struct Data {
    pub kind: [u8; 4],
    pub id: i16,
    pub attributes: u8,
    pub name: Option<Vec<u8>>,
    pub bytes: Vec<u8>,
}
/// Serialize the public classic resource-fork format. Runtime map links and
/// loaded-handle addresses are never persisted by this independent writer.
pub fn serialize(entries: &[Data]) -> Result<Vec<u8>, String> {
    let mut groups: Vec<([u8; 4], Vec<&Data>)> = Vec::new();
    for e in entries {
        if e.name.as_ref().is_some_and(|n| n.len() > 255) {
            return Err("resource name exceeds255 bytes".into());
        }
        if let Some((_, items)) = groups.iter_mut().find(|(kind, _)| *kind == e.kind) {
            if items.iter().any(|r| r.id == e.id) {
                return Err("duplicate resource ID in type".into());
            }
            items.push(e);
        } else {
            groups.push((e.kind, vec![e]));
        }
    }
    let type_bytes = 2 + groups.len() * 8;
    let refs_bytes = entries.len() * 12;
    if type_bytes + refs_bytes > u16::MAX as usize || groups.len() > 65535 {
        return Err("resource map exceeds classic offset limits".into());
    }
    let mut data = Vec::new();
    let mut names = Vec::new();
    let mut types = Vec::new();
    let mut refs = Vec::new();
    types.extend_from_slice(
        &(if groups.is_empty() {
            0xffff
        } else {
            groups.len() as u16 - 1
        })
        .to_be_bytes(),
    );
    for (kind, items) in &groups {
        types.extend_from_slice(kind);
        types.extend_from_slice(&(items.len() as u16 - 1).to_be_bytes());
        types.extend_from_slice(&((type_bytes + refs.len()) as u16).to_be_bytes());
        for e in items {
            if data.len() > 0xffffff || e.bytes.len() > u32::MAX as usize {
                return Err("resource data exceeds classic offset limits".into());
            }
            refs.extend_from_slice(&e.id.to_be_bytes());
            let name_offset = if let Some(name) = e.name.as_ref().filter(|n| !n.is_empty()) {
                if names.len() >= 65535 {
                    return Err("resource name list exceeds classic offset limits".into());
                }
                let offset = names.len() as u16;
                names.push(name.len() as u8);
                names.extend_from_slice(name);
                offset
            } else {
                0xffff
            };
            refs.extend_from_slice(&name_offset.to_be_bytes());
            refs.extend_from_slice(
                &(((e.attributes as u32) << 24) | data.len() as u32).to_be_bytes(),
            );
            refs.extend_from_slice(&0u32.to_be_bytes());
            data.extend_from_slice(&(e.bytes.len() as u32).to_be_bytes());
            data.extend_from_slice(&e.bytes);
        }
    }
    let map_length = 28 + types.len() + refs.len() + names.len();
    let map_offset = 256usize
        .checked_add(data.len())
        .ok_or("resource file size overflow")?;
    let name_offset = 28 + types.len() + refs.len();
    if name_offset > 65535 || map_offset > u32::MAX as usize || map_length > u32::MAX as usize {
        return Err("resource map exceeds classic offset limits".into());
    }
    let mut bytes = vec![0; 256];
    for (offset, value) in [
        (0, 256u32),
        (4, map_offset as u32),
        (8, data.len() as u32),
        (12, map_length as u32),
    ] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    let mut map = bytes[..16].to_vec();
    map.extend_from_slice(&[0; 8]);
    map.extend_from_slice(&28u16.to_be_bytes());
    map.extend_from_slice(&(name_offset as u16).to_be_bytes());
    map.extend_from_slice(&types);
    map.extend_from_slice(&refs);
    map.extend_from_slice(&names);
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(&map);
    Ok(bytes)
}

/// A new empty classic resource fork: reserved header area, no data, and a
/// resource map with an empty type list. No native binary template is imported.
pub fn empty_fork() -> Vec<u8> {
    let mut bytes = vec![0u8; 286];
    for (offset, value) in [(0, 256u32), (4, 256), (8, 0), (12, 30)] {
        bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
    }
    let header: Vec<_> = bytes[..16].to_vec();
    bytes[256..272].copy_from_slice(&header);
    bytes[280..282].copy_from_slice(&28u16.to_be_bytes());
    bytes[282..284].copy_from_slice(&30u16.to_be_bytes());
    bytes[284..286].copy_from_slice(&0xffffu16.to_be_bytes());
    bytes
}

fn slice(b: &[u8], p: usize, n: usize) -> Result<&[u8], String> {
    b.get(p..p.checked_add(n).ok_or("offset overflow")?)
        .ok_or_else(|| format!("truncated resource fork at {p:#x}, length {n}"))
}
fn u16be(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_be_bytes(slice(b, p, 2)?.try_into().unwrap()))
}
fn u32be(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_be_bytes(slice(b, p, 4)?.try_into().unwrap()))
}
pub fn parse(b: &[u8]) -> Result<Vec<Resource<'_>>, String> {
    let data_base = u32be(b, 0)? as usize;
    let map_base = u32be(b, 4)? as usize;
    let data = slice(b, data_base, u32be(b, 8)? as usize)?;
    let map = slice(b, map_base, u32be(b, 12)? as usize)?;
    let types = u16be(map, 24)? as usize;
    let count = u16be(map, types)?;
    if count == 0xffff {
        return Ok(Vec::new());
    }
    let mut resources = Vec::new();
    for i in 0..=count as usize {
        let p = types + 2 + i * 8;
        let kind = slice(map, p, 4)?.try_into().unwrap();
        let n = u16be(map, p + 4)?;
        let refs = types + u16be(map, p + 6)? as usize;
        for j in 0..=n as usize {
            let r = refs + j * 12;
            let id = u16be(map, r)? as i16;
            let packed = u32be(map, r + 4)?;
            let offset = (packed & 0x00ff_ffff) as usize;
            let len = u32be(data, offset)? as usize;
            let name_offset = u16be(map, r + 2)?;
            let name = if name_offset == 0xffff {
                None
            } else {
                let position = u16be(map, 26)? as usize + name_offset as usize;
                let len = *slice(map, position, 1)?.first().unwrap() as usize;
                Some(slice(map, position + 1, len)?)
            };
            resources.push(Resource {
                kind,
                id,
                name,
                attributes: (packed >> 24) as u8,
                data: slice(data, offset + 4, len)?,
                map,
                reference_offset: r,
            });
        }
    }
    Ok(resources)
}
