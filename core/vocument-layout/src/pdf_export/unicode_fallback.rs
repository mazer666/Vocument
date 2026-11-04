//! Scalable OpenType / TrueType Unicode Fallback Font embedding for Tagged PDF/A-2b and PDF/UA-1 (`M4.4`, `ADR-0020`).
//!
//! Replaces legacy synthetic Type 3 vector glyphs with genuine, embedded OpenType/TrueType fonts:
//! - Multi-script TrueType font (`DroidSansFallback.ttf`, Apache-2.0): CJK, Greek, Cyrillic, Math, Latin extended.
//! - Arabic OpenType font (`NotoNaskhArabic-Regular.otf`, SIL OFL-1.1): Arabic cursive script with contextual shaping.
//! - Type 0 CIDFont architecture (`/Subtype /Type0`, `/Encoding /Identity-H`, `/CIDFontType2` / `/CIDFontType0`).
//! - 100% accurate `/ToUnicode` CMaps mapping CIDs back to original Unicode scalars without character limits.
//! - FlateDecode compressed font programs and zero lossy transliterations.

use std::collections::BTreeMap;
use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

use miniz_oxide::deflate::compress_to_vec_zlib;

use super::PdfWriter;
use super::font_embedding::char_to_win_ansi_code;

/// Bundled Droid Sans Fallback TrueType font program (Apache-2.0).
pub const DROID_SANS_FALLBACK_BYTES: &[u8] = include_bytes!("../../assets/droid_sans_fallback.ttf");

/// Bundled Noto Naskh Arabic OpenType font program (SIL OFL-1.1).
pub const NOTO_NASKH_ARABIC_BYTES: &[u8] = include_bytes!("../../assets/noto_naskh_arabic.otf");

/// Resource name of the multi-script Unicode fallback font in page resource dictionaries.
pub const FALLBACK_UNICODE_FONT_RESOURCE_NAME: &str = "F2";

/// Alias for backwards compatibility.
pub const FALLBACK_FONT_RESOURCE_NAME: &str = FALLBACK_UNICODE_FONT_RESOURCE_NAME;

/// Resource name of the Arabic fallback font in page resource dictionaries.
pub const FALLBACK_ARABIC_FONT_RESOURCE_NAME: &str = "F3";

/// Target script family for a fallback character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallbackScript {
    Unicode,
    Arabic,
}

/// Fallback glyph registration metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FallbackGlyph {
    pub script: FallbackScript,
    pub gid: u16,
    pub width: u32,
}

/// Registry tracking unique fallback Unicode characters and their corresponding OpenType CIDs.
#[derive(Debug, Default, Clone)]
pub struct UnicodeFallbackRegistry {
    char_map: BTreeMap<char, FallbackGlyph>,
    droid_gids: BTreeMap<u16, (char, u32)>,
    arabic_gids: BTreeMap<u16, (char, u32)>,
}

impl UnicodeFallbackRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Checks if a character belongs to the Arabic Unicode blocks.
    pub fn is_arabic_char(c: char) -> bool {
        matches!(c,
            '\u{0600}'..='\u{06FF}' |
            '\u{0750}'..='\u{077F}' |
            '\u{08A0}'..='\u{08FF}' |
            '\u{FB50}'..='\u{FDFF}' |
            '\u{FE70}'..='\u{FEFF}'
        )
    }

    /// Registers a character if it is outside the base WinAnsi font. Returns its fallback glyph reference.
    pub fn register_char(&mut self, c: char) -> Option<FallbackGlyph> {
        if char_to_win_ansi_code(c).is_some() {
            return None;
        }
        if let Some(&glyph) = self.char_map.get(&c) {
            return Some(glyph);
        }

        if Self::is_arabic_char(c) {
            if let Ok(face) = ttf_parser::Face::parse(NOTO_NASKH_ARABIC_BYTES, 0) {
                if let Some(gid) = face.glyph_index(c) {
                    let upm = u32::from(face.units_per_em().max(1));
                    let adv = face.glyph_hor_advance(gid).unwrap_or(1000);
                    let width = (u32::from(adv) * 1000 + upm / 2) / upm;
                    let glyph = FallbackGlyph {
                        script: FallbackScript::Arabic,
                        gid: gid.0,
                        width,
                    };
                    self.char_map.insert(c, glyph);
                    self.arabic_gids.insert(gid.0, (c, width));
                    return Some(glyph);
                }
            }
        }

        if let Ok(face) = ttf_parser::Face::parse(DROID_SANS_FALLBACK_BYTES, 0) {
            if let Some(gid) = face.glyph_index(c) {
                let upm = u32::from(face.units_per_em().max(1));
                let adv = face.glyph_hor_advance(gid).unwrap_or(256);
                let width = (u32::from(adv) * 1000 + upm / 2) / upm;
                let glyph = FallbackGlyph {
                    script: FallbackScript::Unicode,
                    gid: gid.0,
                    width,
                };
                self.char_map.insert(c, glyph);
                self.droid_gids.insert(gid.0, (c, width));
                return Some(glyph);
            }
        }

        None
    }

    pub fn has_unicode_fallback(&self) -> bool {
        !self.droid_gids.is_empty()
    }

    pub fn has_arabic_fallback(&self) -> bool {
        !self.arabic_gids.is_empty()
    }

    pub fn is_empty(&self) -> bool {
        self.droid_gids.is_empty() && self.arabic_gids.is_empty()
    }

    pub fn len(&self) -> usize {
        self.char_map.len()
    }
}

