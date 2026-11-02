//! Classic font selection and integer advances. Glyph artwork is separate.
use std::collections::BTreeMap;
#[derive(Clone)]
pub struct Metrics {
    pub info: [i16; 4],
    pub widths: Vec<i16>,
}
pub struct Fonts {
    catalog: BTreeMap<(i16, i16, u8), Metrics>,
    availability: BTreeMap<(i16, i16), bool>,
    initialized: bool,
    rasters: Option<Rasters>,
    title_bounds: Vec<(i16, i16)>,
}
impl Fonts {
    pub fn new() -> Self {
        Self {
            catalog: BTreeMap::new(),
            availability: BTreeMap::new(),
            initialized: false,
            rasters: None,
            title_bounds: Vec::new(),
        }
    }
    /// Families supplied by this runtime, in menu order. Other native fonts
    /// are not advertised without an independent provider and metric profile.
    pub fn family_names(&self) -> [&'static [u8]; 4] {
        [b"Charcoal", b"Courier", b"Geneva", b"Monaco"]
    }
    pub fn family_name(&self, id: i16) -> &'static [u8] {
        match id {
            0 | 2002 => b"Charcoal",
            1 | 3 => b"Geneva",
            4 => b"Monaco",
            22 => b"Courier",
            _ => b"",
        }
    }
    pub fn family_number(&self, name: &[u8]) -> i16 {
        for (id, candidate) in [
            (2002, b"Charcoal".as_slice()),
            (22, b"Courier".as_slice()),
            (3, b"Geneva".as_slice()),
            (4, b"Monaco".as_slice()),
        ] {
            if name.eq_ignore_ascii_case(candidate) {
                return id;
            }
        }
        0
    }
    fn metrics_family(id: i16) -> i16 {
        match id {
            1 => 3,
            2002 => 0,
            _ => id,
        }
    }
    pub fn dispatch<B: m68k::AddressBus>(
        &self,
        op: u16,
        cpu: &mut m68k::CpuCore,
        m: &mut B,
    ) -> bool {
        let sp = cpu.dar[15];
        match op {
            0xa902 => {
                let size = m.read_word(sp) as i16;
                let family = m.read_word(sp + 2) as i16;
                // The OS 9 profile resolves the application alias here too.
                let family = Self::metrics_family(family);
                let available = self
                    .availability
                    .get(&(family, size))
                    .copied()
                    .unwrap_or_else(|| size > 0 && self.catalog.contains_key(&(family, size, 0)));
                m.write_byte(sp + 4, available as u8);
                cpu.dar[15] += 4;
            }
            0xa8ff => {
                let p = m.read_long(sp);
                let id = m.read_word(sp + 4) as i16;
                let name = self.family_name(id);
                m.write_byte(p, name.len() as u8);
                for (i, b) in name.iter().enumerate() {
                    m.write_byte(p + 1 + i as u32, *b);
                }
                cpu.dar[15] += 6;
            }
            0xa900 => {
                let output = m.read_long(sp);
                let p = m.read_long(sp + 4);
                let count = m.read_byte(p) as u32;
                let name: Vec<u8> = (0..count).map(|i| m.read_byte(p + 1 + i)).collect();
                m.write_word(output, self.family_number(&name) as u16);
                cpu.dar[15] += 8;
            }
            _ => return false,
        }
        cpu.pc += 2;
        true
    }
    pub fn initialize<B: m68k::AddressBus>(&mut self, m: &mut B) -> Result<(), String> {
        self.init()?;
        // Public Font Manager low-memory globals describe this font profile.
        m.write_word(0x984, self.family_number(b"Geneva") as u16);
        m.write_word(0xba6, self.family_number(b"Charcoal") as u16);
        m.write_word(0xba8, 12);
        Ok(())
    }
    pub fn init(&mut self) -> Result<(), String> {
        if self.initialized {
            return Ok(());
        }
        let data: serde_json::Value =
            serde_json::from_str(include_str!("../data/font-metrics.json"))
                .map_err(|e| e.to_string())?;
        let courier: serde_json::Value =
            serde_json::from_str(include_str!("../data/courier-metrics.json"))
                .map_err(|e| e.to_string())?;
        for r in data["records"]
            .as_array()
            .ok_or("invalid font catalog")?
            .iter()
            .chain(
                courier["fonts"]
                    .as_array()
                    .ok_or("invalid Courier catalog")?
                    .iter(),
            )
        {
            if let Some(real) = r["real"].as_i64() {
                self.availability.insert(
                    (
                        r["font"].as_i64().ok_or("missing availability family")? as i16,
                        r["size"].as_i64().ok_or("missing availability size")? as i16,
                    ),
                    real != 0,
                );
            }
            let info: Vec<i16> = r["info"]
                .as_array()
                .ok_or("missing font info")?
                .iter()
                .map(|v| v.as_i64().unwrap() as i16)
                .collect();
            let widths: Vec<i16> = r["widths"]
                .as_array()
                .ok_or("missing widths")?
                .iter()
                .map(|v| v.as_i64().unwrap() as i16)
                .collect();
            if widths.len() != 256 {
                return Err("invalid font advance count".into());
            }
            self.catalog.insert(
                (
                    r["font"].as_i64().unwrap() as i16,
                    r["size"].as_i64().unwrap() as i16,
                    r["face"].as_u64().unwrap() as u8,
                ),
                Metrics {
                    info: info.try_into().map_err(|_| "invalid font info count")?,
                    widths,
                },
            );
        }
        // The pinned native system-font profile has these additive style
        // metrics at12pt. All128 combinations and independent StringWidth
        // labels are measured by SystemStyleProbe; no glyph pixels are copied.
        let base = self
            .catalog
            .get(&(0, 12, 0))
            .ok_or("missing system-font baseline")?
            .clone();
        for face in 0..128u8 {
            let mut metrics = base.clone();
            if face & 0x18 != 0 {
                metrics.info[0] += 1;
            }
            metrics.info[1] += i16::from(face & 8 != 0) + 2 * i16::from(face & 16 != 0);
            if face & 32 != 0 {
                metrics.info[2] -= 1;
                for width in &mut metrics.widths {
                    *width = (*width - 1).max(0);
                }
            }
            if let Some(old) = self.catalog.get(&(0, 12, face)) {
                if old.info != metrics.info || old.widths != metrics.widths {
                    return Err("system-font style calibration conflicts with baseline".into());
                }
            }
            self.catalog.insert((0, 12, face), metrics);
        }
        let bounds: serde_json::Value =
            serde_json::from_str(include_str!("../data/title-font-bounds.json"))
                .map_err(|e| e.to_string())?;
        if bounds["font"] != 0
            || bounds["family"] != 2002
            || bounds["size"] != 12
            || bounds["face"] != 0
        {
            return Err("unsupported title-font profile".into());
        }
        self.title_bounds = bounds["bounds"]
            .as_array()
            .ok_or("missing title glyph bounds")?
            .iter()
            .map(|g| {
                let values = g.as_array().ok_or("invalid title glyph bounds")?;
                if values.len() != 2 {
                    return Err("invalid title glyph interval".into());
                }
                let left = i16::try_from(values[0].as_i64().ok_or("invalid glyph left")?)
                    .map_err(|_| "glyph bound overflow")?;
                let right = i16::try_from(values[1].as_i64().ok_or("invalid glyph right")?)
                    .map_err(|_| "glyph bound overflow")?;
                if left > right {
                    return Err("reversed title glyph interval".into());
                }
                Ok((left, right))
            })
            .collect::<Result<Vec<_>, String>>()?;
        if self.title_bounds.len() != 256 {
            return Err("invalid title glyph count".into());
        }
        let settings = fontdue::FontSettings::default();
        self.rasters = Some(Rasters {
            regular: fontdue::Font::from_bytes(
                include_bytes!("../fonts/NimbusSans-Regular.ttf") as &[u8],
                settings,
            )
            .map_err(|e| e.to_string())?,
            bold: fontdue::Font::from_bytes(
                include_bytes!("../fonts/NimbusSans-Bold.ttf") as &[u8],
                settings,
            )
            .map_err(|e| e.to_string())?,
            mono: fontdue::Font::from_bytes(
                include_bytes!("../fonts/NimbusMonoPS-Regular.ttf") as &[u8],
                settings,
            )
            .map_err(|e| e.to_string())?,
            coppet: fontdue::Font::from_bytes(
                include_bytes!("../fonts/Coppet-Regular.ttf") as &[u8],
                settings,
            )
            .map_err(|e| e.to_string())?,
        });
        self.initialized = true;
        Ok(())
    }
    pub fn select(&self, font: i16, size: i16, face: u8) -> Result<&Metrics, String> {
        if !self.initialized {
            return Err("Font Manager not initialized".into());
        }
        // Application font and size zero follow the classic system defaults.
        let font = Self::metrics_family(font);
        let size = if size == 0 { 12 } else { size };
        self.catalog
            .get(&(font, size, face))
            .ok_or_else(|| format!("unsupported font selection {font}/{size}/{face}"))
    }
    pub fn text_width(&self, font: i16, size: i16, face: u8, text: &[u8]) -> Result<i16, String> {
        let metrics = self.select(font, size, face)?;
        let total: i32 = text
            .iter()
            .map(|c| metrics.widths[*c as usize] as i32)
            .sum();
        // Classic Integer widths wrap at the API's 16-bit boundary.
        Ok(total as i16)
    }
    /// System-title extent differs from summed advances. Keep pen positions
    /// for blank glyphs as zero-width intervals, including trailing spaces.
    pub fn title_width(&self, text: &[u8]) -> Result<i16, String> {
        let metrics = self.select(0, 12, 0)?;
        let mut pen = 0i32;
        let mut extent: Option<(i32, i32)> = None;
        for &byte in text {
            let (left, right) = self.title_bounds[byte as usize];
            let span = (pen + left as i32, pen + right as i32);
            extent = Some(if let Some((a, b)) = extent {
                (a.min(span.0), b.max(span.1))
            } else {
                span
            });
            pen += metrics.widths[byte as usize] as i32;
        }
        Ok(extent
            .map(|(left, right)| (right - left) as i16)
            .unwrap_or(0))
    }
}

