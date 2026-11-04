//! Font embedding, WinAnsi text encoding and sRGB output intent for the PDF/A-2b and PDF/UA-1
//! export (`ADR-0020`, `ROADMAP.md` M4.4).
//!
//! ISO 19005-2 (6.2.11) and ISO 14289-1 (7.21) require every font that is used for rendering to
//! be embedded and the `/Widths` array to agree with the embedded font program. The export
//! therefore embeds the bundled Lato Regular (SIL OFL-1.1, unmodified) as a TrueType font
//! program and derives every width straight from that program.

use std::fmt::Write as FmtWrite;
use std::io::Write as IoWrite;

use super::PdfWriter;

/// Bundled Lato Regular font program (SIL OFL-1.1, embedded unmodified so the Reserved Font Name
/// stays valid).
const LATO_REGULAR_BYTES: &[u8] = include_bytes!("../../assets/lato_regular.ttf");

/// Bundled sRGB ICC profile used as the PDF/A output intent.
const SRGB_ICC_PROFILE_BYTES: &[u8] = include_bytes!("../../assets/srgb.icc");

/// PostScript name of the embedded font program, used as `/BaseFont` and `/FontName`.
const EMBEDDED_FONT_POSTSCRIPT_NAME: &str = "Lato-Regular";

/// First and last character code of the simple font's `/Widths` array.
const FIRST_CHARACTER_CODE: u8 = 32;
const LAST_CHARACTER_CODE: u8 = 255;

/// Unicode value of every WinAnsi (Windows-1252) code in `0x80..=0x9F`; `None` marks codes the
/// encoding leaves undefined.
const WIN_ANSI_HIGH_CONTROL_RANGE: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

/// Unicode character a WinAnsi character code stands for, or `None` for undefined codes.
pub(super) fn win_ansi_code_to_char(code: u8) -> Option<char> {
    match code {
        0x20..=0x7E | 0xA0..=0xFF => Some(char::from(code)),
        0x80..=0x9F => WIN_ANSI_HIGH_CONTROL_RANGE[usize::from(code - 0x80)],
        _ => None,
    }
}

/// WinAnsi character code of a Unicode character, or `None` when the encoding cannot represent it.
pub(super) fn char_to_win_ansi_code(character: char) -> Option<u8> {
    let scalar = character as u32;
    if (0x20..=0x7E).contains(&scalar) || (0xA0..=0xFF).contains(&scalar) {
        return u8::try_from(scalar).ok();
    }
    (0x80u8..=0x9F).find(|code| win_ansi_code_to_char(*code) == Some(character))
}

/// Returns `true` if `text` contains any Unicode scalar not directly representable in WinAnsi.
pub(super) fn has_non_win_ansi(text: &str) -> bool {
    text.chars().any(|c| char_to_win_ansi_code(c).is_none())
}

/// Encodes `text` as a UTF-16BE hexadecimal PDF string with BOM (`<FEFF...>`).
pub(super) fn encode_utf16be_hex(text: &str) -> String {
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(hex, "{:04X}", unit);
    }
    hex.push('>');
    hex
}

