//! WordprocessingML property parsing shared by `styles.xml` and `document.xml`.

use vocument_flow::styles::{Alignment, ColorRgba};

use crate::util::{parse_hex_color, twips_to_pt};
use crate::xml::Element;

/// Transitional WordprocessingML namespace (what Word writes).
pub const W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
/// ISO strict WordprocessingML namespace.
pub const W_STRICT: &str = "http://purl.oclc.org/ooxml/wordprocessingml/main";

/// True for a WordprocessingML element of the given local name, in either namespace variant.
pub fn is_w(element: &Element, local: &str) -> bool {
    element.local == local && (element.namespace == W || element.namespace == W_STRICT)
}

/// First WordprocessingML child with the given local name.
pub fn child_w<'a>(element: &'a Element, local: &str) -> Option<&'a Element> {
    element.child_elements().find(|child| is_w(child, local))
}

/// The `w:val` attribute (namespaced in transitional, bare in some strict producers).
pub fn val(element: &Element) -> Option<&str> {
    element.attribute_any_namespace("val")
}

/// A `w:*` attribute by local name.
pub fn attr<'a>(element: &'a Element, local: &str) -> Option<&'a str> {
    element.attribute_any_namespace(local)
}

/// Interprets an on/off element (`w:b`, `w:i`, …): present without value is true.
pub fn on_off(element: &Element) -> bool {
    !matches!(val(element), Some("0" | "false" | "off" | "none"))
}

fn parse_i64(text: Option<&str>) -> Option<i64> {
    text.and_then(|value| value.trim().parse::<i64>().ok())
}

/// Named highlight colours of `w:highlight`.
fn highlight_color(name: &str) -> Option<ColorRgba> {
    let hex = match name {
        "yellow" => "FFFF00",
        "green" => "00FF00",
        "cyan" => "00FFFF",
        "magenta" => "FF00FF",
        "blue" => "0000FF",
        "red" => "FF0000",
        "darkBlue" => "000080",
        "darkCyan" => "008080",
        "darkGreen" => "008000",
        "darkMagenta" => "800080",
        "darkRed" => "800000",
        "darkYellow" => "808000",
        "darkGray" => "808080",
        "lightGray" => "C0C0C0",
        "black" => "000000",
        "white" => "FFFFFF",
        _ => return None,
    };
    parse_hex_color(hex)
}

/// Run (character) properties. `None` means "not specified here" so layers can be stacked.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RunProps {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub subscript: Option<bool>,
    pub superscript: Option<bool>,
    pub size_pt: Option<f32>,
    pub family: Option<String>,
    /// `Some(None)` is the explicit colour `auto`.
    pub color: Option<Option<ColorRgba>>,
    pub background: Option<ColorRgba>,
    pub caps: Option<bool>,
    pub small_caps: Option<bool>,
    pub hidden: Option<bool>,
    pub character_style: Option<String>,
    /// Number of child elements that were not interpreted (spacing, kerning, effects, …).
    pub uninterpreted: u32,
}

impl RunProps {
    /// Parses a `w:rPr` element.
    pub fn from_element(properties: &Element) -> Self {
        let mut run = Self::default();
        for child in properties.child_elements() {
            match child.local.as_str() {
                "b" => run.bold = Some(on_off(child)),
                "i" => run.italic = Some(on_off(child)),
                "u" => run.underline = Some(val(child).is_none_or(|value| value != "none")),
                "strike" | "dstrike" => run.strike = Some(on_off(child)),
                "vertAlign" => match val(child) {
                    Some("superscript") => {
                        run.superscript = Some(true);
                        run.subscript = Some(false);
                    }
                    Some("subscript") => {
                        run.subscript = Some(true);
                        run.superscript = Some(false);
                    }
                    _ => {
                        run.superscript = Some(false);
                        run.subscript = Some(false);
                    }
                },
                "sz" => {
                    run.size_pt = parse_i64(val(child))
                        .filter(|half_points| *half_points > 0)
                        .map(|half_points| half_points as f32 / 2.0);
                }
                "rFonts" => {
                    run.family = attr(child, "ascii")
                        .or_else(|| attr(child, "hAnsi"))
                        .or_else(|| attr(child, "cs"))
                        .or_else(|| attr(child, "eastAsia"))
                        .map(str::to_string);
                }
                "color" => {
                    run.color = Some(val(child).and_then(parse_hex_color));
                }
                "highlight" => {
                    if let Some(color) = val(child).and_then(highlight_color) {
                        run.background = Some(color);
                    }
                }
                "shd" => {
                    if run.background.is_none() {
                        run.background = attr(child, "fill").and_then(parse_hex_color);
                    }
                }
                "caps" => run.caps = Some(on_off(child)),
                "smallCaps" => run.small_caps = Some(on_off(child)),
                "vanish" => run.hidden = Some(on_off(child)),
                "rStyle" => run.character_style = val(child).map(str::to_string),
                // Structure or revision markers, handled by the run converter itself.
                "ins" | "del" | "rPrChange" | "moveFrom" | "moveTo" => {}
                _ => run.uninterpreted += 1,
            }
        }
        run
    }