struct Rasters {
    regular: fontdue::Font,
    bold: fontdue::Font,
    mono: fontdue::Font,
    coppet: fontdue::Font,
}
pub struct Glyph {
    pub width: usize,
    pub height: usize,
    pub left: i16,
    pub top: i16,
    pub coverage: Vec<u8>,
    pub advance: i16,
}
impl Fonts {
    pub fn glyph(&self, font: i16, size: i16, face: u8, byte: u8) -> Result<Glyph, String> {
        let font = Self::metrics_family(font);
        let selected = self.select(font, size, face)?;
        if face & !1 != 0 {
            return Err("styled glyph artwork beyond plain/bold is not implemented".into());
        }
        let size = if size == 0 { 12 } else { size };
        let rasters = self.rasters.as_ref().ok_or("glyph fonts not initialized")?;
        let raster = if font == 4 || font == 22 {
            &rasters.mono
        } else if font == 0 || face & 1 != 0 {
            &rasters.bold
        } else if size == 9 {
            &rasters.coppet
        } else {
            &rasters.regular
        };
        let (metrics, coverage) =
            raster.rasterize(crate::mac_roman::MAC_ROMAN[byte as usize], size as f32);
        Ok(Glyph {
            width: metrics.width,
            height: metrics.height,
            left: metrics.xmin as i16,
            top: -(metrics.height as i16 + metrics.ymin as i16),
            coverage,
            advance: selected.widths[byte as usize],
        })
    }
}

