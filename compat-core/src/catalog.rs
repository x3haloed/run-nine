//! Classic directory identities and colon-path traversal, independent of host paths.
use alloc::{
    collections::{BTreeMap, BTreeSet},
    vec::Vec,
};
#[derive(Clone, Default)]
pub struct Metadata {
    pub finder: [u8; 16],
    pub extended: [u8; 16],
    pub dates: [u32; 3],
}
#[derive(Clone)]
pub struct Directory {
    pub parent: u32,
    pub name: Vec<u8>,
    pub metadata: Metadata,
}
pub struct Catalog {
    directories: BTreeMap<u32, Directory>,
    next: u32,
    objects: BTreeSet<u32>,
    root_metadata: Metadata,
}
pub struct Target {
    pub parent: u32,
    pub name: Vec<u8>,
    pub directory: Option<u32>,
    pub exists: bool,
}
impl Default for Catalog {
    fn default() -> Self {
        Self {
            directories: BTreeMap::new(),
            next: 3,
            objects: BTreeSet::new(),
            root_metadata: Default::default(),
        }
    }
}
impl Catalog {
    pub fn valid_name(name: &[u8]) -> Result<(), i16> {
        if name.is_empty() || name.len() > 31 || name.contains(&0) || name.contains(&b':') {
            Err(-37)
        } else {
            Ok(())
        }
    }
    pub fn contains(&self, id: u32) -> bool {
        id == 2 || self.directories.contains_key(&id)
    }
    pub fn advance_next_id(&mut self, next: u32) -> Result<(), i16> {
        if next < 3 {
            return Err(-50);
        }
        self.next = self.next.max(next);
        Ok(())
    }
    pub fn next_id(&self) -> u32 {
        self.next
    }
    pub fn entries(&self) -> impl Iterator<Item = (&u32, &Directory)> {
        self.directories.iter()
    }
    pub fn directory(&self, id: u32) -> Option<&Directory> {
        self.directories.get(&id)
    }
    pub fn metadata(&self, id: u32) -> Option<&Metadata> {
        if id == 2 {
            Some(&self.root_metadata)
        } else {
            self.directories.get(&id).map(|d| &d.metadata)
        }
    }
    pub fn metadata_mut(&mut self, id: u32) -> Option<&mut Metadata> {
        if id == 2 {
            Some(&mut self.root_metadata)
        } else {
            self.directories.get_mut(&id).map(|d| &mut d.metadata)
        }
    }
    pub fn child(&self, parent: u32, name: &[u8]) -> Option<(u32, &Directory)> {
        self.directories
            .iter()
            .find(|(_, d)| d.parent == parent && d.name.eq_ignore_ascii_case(name))
            .map(|(i, d)| (*i, d))
    }
    pub fn insert(&mut self, id: u32, parent: u32, name: Vec<u8>) -> Result<(), i16> {
        Self::valid_name(&name)?;
        if !self.contains(parent) {
            return Err(-120);
        }
        if id < 3 || self.objects.contains(&id) || self.child(parent, &name).is_some() {
            return Err(-48);
        }
        let next = id.checked_add(1).ok_or(-36i16)?;
        self.objects.insert(id);
        self.directories.insert(
            id,
            Directory {
                parent,
                name,
                metadata: Default::default(),
            },
        );
        self.next = self.next.max(next);
        Ok(())
    }
    pub fn file_identity(&mut self, requested: Option<u32>) -> Result<u32, i16> {
        let id = requested.unwrap_or(self.next);
        if id < 3 || self.objects.contains(&id) {
            return Err(-48);
        }
        let next = id.checked_add(1).ok_or(-36i16)?;
        self.objects.insert(id);
        self.next = self.next.max(next);
        Ok(id)
    }
    pub fn create(&mut self, parent: u32, name: Vec<u8>) -> Result<u32, i16> {
        let id = self.next;
        self.insert(id, parent, name)?;
        Ok(id)
    }
    /// Roll back a newly created empty catalog node after storage failure.
    pub fn remove_empty(&mut self, id: u32) -> Result<(), i16> {
        if self.directories.values().any(|d| d.parent == id) {
            return Err(-47);
        }
        self.directories.remove(&id).map(|_| ()).ok_or(-120)
    }
    /// `path` is relative to a resolved volume/directory. A leading colon is
    /// optional here: the File Manager adapter has already selected the volume.
    /// The file callback returns actual catalog spelling, never oracle output.
    pub fn resolve<F>(
        &self,
        start: u32,
        path: &[u8],
        volume_name: &[u8],
        mut file: F,
    ) -> Result<Target, i16>
    where
        F: FnMut(u32, &[u8]) -> Option<Vec<u8>>,
    {
        if !self.contains(start) {
            return Err(-120);
        }
        if path.contains(&0) {
            return Err(-37);
        }
        let mut parts: Vec<&[u8]> = path.split(|b| *b == b':').collect();
        if path.first() == Some(&b':') {
            parts.remove(0);
        }
        if parts.last() == Some(&&b""[..]) {
            parts.pop();
        }
        let mut current = start;
        if let Some(last) = parts.pop() {
            for component in parts {
                if component.is_empty() {
                    current = self.directory(current).ok_or(-120i16)?.parent;
                } else {
                    Self::valid_name(component)?;
                    current = self.child(current, component).ok_or(-120i16)?.0;
                }
            }
            if !last.is_empty() {
                Self::valid_name(last)?;
                if let Some((id, d)) = self.child(current, last) {
                    return Ok(Target {
                        parent: current,
                        name: d.name.clone(),
                        directory: Some(id),
                        exists: true,
                    });
                }
                let actual = file(current, last);
                let exists = actual.is_some();
                return Ok(Target {
                    parent: current,
                    name: actual.unwrap_or_else(|| last.to_vec()),
                    directory: None,
                    exists,
                });
            }
            current = self.directory(current).ok_or(-120i16)?.parent;
        }
        let (parent, name) = if current == 2 {
            (1, volume_name.to_vec())
        } else {
            let d = self.directory(current).ok_or(-120i16)?;
            (d.parent, d.name.clone())
        };
        Ok(Target {
            parent,
            name,
            directory: Some(current),
            exists: true,
        })
    }
}