    /// Overlays `upper` on `self`: every property `upper` specifies wins.
    pub fn overlay(&mut self, upper: &RunProps) {
        macro_rules! take {
            ($($field:ident),*) => {
                $(if upper.$field.is_some() { self.$field = upper.$field.clone(); })*
            };
        }
        take!(
            bold,
            italic,
            underline,
            strike,
            subscript,
            superscript,
            size_pt,
            family,
            color,
            background,
            caps,
            small_caps,
            hidden
        );
        self.uninterpreted += upper.uninterpreted;
    }
}

/// Paragraph properties; `None` means "not specified here".
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParagraphProps {
    pub style_id: Option<String>,
    pub alignment: Option<Alignment>,
    pub space_before_pt: Option<f32>,
    pub space_after_pt: Option<f32>,
    /// Line spacing as a multiple of the font's natural line height.
    pub line_spacing_multiple: Option<f32>,
    pub first_line_indent_pt: Option<f32>,
    pub left_indent_pt: Option<f32>,
    pub right_indent_pt: Option<f32>,
    pub keep_with_next: Option<bool>,
    pub keep_lines_together: Option<bool>,
    pub widow_control: Option<bool>,
    pub page_break_before: Option<bool>,
    /// 0-based outline level; `9` means "body text".
    pub outline_level: Option<u8>,
    /// `(numId, ilvl)`.
    pub numbering: Option<(String, u8)>,
}

impl ParagraphProps {
    /// Parses a `w:pPr` element. Children that carry no mapped meaning are returned so the
    /// caller can preserve them as raw XML.
    pub fn from_element(properties: &Element) -> (Self, Vec<&Element>) {
        let mut paragraph = Self::default();
        let mut unhandled = Vec::new();
        for child in properties.child_elements() {
            match child.local.as_str() {
                "pStyle" => paragraph.style_id = val(child).map(str::to_string),
                "jc" => {
                    paragraph.alignment = match val(child) {
                        Some("left" | "start") => Some(Alignment::Left),
                        Some("center") => Some(Alignment::Center),
                        Some("right" | "end") => Some(Alignment::Right),
                        Some("both" | "distribute" | "thaiDistribute") => {
                            Some(Alignment::Justified)
                        }
                        _ => None,
                    }
                }
                "spacing" => {
                    paragraph.space_before_pt =
                        parse_i64(attr(child, "before")).map(|t| twips_to_pt(t.max(0)));
                    paragraph.space_after_pt =
                        parse_i64(attr(child, "after")).map(|t| twips_to_pt(t.max(0)));
                    if let Some(line) = parse_i64(attr(child, "line")).filter(|l| *l > 0) {
                        // `auto` is 240ths of a line; `exact`/`atLeast` are twips and are
                        // approximated against a 12 pt reference line.
                        paragraph.line_spacing_multiple = Some(match attr(child, "lineRule") {
                            Some("exact" | "atLeast") => (twips_to_pt(line) / 12.0).max(0.5),
                            _ => line as f32 / 240.0,
                        });
                    }
                }
                "ind" => {
                    paragraph.left_indent_pt =
                        parse_i64(attr(child, "left").or(attr(child, "start"))).map(twips_to_pt);
                    paragraph.right_indent_pt =
                        parse_i64(attr(child, "right").or(attr(child, "end"))).map(twips_to_pt);
                    let first_line = parse_i64(attr(child, "firstLine")).map(twips_to_pt);
                    let hanging = parse_i64(attr(child, "hanging")).map(twips_to_pt);
                    paragraph.first_line_indent_pt = match (first_line, hanging) {
                        (_, Some(hanging)) => Some(-hanging),
                        (first_line, None) => first_line,
                    };
                }
                "keepNext" => paragraph.keep_with_next = Some(on_off(child)),
                "keepLines" => paragraph.keep_lines_together = Some(on_off(child)),
                "widowControl" => paragraph.widow_control = Some(on_off(child)),
                "pageBreakBefore" => paragraph.page_break_before = Some(on_off(child)),
                "outlineLvl" => {
                    paragraph.outline_level = parse_i64(val(child))
                        .and_then(|level| u8::try_from(level).ok())
                        .filter(|level| *level <= 9);
                }
                "numPr" => {
                    let id = child_w(child, "numId").and_then(val).map(str::to_string);
                    let level = child_w(child, "ilvl")
                        .and_then(val)
                        .and_then(|level| level.parse::<u8>().ok())
                        .unwrap_or(0)
                        .min(8);
                    if let Some(id) = id {
                        paragraph.numbering = Some((id, level));
                    }
                }
                // Handled by the paragraph converter.
                "rPr" | "sectPr" | "pPrChange" => {}
                _ => unhandled.push(child),
            }
        }
        (paragraph, unhandled)
    }