#[cfg(test)]
mod alias_tests {
    use super::*;
    #[test]
    fn advertised_aliases_share_metric_and_raster_providers() {
        let mut fonts = Fonts::new();
        fonts.init().unwrap();
        for (alias, base) in [(2002, 0), (1, 3)] {
            for size in [0, 9, 12] {
                for face in [0, 1] {
                    assert_eq!(
                        fonts.select(alias, size, face).unwrap().info,
                        fonts.select(base, size, face).unwrap().info
                    );
                    assert_eq!(
                        fonts.select(alias, size, face).unwrap().widths,
                        fonts.select(base, size, face).unwrap().widths
                    );
                    for byte in [b'A', b'i', b' ', 0x8e] {
                        let a = fonts.glyph(alias, size, face, byte).unwrap();
                        let b = fonts.glyph(base, size, face, byte).unwrap();
                        assert_eq!(
                            (a.width, a.height, a.left, a.top, a.advance, a.coverage),
                            (b.width, b.height, b.left, b.top, b.advance, b.coverage)
                        );
                    }
                }
            }
        }
        for name in fonts.family_names() {
            let id = fonts.family_number(name);
            for size in [0, 9, 12] {
                fonts.glyph(id, size, 0, b'A').unwrap();
            }
        }
    }
}