/// Returns a clean, legible visual transliteration/representation for Unicode characters outside WinAnsi,
/// preventing unsightly '?' glyphs in rendered PDF views (`ROADMAP.md` M4.4).
pub(super) fn visual_unicode_fallback(c: char) -> &'static str {
    match c {
        // Polish & Central European diacritics
        'Ą' => "A",
        'ą' => "a",
        'Ć' => "C",
        'ć' => "c",
        'Č' => "C",
        'č' => "c",
        'Ď' => "D",
        'ď' => "d",
        'Ę' => "E",
        'ę' => "e",
        'Ě' => "E",
        'ě' => "e",
        'Ł' => "L",
        'ł' => "l",
        'Ń' => "N",
        'ń' => "n",
        'Ň' => "N",
        'ň' => "n",
        'Ř' => "R",
        'ř' => "r",
        'Ś' => "S",
        'ś' => "s",
        'Ť' => "T",
        'ť' => "t",
        'Ů' => "U",
        'ů' => "u",
        'Ź' => "Z",
        'ź' => "z",
        'Ż' => "Z",
        'ż' => "z",
        'Ő' | 'Ö' => "O",
        'ő' => "o",
        'Ű' | 'Ü' => "U",
        'ű' => "u",
        'Đ' => "D",
        'đ' => "d",
        'ı' => "i",
        'ā' | 'ă' => "a",
        'Ā' | 'Ă' => "A",
        'ē' | 'ĕ' | 'ė' => "e",
        'Ē' | 'Ĕ' | 'Ė' => "E",
        'ī' | 'ĭ' | 'į' => "i",
        'Ī' | 'Ĭ' | 'Į' => "I",
        'ō' | 'ŏ' => "o",
        'Ō' | 'Ŏ' => "O",
        'ū' | 'ŭ' | 'ų' => "u",
        'Ū' | 'Ŭ' | 'Ų' => "U",
        'ğ' => "g",
        'Ğ' => "G",
        'ş' => "s",
        'Ş' => "S",
        'ţ' => "t",
        'Ţ' => "T",
        // Mathematical symbols & relations
        '≤' => "<=",
        '≥' => ">=",
        '≠' => "!=",
        '≈' => "~",
        '−' => "-",
        '×' => "x",
        '÷' => "/",
        '±' => "+/-",
        '∞' => "inf",
        '∑' => "SUM",
        '∏' => "PROD",
        '√' => "sqrt",
        '∫' => "int",
        '→' => "->",
        '←' => "<-",
        '↔' => "<->",
        '⇒' => "=>",
        '•' => "*",
        // Greek letters
        'α' => "a",
        'β' => "b",
        'γ' => "g",
        'δ' => "d",
        'ε' => "e",
        'ζ' => "z",
        'η' => "e",
        'θ' => "th",
        'ι' => "i",
        'κ' => "k",
        'λ' => "l",
        'μ' => "m",
        'ν' => "n",
        'ξ' => "x",
        'ο' => "o",
        'π' => "pi",
        'ρ' => "r",
        'σ' | 'ς' => "s",
        'τ' => "t",
        'υ' => "u",
        'φ' => "ph",
        'χ' => "ch",
        'ψ' => "ps",
        'ω' => "o",
        'Α' => "A",
        'Β' => "B",
        'Γ' => "G",
        'Δ' => "D",
        'Ε' => "E",
        'Ζ' => "Z",
        'Η' => "E",
        'Θ' => "TH",
        'Ι' => "I",
        'Κ' => "K",
        'Λ' => "L",
        'Μ' => "M",
        'Ν' => "N",
        'Ξ' => "X",
        'Ο' => "O",
        'Π' => "PI",
        'Ρ' => "R",
        'Σ' => "S",
        'Τ' => "T",
        'Υ' => "U",
        'Φ' => "PH",
        'Χ' => "CH",
        'Ψ' => "PS",
        'Ω' => "O",
        // Cyrillic letters
        'Д' => "D",
        'д' => "d",
        'Ж' => "Zh",
        'ж' => "zh",
        'Г' => "G",
        'г' => "g",
        'П' => "P",
        'п' => "p",
        'Ф' => "F",
        'ф' => "f",
        'И' => "I",
        'и' => "i",
        'Й' => "J",
        'й' => "j",
        'Л' => "L",
        'л' => "l",
        'Ц' => "Ts",
        'ц' => "ts",
        'Ч' => "Ch",
        'ч' => "ch",
        'Ш' => "Sh",
        'ш' => "sh",
        'Щ' => "Shch",
        'щ' => "shch",
        'Ы' => "Y",
        'ы' => "y",
        'Э' => "E",
        'э' => "e",
        'Ю' => "Yu",
        'ю' => "yu",
        'Я' => "Ya",
        'я' => "ya",
        // CJK / Ideographs
        '漢' => "[Kan]",
        '字' => "[ji]",
        // Punctuation & spaces
        '\u{2009}' | '\u{200A}' | '\u{202F}' => " ",
        '\u{2010}' | '\u{2011}' | '\u{2012}' => "-",
        _ => " ",
    }
}

