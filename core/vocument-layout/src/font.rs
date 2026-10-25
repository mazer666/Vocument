//! OpenType font parser, metrics extraction, and deterministic font registry (`ADR-0020`, `AUTHOR-004`).

use std::collections::HashMap;
use std::sync::Arc;

use crate::error::LayoutError;
use crate::geometry::FixedPt;

/// Calibrated OpenType font metrics and glyph advance tables.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontMetrics {
    pub units_per_em: u16,
    pub ascender: i16,
    pub descender: i16,
    pub line_gap: i16,
    pub cap_height: i16,
    pub x_height: i16,
}

impl Default for FontMetrics {
    fn default() -> Self {
        Self {
            units_per_em: 2048,
            ascender: 1900,
            descender: -500,
            line_gap: 0,
            cap_height: 1450,
            x_height: 1050,
        }
    }
}

/// A parsed font instance: either backed by raw OpenType TTF bytes or calibrated metrics.
pub struct Font {
    pub family: String,
    pub is_bold: bool,
    pub is_italic: bool,
    pub metrics: FontMetrics,
    raw_data: Option<Vec<u8>>,
    glyph_widths_cache: HashMap<char, u16>,
}

impl PartialEq for Font {
    fn eq(&self, other: &Self) -> bool {
        self.family == other.family
            && self.is_bold == other.is_bold
            && self.is_italic == other.is_italic
            && self.metrics == other.metrics
    }
}

impl std::fmt::Debug for Font {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Font")
            .field("family", &self.family)
            .field("is_bold", &self.is_bold)
            .field("is_italic", &self.is_italic)
            .field("metrics", &self.metrics)
            .finish()
    }
}

impl Font {
    /// Creates a calibrated standard typeface font instance.
    pub fn calibrated(family: impl Into<String>, is_bold: bool, is_italic: bool) -> Self {
        let fam = family.into();
        let (metrics, default_cache) = Self::compute_calibrated_metrics(&fam);
        Self {
            family: fam,
            is_bold,
            is_italic,
            metrics,
            raw_data: None,
            glyph_widths_cache: default_cache,
        }
    }

    /// Parses an OpenType / TrueType font from raw file bytes.
    pub fn from_ttf_bytes(
        family: impl Into<String>,
        is_bold: bool,
        is_italic: bool,
        data: Vec<u8>,
    ) -> Result<Self, LayoutError> {
        let fam = family.into();
        let (metrics, glyph_widths_cache) = {
            let face = ttf_parser::Face::parse(&data, 0).map_err(|err| LayoutError::FontError {
                family: fam.clone(),
                details: format!("failed to parse OpenType font: {:?}", err),
            })?;

            let metrics = FontMetrics {
                units_per_em: face.units_per_em(),
                ascender: face.ascender(),
                descender: face.descender(),
                line_gap: face.line_gap(),
                cap_height: face.capital_height().unwrap_or(face.ascender() * 3 / 4),
                x_height: face.x_height().unwrap_or(face.ascender() / 2),
            };

            let mut cache = HashMap::new();
            for c in ' '..='~' {
                if let Some(glyph_id) = face.glyph_index(c) {
                    if let Some(adv) = face.glyph_hor_advance(glyph_id) {
                        cache.insert(c, adv);
                    }
                }
            }
            (metrics, cache)
        };

        let font = Self {
            family: fam,
            is_bold,
            is_italic,
            metrics,
            raw_data: Some(data),
            glyph_widths_cache,
        };

        Ok(font)
    }

    /// Computes the horizontal advance width in 26.6 fixed-point points.
    pub fn advance_width_fixed(&self, c: char, font_size_pt: f32) -> FixedPt {
        let upm = self.metrics.units_per_em as i64;
        let font_size_fixed = FixedPt::from_f32(font_size_pt).0 as i64;

        let font_units: i64 = if let Some(&adv) = self.glyph_widths_cache.get(&c) {
            adv as i64
        } else if let Some(raw) = &self.raw_data {
            if let Ok(face) = ttf_parser::Face::parse(raw, 0) {
                face.glyph_index(c)
                    .and_then(|gid| face.glyph_hor_advance(gid))
                    .map(|adv| adv as i64)
                    .unwrap_or_else(|| self.fallback_font_units_for_char(c))
            } else {
                self.fallback_font_units_for_char(c)
            }
        } else {
            self.fallback_font_units_for_char(c)
        };

        let fixed_val = (font_units * font_size_fixed) / upm;
        FixedPt(fixed_val as i32)
    }

    /// Computes kerning between two adjacent characters in 26.6 fixed-point points.
    pub fn kerning_fixed(&self, first: char, second: char, font_size_pt: f32) -> FixedPt {
        if let Some(raw) = &self.raw_data {
            if let Ok(face) = ttf_parser::Face::parse(raw, 0) {
                if let (Some(g1), Some(g2)) = (face.glyph_index(first), face.glyph_index(second)) {
                    if let Some(kern_table) = face.tables().kern {
                        for subtable in kern_table.subtables {
                            if let Some(k) = subtable.glyphs_kerning(g1, g2) {
                                let font_size_fixed = FixedPt::from_f32(font_size_pt).0 as i64;
                                let upm = self.metrics.units_per_em as i64;
                                let fixed_val = (k as i64 * font_size_fixed) / upm;
                                return FixedPt(fixed_val as i32);
                            }
                        }
                    }
                }
            }
        }
        FixedPt::ZERO
    }

    pub fn ascender_pt(&self, font_size_pt: f32) -> f32 {
        (self.metrics.ascender as f32 / self.metrics.units_per_em as f32) * font_size_pt
    }