    /// Overlays `upper` on `self`.
    pub fn overlay(&mut self, upper: &ParagraphProps) {
        macro_rules! take {
            ($($field:ident),*) => {
                $(if upper.$field.is_some() { self.$field = upper.$field.clone(); })*
            };
        }
        take!(
            alignment,
            space_before_pt,
            space_after_pt,
            line_spacing_multiple,
            first_line_indent_pt,
            left_indent_pt,
            right_indent_pt,
            keep_with_next,
            keep_lines_together,
            widow_control,
            page_break_before,
            outline_level,
            numbering
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::limits::OfficeLimits;
    use crate::xml::parse;

    fn element(inner: &str, tag: &str) -> Element {
        let xml = format!(r#"<w:{tag} xmlns:w="{W}">{inner}</w:{tag}>"#);
        parse(xml.as_bytes(), "t", &OfficeLimits::default()).expect("parse")
    }

    #[test]
    fn run_properties_cover_the_common_formatting() {
        let run = RunProps::from_element(&element(
            r#"<w:b/><w:i w:val="0"/><w:u w:val="single"/><w:strike/><w:vertAlign w:val="superscript"/>
               <w:sz w:val="22"/><w:rFonts w:ascii="Calibri"/><w:color w:val="FF0000"/>
               <w:highlight w:val="yellow"/><w:caps/><w:spacing w:val="20"/>"#,
            "rPr",
        ));
        assert_eq!(run.bold, Some(true));
        assert_eq!(run.italic, Some(false));
        assert_eq!(run.underline, Some(true));
        assert_eq!(run.superscript, Some(true));
        assert_eq!(run.size_pt, Some(11.0));
        assert_eq!(run.family.as_deref(), Some("Calibri"));
        assert_eq!(run.color, Some(parse_hex_color("FF0000")));
        assert_eq!(run.background, parse_hex_color("FFFF00"));
        assert_eq!(run.caps, Some(true));
        assert_eq!(run.uninterpreted, 1);
    }

    #[test]
    fn paragraph_properties_convert_twips_and_keep_unknown_children() {
        let el = element(
            r#"<w:pStyle w:val="Heading1"/><w:jc w:val="both"/>
               <w:spacing w:before="240" w:after="120" w:line="360" w:lineRule="auto"/>
               <w:ind w:left="720" w:hanging="360"/><w:numPr><w:ilvl w:val="1"/><w:numId w:val="3"/></w:numPr>
               <w:bidi/><w:tabs><w:tab w:val="left" w:pos="1"/></w:tabs>"#,
            "pPr",
        );
        let (paragraph, unhandled) = ParagraphProps::from_element(&el);
        assert_eq!(paragraph.style_id.as_deref(), Some("Heading1"));
        assert_eq!(paragraph.alignment, Some(Alignment::Justified));
        assert_eq!(paragraph.space_before_pt, Some(12.0));
        assert_eq!(paragraph.space_after_pt, Some(6.0));
        assert_eq!(paragraph.line_spacing_multiple, Some(1.5));
        assert_eq!(paragraph.left_indent_pt, Some(36.0));
        assert_eq!(paragraph.first_line_indent_pt, Some(-18.0));
        assert_eq!(paragraph.numbering, Some(("3".to_string(), 1)));
        assert_eq!(unhandled.len(), 2, "bidi and tabs are preserved as raw XML");
    }
}