/// Encodes text as a PDF literal string in WinAnsi. Characters WinAnsi cannot represent
/// are visually mapped to legible representations rather than replaced with '?'.
pub(super) fn encode_win_ansi_literal(text: &str) -> String {
    let mut literal = String::with_capacity(text.len() * 2);
    for character in text.chars() {
        if let Some(code) = char_to_win_ansi_code(character) {
            encode_single_byte_to_literal(code, &mut literal);
        } else {
            let fallback = visual_unicode_fallback(character);
            for fb_char in fallback.chars() {
                let code = char_to_win_ansi_code(fb_char).unwrap_or(b' ');
                encode_single_byte_to_literal(code, &mut literal);
            }
        }
    }
    literal
}

pub(super) fn encode_single_byte_to_literal(code: u8, literal: &mut String) {
    match code {
        b'(' | b')' | b'\\' => {
            literal.push('\\');
            literal.push(char::from(code));
        }
        0x20..=0x7E => literal.push(char::from(code)),
        _ => {
            let _ = write!(literal, "\\{:03o}", code);
        }
    }
}

/// Encodes text as a PDF text string for values that are not content-stream text, such as
/// annotation contents and authors. Printable ASCII stays a plain literal string (identical in
/// PDFDocEncoding); anything else becomes UTF-16BE with byte-order mark in hexadecimal notation.
pub(super) fn encode_pdf_text_string(text: &str) -> String {
    if text
        .chars()
        .all(|character| matches!(character, ' '..='~' | '\n'))
    {
        let mut literal = String::with_capacity(text.len() + 2);
        literal.push('(');
        for character in text.chars() {
            match character {
                '(' | ')' | '\\' => {
                    literal.push('\\');
                    literal.push(character);
                }
                '\n' => literal.push_str("\\n"),
                other => literal.push(other),
            }
        }
        literal.push(')');
        return literal;
    }
    let mut hex = String::from("<FEFF");
    for unit in text.encode_utf16() {
        let _ = write!(hex, "{:04X}", unit);
    }
    hex.push('>');
    hex
}

/// Glyph metrics of the embedded font program.
#[derive(Debug)]
pub(super) struct EmbeddedFontMetrics {
    face: Option<ttf_parser::Face<'static>>,
}

impl EmbeddedFontMetrics {
    /// Parses the bundled font program. A parse failure is covered by a unit test; should it
    /// ever happen the metrics degrade to a fixed width instead of aborting the export.
    pub(super) fn load() -> Self {
        Self {
            face: ttf_parser::Face::parse(LATO_REGULAR_BYTES, 0).ok(),
        }
    }

    /// Advance width of a WinAnsi code in thousandths of the font size (PDF glyph space).
    pub(super) fn width_for_code(&self, code: u8) -> u32 {
        let Some(face) = &self.face else {
            return 600;
        };
        let Some(character) = win_ansi_code_to_char(code) else {
            return 0;
        };
        let units_per_em = u32::from(face.units_per_em().max(1));
        face.glyph_index(character)
            .and_then(|glyph| face.glyph_hor_advance(glyph))
            .map(|advance| (u32::from(advance) * 1000 + units_per_em / 2) / units_per_em)
            .unwrap_or(0)
    }

    /// Width of a text run in points at the given font size, measured with the embedded font.
    pub(super) fn text_width_pt(&self, text: &str, font_size_pt: f32) -> f32 {
        let thousandths: u32 = text
            .chars()
            .map(|character| {
                if let Some(code) = char_to_win_ansi_code(character) {
                    self.width_for_code(code)
                } else {
                    let fallback = visual_unicode_fallback(character);
                    fallback
                        .chars()
                        .map(|c| self.width_for_code(char_to_win_ansi_code(c).unwrap_or(b' ')))
                        .sum()
                }
            })
            .sum();
        thousandths as f32 * font_size_pt / 1000.0
    }