    pub fn descender_pt(&self, font_size_pt: f32) -> f32 {
        (self.metrics.descender as f32 / self.metrics.units_per_em as f32) * font_size_pt
    }

    pub fn line_height_pt(&self, font_size_pt: f32, mult: f32) -> f32 {
        let base_height = ((self.metrics.ascender - self.metrics.descender + self.metrics.line_gap)
            as f32
            / self.metrics.units_per_em as f32)
            * font_size_pt;
        (base_height * mult).max(font_size_pt * 1.1)
    }

    fn fallback_font_units_for_char(&self, c: char) -> i64 {
        if self.family.to_lowercase().contains("courier")
            || self.family.to_lowercase().contains("mono")
        {
            return 1200; // Fixed pitch
        }
        match c {
            ' ' => 560,
            '.' | ',' | ':' | ';' | '!' | '\'' | '`' | '|' => 450,
            'i' | 'l' | 'j' | 'I' => 500,
            'f' | 't' | 'r' => 600,
            'm' | 'w' | 'M' | 'W' => 1700,
            '0'..='9' => 1100,
            'A'..='Z' => 1350,
            '—' | '–' => 1600,
            _ => 1050,
        }
    }

    fn compute_calibrated_metrics(family: &str) -> (FontMetrics, HashMap<char, u16>) {
        let lower = family.to_lowercase();
        let mut map = HashMap::new();

        if lower.contains("courier") || lower.contains("mono") {
            let m = FontMetrics {
                units_per_em: 2048,
                ascender: 1705,
                descender: -615,
                line_gap: 0,
                cap_height: 1200,
                x_height: 900,
            };
            for c in ' '..='~' {
                map.insert(c, 1228); // Standard monospaced pitch
            }
            (m, map)
        } else if lower.contains("times") || lower.contains("serif") {
            let m = FontMetrics {
                units_per_em: 2048,
                ascender: 1825,
                descender: -443,
                line_gap: 86,
                cap_height: 1356,
                x_height: 916,
            };
            // Calibrate core punctuation and digits
            map.insert(' ', 512);
            for d in '0'..='9' {
                map.insert(d, 1024);
            }
            map.insert('.', 512);
            map.insert(',', 512);
            map.insert('i', 570);
            map.insert('l', 570);
            map.insert('m', 1600);
            map.insert('w', 1450);
            (m, map)
        } else {
            // Standard Sans (Inter / Helvetica calibrated)
            let m = FontMetrics {
                units_per_em: 2048,
                ascender: 1900,
                descender: -500,
                line_gap: 0,
                cap_height: 1450,
                x_height: 1050,
            };
            map.insert(' ', 570);
            for d in '0'..='9' {
                map.insert(d, 1140);
            }
            map.insert('.', 500);
            map.insert(',', 500);
            map.insert('i', 480);
            map.insert('l', 480);
            map.insert('m', 1750);
            map.insert('w', 1500);
            (m, map)
        }
    }
}

/// Central registry managing font loading, resolution, and caching.
#[derive(Debug, Default)]
pub struct FontRegistry {
    fonts: HashMap<String, Arc<Font>>,
}

impl FontRegistry {
    pub fn new() -> Self {
        let mut reg = Self::default();
        // Pre-populate standard calibrated fonts
        reg.register_calibrated("Inter");
        reg.register_calibrated("Helvetica");
        reg.register_calibrated("Times");
        reg.register_calibrated("Times New Roman");
        reg.register_calibrated("Courier");
        reg
    }

    pub fn register_calibrated(&mut self, family: &str) {
        let key_regular = Self::font_key(family, false, false);
        let key_bold = Self::font_key(family, true, false);
        let key_italic = Self::font_key(family, false, true);
        let key_bold_italic = Self::font_key(family, true, true);

        self.fonts.insert(
            key_regular,
            Arc::new(Font::calibrated(family, false, false)),
        );
        self.fonts
            .insert(key_bold, Arc::new(Font::calibrated(family, true, false)));
        self.fonts
            .insert(key_italic, Arc::new(Font::calibrated(family, false, true)));
        self.fonts.insert(
            key_bold_italic,
            Arc::new(Font::calibrated(family, true, true)),
        );
    }

    pub fn register_ttf(
        &mut self,
        family: &str,
        is_bold: bool,
        is_italic: bool,
        data: Vec<u8>,
    ) -> Result<(), LayoutError> {
        let font = Font::from_ttf_bytes(family, is_bold, is_italic, data)?;
        let key = Self::font_key(family, is_bold, is_italic);
        self.fonts.insert(key, Arc::new(font));
        Ok(())
    }

    /// Resolves font for a family and style, falling back deterministically if unlisted.
    pub fn resolve(&self, family: &str, is_bold: bool, is_italic: bool) -> Arc<Font> {
        let key = Self::font_key(family, is_bold, is_italic);
        if let Some(font) = self.fonts.get(&key) {
            return Arc::clone(font);
        }
        let fallback_key = Self::font_key(family, false, false);
        if let Some(font) = self.fonts.get(&fallback_key) {
            return Arc::clone(font);
        }

        // Default fallback to Inter / Standard Sans
        let default_key = Self::font_key("Inter", is_bold, is_italic);
        if let Some(font) = self.fonts.get(&default_key) {
            return Arc::clone(font);
        }

        Arc::new(Font::calibrated(family, is_bold, is_italic))
    }

    fn font_key(family: &str, is_bold: bool, is_italic: bool) -> String {
        format!("{}:{}:{}", family.to_lowercase(), is_bold, is_italic)
    }
}
