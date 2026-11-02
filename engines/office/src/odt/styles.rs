//! ODF 1.2/1.3 namespaces, `meta.xml`, and `styles.xml` / `office:automatic-styles` resolution.

use std::collections::{BTreeMap, HashMap, HashSet};

use vocument_flow::metadata::{Author, DocumentMetadata};
use vocument_flow::model::{ListKind, PageOrientation, SectionGeometry};
use vocument_flow::styles::{
    Alignment, CharacterStyle, ColorRgba, FontStyle, ParagraphStyle, Stylesheet,
};

use crate::error::OfficeError;
use crate::limits::OfficeLimits;
use crate::package::OfficePackage;
use crate::util::{parse_hex_color, parse_length_pt};
use crate::xml::{Element, parse};

pub const NS_OFFICE: &str = "urn:oasis:names:tc:opendocument:xmlns:office:1.0";
pub const NS_TEXT: &str = "urn:oasis:names:tc:opendocument:xmlns:text:1.0";
pub const NS_STYLE: &str = "urn:oasis:names:tc:opendocument:xmlns:style:1.0";
pub const NS_FO: &str = "urn:oasis:names:tc:opendocument:xmlns:xsl-fo-compatible:1.0";
pub const NS_TABLE: &str = "urn:oasis:names:tc:opendocument:xmlns:table:1.0";
pub const NS_DRAW: &str = "urn:oasis:names:tc:opendocument:xmlns:drawing:1.0";
pub const NS_XLINK: &str = "http://www.w3.org/1999/xlink";
pub const NS_DC: &str = "http://purl.org/dc/elements/1.1/";
pub const NS_META: &str = "urn:oasis:names:tc:opendocument:xmlns:meta:1.0";
pub const NS_SVG: &str = "urn:oasis:names:tc:opendocument:xmlns:svg-compatible:1.0";
pub const NS_LOEXT: &str = "urn:org:documentfoundation:names:experimental:office:xmlns:loext:1.0";
pub const NS_MANIFEST: &str = "urn:oasis:names:tc:opendocument:xmlns:manifest:1.0";

#[derive(Debug, Clone, Default)]
pub struct OdtTextProps {
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub superscript: Option<bool>,
    pub subscript: Option<bool>,
    pub size_pt: Option<f32>,
    pub family: Option<String>,
    pub color: Option<ColorRgba>,
    pub background: Option<ColorRgba>,
    pub hidden: bool,
}

impl OdtTextProps {
    pub fn from_element(el: &Element) -> Self {
        let mut p = Self::default();
        if let Some(fw) = el
            .attribute(NS_FO, "font-weight")
            .or_else(|| el.attribute(NS_STYLE, "font-weight-asian"))
        {
            p.bold = Some(fw == "bold" || fw.parse::<u16>().map(|n| n >= 600).unwrap_or(false));
        }
        if let Some(fs) = el
            .attribute(NS_FO, "font-style")
            .or_else(|| el.attribute(NS_STYLE, "font-style-asian"))
        {
            p.italic = Some(matches!(fs, "italic" | "oblique"));
        }
        if let Some(u) = el
            .attribute(NS_STYLE, "text-underline-style")
            .or_else(|| el.attribute(NS_STYLE, "text-underline-type"))
        {
            p.underline = Some(u != "none");
        }
        if let Some(s) = el
            .attribute(NS_STYLE, "text-line-through-style")
            .or_else(|| el.attribute(NS_STYLE, "text-line-through-type"))
        {
            p.strike = Some(s != "none");
        }
        if let Some(pos) = el.attribute(NS_STYLE, "text-position") {
            let first = pos.split_whitespace().next().unwrap_or("");
            if first.starts_with("super")
                || first
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map(|v| v > 0.0)
                    .unwrap_or(false)
            {
                p.superscript = Some(true);
            } else if first.starts_with("sub")
                || first
                    .trim_end_matches('%')
                    .parse::<f32>()
                    .map(|v| v < 0.0)
                    .unwrap_or(false)
            {
                p.subscript = Some(true);
            }
        }
        if let Some(sz) = el
            .attribute(NS_FO, "font-size")
            .and_then(parse_length_pt)
            .filter(|v| *v > 0.0)
        {
            p.size_pt = Some(sz.clamp(4.0, 200.0));
        }
        if let Some(fam) = el
            .attribute(NS_STYLE, "font-name")
            .or_else(|| el.attribute(NS_FO, "font-family"))
            .map(|s| s.trim_matches(['\'', '"']).trim())
            .filter(|s| !s.is_empty())
        {
            p.family = Some(fam.to_string());
        }
        if let Some(c) = el.attribute(NS_FO, "color").and_then(parse_hex_color) {
            p.color = Some(c);
        }
        if let Some(bg) = el
            .attribute(NS_FO, "background-color")
            .and_then(parse_hex_color)
        {
            p.background = Some(bg);
        }
        if el.attribute(NS_TEXT, "display") == Some("none") {
            p.hidden = true;
        }
        p
    }