    fn scaled(&self, value: i16) -> i32 {
        match &self.face {
            Some(face) => {
                let units_per_em = i32::from(face.units_per_em().max(1));
                (i32::from(value) * 1000) / units_per_em
            }
            None => i32::from(value),
        }
    }

    fn bounding_box(&self) -> [i32; 4] {
        match &self.face {
            Some(face) => {
                let bounds = face.global_bounding_box();
                [
                    self.scaled(bounds.x_min),
                    self.scaled(bounds.y_min),
                    self.scaled(bounds.x_max),
                    self.scaled(bounds.y_max),
                ]
            }
            None => [-166, -225, 1000, 931],
        }
    }

    fn ascent(&self) -> i32 {
        self.face
            .as_ref()
            .map_or(718, |face| self.scaled(face.ascender()))
    }

    fn descent(&self) -> i32 {
        self.face
            .as_ref()
            .map_or(-207, |face| self.scaled(face.descender()))
    }

    fn cap_height(&self) -> i32 {
        self.face
            .as_ref()
            .and_then(|face| face.capital_height())
            .map_or(718, |height| self.scaled(height))
    }
}

impl PdfWriter {
    /// Writes the embedded font: font dictionary, descriptor, font program and `/ToUnicode` CMap.
    pub(super) fn write_embedded_font(
        &mut self,
        font_ids: &EmbeddedFontObjectIds,
        metrics: &EmbeddedFontMetrics,
    ) {
        self.write_to_unicode_cmap(font_ids.to_unicode);

        self.begin_obj(font_ids.font);
        let widths = (FIRST_CHARACTER_CODE..=LAST_CHARACTER_CODE)
            .map(|code| metrics.width_for_code(code).to_string())
            .collect::<Vec<_>>()
            .join(" ");
        let _ = writeln!(
            self.buffer,
            "<< /Type /Font /Subtype /TrueType /BaseFont /{} /Encoding /WinAnsiEncoding /FirstChar {} /LastChar {} /Widths [ {} ] /FontDescriptor {} 0 R /ToUnicode {} 0 R >>",
            EMBEDDED_FONT_POSTSCRIPT_NAME,
            FIRST_CHARACTER_CODE,
            LAST_CHARACTER_CODE,
            widths,
            font_ids.descriptor,
            font_ids.to_unicode
        );
        self.end_obj();

        self.begin_obj(font_ids.descriptor);
        let bounds = metrics.bounding_box();
        let _ = writeln!(
            self.buffer,
            "<< /Type /FontDescriptor /FontName /{} /Flags 32 /FontBBox [ {} {} {} {} ] /Ascent {} /Descent {} /CapHeight {} /ItalicAngle 0 /StemV 80 /FontFile2 {} 0 R >>",
            EMBEDDED_FONT_POSTSCRIPT_NAME,
            bounds[0],
            bounds[1],
            bounds[2],
            bounds[3],
            metrics.ascent(),
            metrics.descent(),
            metrics.cap_height(),
            font_ids.font_program
        );
        self.end_obj();

        self.begin_obj(font_ids.font_program);
        let _ = write!(
            self.buffer,
            "<< /Length {} /Length1 {} >>\nstream\n",
            LATO_REGULAR_BYTES.len(),
            LATO_REGULAR_BYTES.len()
        );
        self.buffer.extend_from_slice(LATO_REGULAR_BYTES);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();
    }