/// Object IDs allocated for embedded fallback fonts.
#[derive(Debug, Clone, Copy, Default)]
pub struct FallbackFontObjectIds {
    pub unicode_font: Option<usize>,
    pub unicode_descendant: Option<usize>,
    pub unicode_descriptor: Option<usize>,
    pub unicode_file: Option<usize>,
    pub unicode_to_unicode: Option<usize>,

    pub arabic_font: Option<usize>,
    pub arabic_descendant: Option<usize>,
    pub arabic_descriptor: Option<usize>,
    pub arabic_file: Option<usize>,
    pub arabic_to_unicode: Option<usize>,
}

impl PdfWriter {
    /// Writes embedded OpenType/TrueType Type 0 fallback fonts and their `/ToUnicode` CMaps.
    pub fn write_fallback_fonts(
        &mut self,
        font_ids: &FallbackFontObjectIds,
        registry: &UnicodeFallbackRegistry,
    ) {
        if let (Some(font_id), Some(desc_id), Some(fd_id), Some(file_id), Some(to_u_id)) = (
            font_ids.unicode_font,
            font_ids.unicode_descendant,
            font_ids.unicode_descriptor,
            font_ids.unicode_file,
            font_ids.unicode_to_unicode,
        ) {
            self.write_truetype_cid_font(
                font_id,
                desc_id,
                fd_id,
                file_id,
                to_u_id,
                "DroidSansFallback",
                DROID_SANS_FALLBACK_BYTES,
                [-558, -265, 1121, 1042],
                1042,
                -265,
                750,
                &registry.droid_gids,
            );
        }

        if let (Some(font_id), Some(desc_id), Some(fd_id), Some(file_id), Some(to_u_id)) = (
            font_ids.arabic_font,
            font_ids.arabic_descendant,
            font_ids.arabic_descriptor,
            font_ids.arabic_file,
            font_ids.arabic_to_unicode,
        ) {
            self.write_cff_cid_font(
                font_id,
                desc_id,
                fd_id,
                file_id,
                to_u_id,
                "NotoNaskhArabic-Regular",
                NOTO_NASKH_ARABIC_BYTES,
                [-232, -610, 6092, 1056],
                1069,
                -634,
                750,
                &registry.arabic_gids,
            );
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn write_truetype_cid_font(
        &mut self,
        font_id: usize,
        descendant_id: usize,
        descriptor_id: usize,
        file_id: usize,
        to_unicode_id: usize,
        base_font_name: &str,
        raw_ttf_bytes: &[u8],
        bbox: [i32; 4],
        ascent: i32,
        descent: i32,
        cap_height: i32,
        gids: &BTreeMap<u16, (char, u32)>,
    ) {
        // 1. Embedded FontFile2 stream (compressed)
        let compressed = compress_to_vec_zlib(raw_ttf_bytes, 6);
        self.begin_obj(file_id);
        let _ = write!(
            self.buffer,
            "<< /Length {} /Length1 {} /Filter /FlateDecode >>\nstream\n",
            compressed.len(),
            raw_ttf_bytes.len()
        );
        self.buffer.extend_from_slice(&compressed);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();

        // 2. FontDescriptor dictionary
        self.begin_obj(descriptor_id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /FontDescriptor /FontName /{} /Flags 32 /FontBBox [ {} {} {} {} ] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {} 0 R >>",
            base_font_name,
            bbox[0],
            bbox[1],
            bbox[2],
            bbox[3],
            ascent,
            descent,
            cap_height,
            file_id
        );
        self.end_obj();

        // 3. /ToUnicode CMap
        self.write_cid_to_unicode_cmap(to_unicode_id, gids);

        // 4. Descendant CIDFontType2 dictionary
        self.begin_obj(descendant_id);
        let mut widths_str = String::new();
        for (&gid, &(_, width)) in gids {
            let _ = write!(widths_str, "{} [ {} ] ", gid, width);
        }
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /CIDToGIDMap /Identity /DW 1000 /W [ {}] >>",
            base_font_name, descriptor_id, widths_str
        );
        self.end_obj();

        // 5. Root Type 0 Font dictionary
        self.begin_obj(font_id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [ {} 0 R ] /ToUnicode {} 0 R >>",
            base_font_name, descendant_id, to_unicode_id
        );
        self.end_obj();
    }

    #[allow(clippy::too_many_arguments)]
    fn write_cff_cid_font(
        &mut self,
        font_id: usize,
        descendant_id: usize,
        descriptor_id: usize,
        file_id: usize,
        to_unicode_id: usize,
        base_font_name: &str,
        raw_otf_bytes: &[u8],
        bbox: [i32; 4],
        ascent: i32,
        descent: i32,
        cap_height: i32,
        gids: &BTreeMap<u16, (char, u32)>,
    ) {
        // 1. Embedded FontFile3 stream (compressed with /Subtype /OpenType)
        let compressed = compress_to_vec_zlib(raw_otf_bytes, 6);
        self.begin_obj(file_id);
        let _ = write!(
            self.buffer,
            "<< /Length {} /Subtype /OpenType /Filter /FlateDecode >>\nstream\n",
            compressed.len()
        );
        self.buffer.extend_from_slice(&compressed);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();

        // 2. FontDescriptor dictionary
        self.begin_obj(descriptor_id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /FontDescriptor /FontName /{} /Flags 4 /FontBBox [ {} {} {} {} ] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile3 {} 0 R >>",
            base_font_name,
            bbox[0],
            bbox[1],
            bbox[2],
            bbox[3],
            ascent,
            descent,
            cap_height,
            file_id
        );
        self.end_obj();

        // 3. /ToUnicode CMap
        self.write_cid_to_unicode_cmap(to_unicode_id, gids);

        // 4. Descendant CIDFontType0 dictionary
        self.begin_obj(descendant_id);
        let mut widths_str = String::new();
        for (&gid, &(_, width)) in gids {
            let _ = write!(widths_str, "{} [ {} ] ", gid, width);
        }
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /CIDFontType0 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {} 0 R /DW 1000 /W [ {}] >>",
            base_font_name, descriptor_id, widths_str
        );
        self.end_obj();

        // 5. Root Type 0 Font dictionary
        self.begin_obj(font_id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [ {} 0 R ] /ToUnicode {} 0 R >>",
            base_font_name, descendant_id, to_unicode_id
        );
        self.end_obj();
    }

    fn write_cid_to_unicode_cmap(
        &mut self,
        to_unicode_id: usize,
        gids: &BTreeMap<u16, (char, u32)>,
    ) {
        self.begin_obj(to_unicode_id);
        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Vocument-CIDToUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n",
        );

        let entries: Vec<(u16, char)> = gids.iter().map(|(&gid, &(ch, _))| (gid, ch)).collect();
        for chunk in entries.chunks(100) {
            let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
            for &(gid, ch) in chunk {
                let mut utf16_buf = [0u16; 2];
                let hex_units: String = ch
                    .encode_utf16(&mut utf16_buf)
                    .iter()
                    .map(|u| format!("{:04X}", u))
                    .collect();
                let _ = writeln!(cmap, "<{:04X}> <{}>", gid, hex_units);
            }
            cmap.push_str("endbfchar\n");
        }

        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        let cmap_bytes = cmap.into_bytes();
        let _ = write!(self.buffer, "<< /Length {} >>\nstream\n", cmap_bytes.len());
        self.buffer.extend_from_slice(&cmap_bytes);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }
}