    pub fn overlay(&mut self, child: &Self) {
        if child.bold.is_some() {
            self.bold = child.bold;
        }
        if child.italic.is_some() {
            self.italic = child.italic;
        }
        if child.underline.is_some() {
            self.underline = child.underline;
        }
        if child.strike.is_some() {
            self.strike = child.strike;
        }
        if child.superscript.is_some() || child.subscript.is_some() {
            self.superscript = child.superscript;
            self.subscript = child.subscript;
        }
        if child.size_pt.is_some() {
            self.size_pt = child.size_pt;
        }
        if child.family.is_some() {
            self.family.clone_from(&child.family);
        }
        if child.color.is_some() {
            self.color = child.color;
        }
        if child.background.is_some() {
            self.background = child.background;
        }
        if child.hidden {
            self.hidden = true;
        }
    }

    pub fn to_font_style(&self) -> FontStyle {
        FontStyle {
            bold: self.bold.unwrap_or(false),
            italic: self.italic.unwrap_or(false),
            underline: self.underline.unwrap_or(false),
            strikethrough: self.strike.unwrap_or(false),
            code: false,
            subscript: self.subscript.unwrap_or(false),
            superscript: self.superscript.unwrap_or(false),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct OdtParaStyle {
    pub name: String,
    pub display_name: Option<String>,
    pub parent: Option<String>,
    pub list_style_name: Option<String>,
    pub master_page_name: Option<String>,
    pub outline_level: Option<u8>,
    pub alignment: Option<Alignment>,
    pub space_before_pt: Option<f32>,
    pub space_after_pt: Option<f32>,
    pub line_spacing_mult: Option<f32>,
    pub first_line_indent_pt: Option<f32>,
    pub keep_with_next: Option<bool>,
    pub keep_together: Option<bool>,
    pub page_break_before: bool,
    pub text: OdtTextProps,
    pub is_automatic: bool,
}

#[derive(Debug, Clone)]
pub struct OdtListLevel {
    pub kind: ListKind,
    pub num_fmt: String,
}

#[derive(Debug, Clone)]
pub struct OdtStyles {
    pub stylesheet: Stylesheet,
    pub para_styles: HashMap<String, OdtParaStyle>,
    pub text_styles: HashMap<String, OdtTextProps>,
    pub list_styles: HashMap<String, HashMap<u8, OdtListLevel>>,
    pub column_widths: HashMap<String, f32>,
    pub cell_backgrounds: HashMap<String, ColorRgba>,
    pub page_layouts: HashMap<String, SectionGeometry>,
    pub default_geometry: SectionGeometry,
    pub master_headers: Vec<Element>,
    pub master_footers: Vec<Element>,
}

impl Default for OdtStyles {
    fn default() -> Self {
        Self {
            stylesheet: Stylesheet::standard_legal(),
            para_styles: HashMap::new(),
            text_styles: HashMap::new(),
            list_styles: HashMap::new(),
            column_widths: HashMap::new(),
            cell_backgrounds: HashMap::new(),
            page_layouts: HashMap::new(),
            default_geometry: SectionGeometry::a4_portrait(),
            master_headers: Vec::new(),
            master_footers: Vec::new(),
        }
    }
}

impl OdtStyles {
    pub fn parse_all(
        package: &OfficePackage,
        content_root: &Element,
        limits: &OfficeLimits,
    ) -> Result<Self, OfficeError> {
        let mut out = Self {
            stylesheet: Stylesheet::standard_legal(),
            default_geometry: SectionGeometry::a4_portrait(),
            ..Self::default()
        };

        if let Some(styles_bytes) = package.part("styles.xml") {
            let styles_root = parse(styles_bytes, "styles.xml", limits)?;
            out.collect_from_root(&styles_root, false);
        }
        out.collect_from_root(content_root, true);
        out.materialize_stylesheet();
        Ok(out)
    }

    fn collect_from_root(&mut self, root: &Element, is_content: bool) {
        for container in root.child_elements() {
            match container.local.as_str() {
                "styles" | "automatic-styles" if container.namespace == NS_OFFICE => {
                    let is_auto = is_content || container.local == "automatic-styles";
                    for child in container.child_elements() {
                        self.parse_style_node(child, is_auto);
                    }
                }
                "master-styles" if container.namespace == NS_OFFICE => {
                    for mp in container
                        .child_elements()
                        .filter(|e| e.is(NS_STYLE, "master-page"))
                    {
                        if let Some(pl_name) = mp.attribute(NS_STYLE, "page-layout-name")
                            && let Some(geom) = self.page_layouts.get(pl_name).cloned()
                        {
                            self.default_geometry = geom;
                        }
                        if self.master_headers.is_empty()
                            && let Some(hdr) = mp.child(NS_STYLE, "header")
                        {
                            self.master_headers.extend(hdr.child_elements().cloned());
                        }
                        if self.master_footers.is_empty()
                            && let Some(ftr) = mp.child(NS_STYLE, "footer")
                        {
                            self.master_footers.extend(ftr.child_elements().cloned());
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn parse_style_node(&mut self, el: &Element, is_auto: bool) {
        if el.is(NS_STYLE, "style") {
            let Some(name) = el.attribute(NS_STYLE, "name") else {
                return;
            };
            let family = el.attribute(NS_STYLE, "family").unwrap_or("paragraph");
            match family {
                "paragraph" => {
                    let ps = parse_para_style_el(el, name, is_auto);
                    self.para_styles.insert(name.to_string(), ps);
                }
                "text" => {
                    if let Some(tp) = el.child(NS_STYLE, "text-properties") {
                        let parsed = OdtTextProps::from_element(tp);
                        if !is_auto {
                            self.stylesheet.character_styles.insert(
                                name.to_string(),
                                CharacterStyle {
                                    id: name.to_string(),
                                    name: el
                                        .attribute(NS_STYLE, "display-name")
                                        .unwrap_or(name)
                                        .to_string(),
                                    font_family: parsed.family.clone(),
                                    font_size_pt: parsed.size_pt,
                                    font_weight: parsed.bold.map(|b| if b { 700 } else { 400 }),
                                    font_style: parsed.to_font_style(),
                                    color: parsed.color,
                                },
                            );
                        }
                        self.text_styles.insert(name.to_string(), parsed);
                    }
                }
                "table-column" => {
                    if let Some(cp) = el.child(NS_STYLE, "table-column-properties")
                        && let Some(w) = cp
                            .attribute(NS_STYLE, "column-width")
                            .and_then(parse_length_pt)
                            .filter(|v| *v > 0.0)
                    {
                        self.column_widths.insert(name.to_string(), w);
                    }
                }
                "table-cell" => {
                    if let Some(tcp) = el.child(NS_STYLE, "table-cell-properties")
                        && let Some(bg) = tcp
                            .attribute(NS_FO, "background-color")
                            .and_then(parse_hex_color)
                    {
                        self.cell_backgrounds.insert(name.to_string(), bg);
                    }
                }
                _ => {}
            }
        } else if el.is(NS_TEXT, "list-style") {
            let Some(name) = el.attribute(NS_STYLE, "name") else {
                return;
            };
            let mut levels = HashMap::new();
            for lvl_el in el.child_elements() {
                let lvl = lvl_el
                    .attribute(NS_TEXT, "level")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(1)
                    .saturating_sub(1);
                if lvl_el.is(NS_TEXT, "list-level-style-number") {
                    let fmt = lvl_el
                        .attribute(NS_STYLE, "num-format")
                        .unwrap_or("1")
                        .to_string();
                    levels.insert(
                        lvl,
                        OdtListLevel {
                            kind: ListKind::Ordered,
                            num_fmt: fmt,
                        },
                    );
                } else if lvl_el.is(NS_TEXT, "list-level-style-bullet") {
                    levels.insert(
                        lvl,
                        OdtListLevel {
                            kind: ListKind::Unordered,
                            num_fmt: "bullet".to_string(),
                        },
                    );
                }
            }
            self.list_styles.insert(name.to_string(), levels);
        } else if el.is(NS_STYLE, "page-layout")
            && let Some(name) = el.attribute(NS_STYLE, "name")
            && let Some(plp) = el.child(NS_STYLE, "page-layout-properties")
        {
            let geom = parse_page_layout_props(plp);
            if self.page_layouts.is_empty() {
                self.default_geometry = geom.clone();
            }
            self.page_layouts.insert(name.to_string(), geom);
        }
    }

    pub fn resolve_para(&self, name: &str) -> OdtParaStyle {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut cur = Some(name);
        while let Some(id) = cur {
            if !visited.insert(id) {
                break;
            }
            if let Some(ps) = self.para_styles.get(id) {
                chain.push(ps);
                cur = ps.parent.as_deref();
            } else {
                break;
            }
        }
        let mut acc = OdtParaStyle::default();
        for ps in chain.into_iter().rev() {
            acc.name.clone_from(&ps.name);
            if ps.display_name.is_some() {
                acc.display_name.clone_from(&ps.display_name);
            }
            if ps.parent.is_some() {
                acc.parent.clone_from(&ps.parent);
            }
            if ps.list_style_name.is_some() {
                acc.list_style_name.clone_from(&ps.list_style_name);
            }
            if ps.master_page_name.is_some() {
                acc.master_page_name.clone_from(&ps.master_page_name);
            }
            if ps.outline_level.is_some() {
                acc.outline_level = ps.outline_level;
            }
            if ps.alignment.is_some() {
                acc.alignment = ps.alignment;
            }
            if ps.space_before_pt.is_some() {
                acc.space_before_pt = ps.space_before_pt;
            }
            if ps.space_after_pt.is_some() {
                acc.space_after_pt = ps.space_after_pt;
            }
            if ps.line_spacing_mult.is_some() {
                acc.line_spacing_mult = ps.line_spacing_mult;
            }
            if ps.first_line_indent_pt.is_some() {
                acc.first_line_indent_pt = ps.first_line_indent_pt;
            }
            if ps.keep_with_next.is_some() {
                acc.keep_with_next = ps.keep_with_next;
            }
            if ps.keep_together.is_some() {
                acc.keep_together = ps.keep_together;
            }
            if ps.page_break_before {
                acc.page_break_before = true;
            }
            acc.text.overlay(&ps.text);
            acc.is_automatic = ps.is_automatic;
        }
        acc
    }

    fn materialize_stylesheet(&mut self) {
        let names: Vec<String> = self.para_styles.keys().cloned().collect();
        for name in names {
            let resolved = self.resolve_para(&name);
            if resolved.is_automatic {
                continue;
            }
            self.stylesheet.paragraph_styles.insert(
                name.clone(),
                ParagraphStyle {
                    id: name.clone(),
                    name: resolved.display_name.unwrap_or_else(|| name.clone()),
                    font_family: resolved
                        .text
                        .family
                        .unwrap_or_else(|| "Liberation Sans".to_string()),
                    font_size_pt: resolved.text.size_pt.unwrap_or(11.0).clamp(4.0, 200.0),
                    line_spacing_mult: resolved.line_spacing_mult.unwrap_or(1.15).clamp(0.5, 10.0),
                    space_before_pt: resolved.space_before_pt.unwrap_or(0.0).max(0.0),
                    space_after_pt: resolved.space_after_pt.unwrap_or(6.0).max(0.0),
                    alignment: resolved.alignment.unwrap_or(Alignment::Left),
                    first_line_indent_pt: resolved.first_line_indent_pt.unwrap_or(0.0),
                    keep_with_next: resolved.keep_with_next.unwrap_or(false),
                    keep_lines_together: resolved.keep_together.unwrap_or(false),
                    orphan_control: true,
                },
            );
        }
    }
}

fn parse_para_style_el(el: &Element, name: &str, is_auto: bool) -> OdtParaStyle {
    let mut ps = OdtParaStyle {
        name: name.to_string(),
        display_name: el.attribute(NS_STYLE, "display-name").map(str::to_string),
        parent: el
            .attribute(NS_STYLE, "parent-style-name")
            .map(str::to_string),
        list_style_name: el
            .attribute(NS_STYLE, "list-style-name")
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        master_page_name: el
            .attribute(NS_STYLE, "master-page-name")
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        outline_level: el
            .attribute(NS_STYLE, "default-outline-level")
            .and_then(|v| v.parse::<u8>().ok()),
        is_automatic: is_auto,
        ..OdtParaStyle::default()
    };
    if let Some(pp) = el.child(NS_STYLE, "paragraph-properties") {
        ps.alignment = match pp.attribute(NS_FO, "text-align") {
            Some("start" | "left") => Some(Alignment::Left),
            Some("center") => Some(Alignment::Center),
            Some("end" | "right") => Some(Alignment::Right),
            Some("justify") => Some(Alignment::Justified),
            _ => None,
        };
        ps.space_before_pt = pp
            .attribute(NS_FO, "margin-top")
            .and_then(parse_length_pt)
            .map(|v| v.max(0.0));
        ps.space_after_pt = pp
            .attribute(NS_FO, "margin-bottom")
            .and_then(parse_length_pt)
            .map(|v| v.max(0.0));
        if let Some(lh) = pp.attribute(NS_FO, "line-height")
            && let Some(pct) = lh.strip_suffix('%').and_then(|v| v.parse::<f32>().ok())
            && pct > 0.0
        {
            ps.line_spacing_mult = Some((pct / 100.0).clamp(0.5, 10.0));
        }
        ps.first_line_indent_pt = pp.attribute(NS_FO, "text-indent").and_then(parse_length_pt);
        if let Some(kwn) = pp.attribute(NS_FO, "keep-with-next") {
            ps.keep_with_next = Some(kwn == "always");
        }
        if let Some(kt) = pp.attribute(NS_FO, "keep-together") {
            ps.keep_together = Some(kt == "always");
        }
        if pp.attribute(NS_FO, "break-before") == Some("page") {
            ps.page_break_before = true;
        }
    }
    if let Some(tp) = el.child(NS_STYLE, "text-properties") {
        ps.text = OdtTextProps::from_element(tp);
    }
    ps
}

fn parse_page_layout_props(plp: &Element) -> SectionGeometry {
    let mut g = SectionGeometry::a4_portrait();
    if let Some(w) = plp
        .attribute(NS_FO, "page-width")
        .and_then(parse_length_pt)
        .filter(|v| *v > 0.0)
    {
        g.width_pt = w.clamp(72.0, 5000.0);
    }
    if let Some(h) = plp
        .attribute(NS_FO, "page-height")
        .and_then(parse_length_pt)
        .filter(|v| *v > 0.0)
    {
        g.height_pt = h.clamp(72.0, 5000.0);
    }
    if plp.attribute(NS_STYLE, "print-orientation") == Some("landscape") || g.width_pt > g.height_pt
    {
        g.orientation = PageOrientation::Landscape;
    }
    let max_h = (g.width_pt * 0.4).max(0.0);
    let max_v = (g.height_pt * 0.4).max(0.0);
    if let Some(m) = plp.attribute(NS_FO, "margin").and_then(parse_length_pt) {
        let mv = m.max(0.0);
        g.margin_top_pt = mv.min(max_v);
        g.margin_bottom_pt = mv.min(max_v);
        g.margin_left_pt = mv.min(max_h);
        g.margin_right_pt = mv.min(max_h);
    }
    if let Some(t) = plp.attribute(NS_FO, "margin-top").and_then(parse_length_pt) {
        g.margin_top_pt = t.max(0.0).min(max_v);
    }
    if let Some(b) = plp
        .attribute(NS_FO, "margin-bottom")
        .and_then(parse_length_pt)
    {
        g.margin_bottom_pt = b.max(0.0).min(max_v);
    }
    if let Some(l) = plp
        .attribute(NS_FO, "margin-left")
        .and_then(parse_length_pt)
    {
        g.margin_left_pt = l.max(0.0).min(max_h);
    }
    if let Some(r) = plp
        .attribute(NS_FO, "margin-right")
        .and_then(parse_length_pt)
    {
        g.margin_right_pt = r.max(0.0).min(max_h);
    }
    if let Some(cols) = plp.child(NS_STYLE, "columns")
        && let Some(cnt) = cols
            .attribute(NS_FO, "column-count")
            .and_then(|v| v.parse::<u32>().ok())
            .filter(|n| *n >= 1)
    {
        g.column_count = cnt.min(6);
        if let Some(gap) = cols
            .attribute(NS_FO, "column-gap")
            .and_then(parse_length_pt)
        {
            g.column_gap_pt = gap.clamp(0.0, 72.0);
        }
    }
    g
}

pub fn parse_meta(
    package: &OfficePackage,
    limits: &OfficeLimits,
) -> Result<DocumentMetadata, OfficeError> {
    let mut meta = DocumentMetadata::new("doc_odt_imported", "Untitled Document");
    let Some(bytes) = package.part("meta.xml") else {
        return Ok(meta);
    };
    let root = parse(bytes, "meta.xml", limits)?;
    let Some(office_meta) = root.child(NS_OFFICE, "meta") else {
        return Ok(meta);
    };
    let mut custom = BTreeMap::new();
    for el in office_meta.child_elements() {
        let t = el.text_content().trim().to_string();
        if t.is_empty() {
            continue;
        }
        match (el.namespace.as_str(), el.local.as_str()) {
            (NS_DC, "title") => meta.title = t,
            (NS_DC, "subject") => meta.subtitle = Some(t),
            (NS_DC, "description") => meta.description = Some(t),
            (NS_DC, "language") => meta.language = t,
            (NS_DC, "creator") | (NS_META, "initial-creator") => {
                if !meta.authors.iter().any(|a| a.name == t) {
                    meta.authors.push(Author::new(t));
                }
            }
            (NS_META, "creation-date") => meta.created_at_utc = t,
            (NS_DC, "date") => meta.modified_at_utc = t,
            (NS_META, "keyword") => meta.keywords.push(t),
            (NS_META, "user-defined") => {
                if let Some(name) = el.attribute(NS_META, "name")
                    && !name.trim().is_empty()
                {
                    custom.insert(name.trim().to_string(), t);
                }
            }
            _ => {}
        }
    }
    meta.custom_attributes = custom;
    Ok(meta)
}