    /// Writes a `/ToUnicode` CMap that maps every defined WinAnsi code to its Unicode character.
    fn write_to_unicode_cmap(&mut self, id: usize) {
        const MAPPINGS_PER_BLOCK: usize = 100;
        let mappings: Vec<(u8, char)> = (FIRST_CHARACTER_CODE..=LAST_CHARACTER_CODE)
            .filter_map(|code| win_ansi_code_to_char(code).map(|character| (code, character)))
            .collect();

        let mut cmap = String::from(
            "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n/CMapName /Vocument-ToUnicode def\n/CMapType 2 def\n1 begincodespacerange\n<00> <FF>\nendcodespacerange\n",
        );
        for block in mappings.chunks(MAPPINGS_PER_BLOCK) {
            let _ = writeln!(cmap, "{} beginbfchar", block.len());
            for (code, character) in block {
                let mut utf16_buffer = [0u16; 2];
                let units: String = character
                    .encode_utf16(&mut utf16_buffer)
                    .iter()
                    .map(|unit| format!("{:04X}", unit))
                    .collect();
                let _ = writeln!(cmap, "<{:02X}> <{}>", code, units);
            }
            cmap.push_str("endbfchar\n");
        }
        cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
        self.write_stream_object(id, cmap.as_bytes());
    }

    /// Writes the PDF/A output intent together with the embedded sRGB ICC profile.
    pub(super) fn write_output_intent(&mut self, output_intent_id: usize, icc_profile_id: usize) {
        self.begin_obj(icc_profile_id);
        let _ = write!(
            self.buffer,
            "<< /N 3 /Length {} >>\nstream\n",
            SRGB_ICC_PROFILE_BYTES.len()
        );
        self.buffer.extend_from_slice(SRGB_ICC_PROFILE_BYTES);
        self.buffer.extend_from_slice(b"\nendstream\n");
        self.end_obj();

        self.begin_obj(output_intent_id);
        let _ = writeln!(
            self.buffer,
            "<< /Type /OutputIntent /S /GTS_PDFA1 /OutputConditionIdentifier (sRGB IEC61966-2.1) /RegistryName (http://www.color.org) /Info (sRGB IEC61966-2.1) /DestOutputProfile {} 0 R >>",
            icc_profile_id
        );
        self.end_obj();
    }
}

/// PDF object numbers of the objects that make up the embedded font.
#[derive(Debug, Clone, Copy)]
pub(super) struct EmbeddedFontObjectIds {
    pub(super) font: usize,
    pub(super) descriptor: usize,
    pub(super) font_program: usize,
    pub(super) to_unicode: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_font_program_parses_and_covers_german_text() {
        let metrics = EmbeddedFontMetrics::load();
        assert!(metrics.face.is_some(), "bundled Lato must parse");
        for character in "ÄÖÜäöüß§€„“”–—•".chars() {
            let code = char_to_win_ansi_code(character).expect("representable in WinAnsi");
            assert!(
                metrics.width_for_code(code) > 0,
                "{character} needs a glyph"
            );
        }
    }

    #[test]
    fn win_ansi_roundtrips_every_defined_code() {
        for code in FIRST_CHARACTER_CODE..=LAST_CHARACTER_CODE {
            if let Some(character) = win_ansi_code_to_char(code) {
                assert_eq!(char_to_win_ansi_code(character), Some(code));
            }
        }
        assert_eq!(char_to_win_ansi_code('漢'), None);
    }

    #[test]
    fn literal_encoding_escapes_delimiters_and_high_codes() {
        assert_eq!(encode_win_ansi_literal("a(b)\\"), "a\\(b\\)\\\\");
        assert_eq!(encode_win_ansi_literal("ä§"), "\\344\\247");
        assert_eq!(encode_win_ansi_literal("漢"), "[Kan]");
        assert!(!encode_win_ansi_literal("Zażółć gęślą jaźń ≤ ≠ π").contains('?'));
    }

    #[test]
    fn pdf_text_string_is_literal_for_ascii_and_utf16_otherwise() {
        assert_eq!(encode_pdf_text_string("A (b)"), "(A \\(b\\))");
        assert_eq!(encode_pdf_text_string("Aä"), "<FEFF004100E4>");
    }
}
