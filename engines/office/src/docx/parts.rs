//! DOCX package parts: relationships (`.rels`), numbering (`numbering.xml`), styles (`styles.xml`),
//! comments (`comments.xml` + `commentsExtended.xml`), footnotes/endnotes, and core properties.

use std::collections::{BTreeMap, HashMap, HashSet};

use vocument_flow::metadata::{Author, DocumentMetadata};
use vocument_flow::model::ListKind;
use vocument_flow::styles::{Alignment, CharacterStyle, FontStyle, ParagraphStyle, Stylesheet};

use crate::docx::props::{ParagraphProps, RunProps, attr, child_w, is_w, val};
use crate::error::OfficeError;
use crate::limits::OfficeLimits;
use crate::package::OfficePackage;
use crate::report::{ConversionReport, Severity};
use crate::xml::parse;

const REL_NS: &str = "http://schemas.openxmlformats.org/package/2006/relationships";
const W15_NS: &str = "http://schemas.microsoft.com/office/word/2012/wordml";

/// Target of a package relationship (`_rels/*.rels`).
#[derive(Debug, Clone)]
pub struct Relationship {
    pub id: String,
    pub rel_type: String,
    pub target: String,
    pub is_external: bool,
}

impl Relationship {
    /// Resolves a relative internal relationship target against `base_dir` (e.g. `"word"`).
    pub fn resolved_part_path(&self, base_dir: &str) -> String {
        let raw = self.target.trim_start_matches('/');
        if self.target.starts_with('/') {
            return normalize_part_path(raw);
        }
        if base_dir.is_empty() {
            normalize_part_path(raw)
        } else {
            normalize_part_path(&format!("{base_dir}/{raw}"))
        }
    }
}

fn normalize_part_path(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

/// Parses a `.rels` XML part into a map keyed by relationship `Id`.
pub fn parse_relationships(
    package: &OfficePackage,
    rels_part: &str,
    limits: &OfficeLimits,
    report: &mut ConversionReport,
) -> Result<HashMap<String, Relationship>, OfficeError> {
    let Some(bytes) = package.part(rels_part) else {
        return Ok(HashMap::new());
    };
    let root = parse(bytes, rels_part, limits)?;
    let mut map = HashMap::new();
    for child in root.child_elements() {
        if child.local != "Relationship" && child.namespace != REL_NS {
            continue;
        }
        let Some(id) = child.attribute_any_namespace("Id") else {
            continue;
        };
        let rel_type = child
            .attribute_any_namespace("Type")
            .unwrap_or_default()
            .to_string();
        let target = child
            .attribute_any_namespace("Target")
            .unwrap_or_default()
            .to_string();
        let is_external = child
            .attribute_any_namespace("TargetMode")
            .is_some_and(|mode| mode.eq_ignore_ascii_case("External"));
        if is_external
            && (rel_type.ends_with("/attachedTemplate")
                || rel_type.ends_with("/subDocument")
                || rel_type.ends_with("/oleObject")
                || rel_type.ends_with("/frame"))
        {
            report.note(
                Severity::Security,
                "docx.rel.external-resource",
                "External template, frame, or OLE relationship ignored for security",
                Some(rels_part),
            );
            continue;
        }
        map.insert(
            id.to_string(),
            Relationship {
                id: id.to_string(),
                rel_type,
                target,
                is_external,
            },
        );
    }
    Ok(map)
}

/// Resolved style definition from `word/styles.xml`.
#[derive(Debug, Clone, Default)]
pub struct ResolvedDocxStyles {
    pub stylesheet: Stylesheet,
    pub default_run: RunProps,
    pub default_paragraph: ParagraphProps,
    pub paragraph_props: HashMap<String, ParagraphProps>,
    pub paragraph_run_props: HashMap<String, RunProps>,
    pub character_run_props: HashMap<String, RunProps>,
    /// Heading level (1..=9) configured on a paragraph style via outlineLvl or standard name.
    pub heading_levels: HashMap<String, u8>,
}

impl ResolvedDocxStyles {
    /// Resolves effective paragraph properties for `style_id`.
    pub fn effective_paragraph(&self, style_id: Option<&str>) -> ParagraphProps {
        let mut props = self.default_paragraph.clone();
        if let Some(id) = style_id
            && let Some(style_props) = self.paragraph_props.get(id)
        {
            props.overlay(style_props);
        }
        props
    }

    /// Resolves effective run properties inherited from defaults, paragraph style, and char style.
    pub fn effective_run(
        &self,
        para_style_id: Option<&str>,
        char_style_id: Option<&str>,
    ) -> RunProps {
        let mut run = self.default_run.clone();
        if let Some(pid) = para_style_id
            && let Some(pr) = self.paragraph_run_props.get(pid)
        {
            run.overlay(pr);
        }
        if let Some(cid) = char_style_id
            && let Some(cr) = self.character_run_props.get(cid)
        {
            run.overlay(cr);
        }
        run
    }
}

#[derive(Default)]
struct RawStyleEntry {
    style_id: String,
    name: String,
    kind: String,
    based_on: Option<String>,
    p_props: ParagraphProps,
    r_props: RunProps,
}

/// Parses `word/styles.xml` and resolves `w:basedOn` inheritance chains.
pub fn parse_styles(
    package: &OfficePackage,
    part_path: &str,
    limits: &OfficeLimits,
) -> Result<ResolvedDocxStyles, OfficeError> {
    let mut resolved = ResolvedDocxStyles {
        stylesheet: Stylesheet::standard_legal(),
        ..ResolvedDocxStyles::default()
    };
    let Some(bytes) = package.part(part_path) else {
        return Ok(resolved);
    };
    let root = parse(bytes, part_path, limits)?;

    let mut raw_entries: HashMap<String, RawStyleEntry> = HashMap::new();
    for child in root.child_elements() {
        if is_w(child, "docDefaults") {
            if let Some(rpr_default) = child_w(child, "rPrDefault")
                && let Some(rpr) = child_w(rpr_default, "rPr")
            {
                resolved.default_run = RunProps::from_element(rpr);
            }
            if let Some(ppr_default) = child_w(child, "pPrDefault")
                && let Some(ppr) = child_w(ppr_default, "pPr")
            {
                let (p, _) = ParagraphProps::from_element(ppr);
                resolved.default_paragraph = p;
            }
        } else if is_w(child, "style") {
            let Some(style_id) = attr(child, "styleId").map(str::to_string) else {
                continue;
            };
            let style_id = style_id.trim().to_string();
            if style_id.is_empty() {
                continue;
            }
            let kind = attr(child, "type").unwrap_or("paragraph").to_string();
            let name = child_w(child, "name")
                .and_then(val)
                .unwrap_or(&style_id)
                .to_string();
            let based_on = child_w(child, "basedOn").and_then(val).map(str::to_string);
            let p_props = child_w(child, "pPr")
                .map(|el| ParagraphProps::from_element(el).0)
                .unwrap_or_default();
            let r_props = child_w(child, "rPr")
                .map(RunProps::from_element)
                .unwrap_or_default();
            raw_entries.insert(
                style_id.clone(),
                RawStyleEntry {
                    style_id,
                    name,
                    kind,
                    based_on,
                    p_props,
                    r_props,
                },
            );
        }
    }

    for style_id in raw_entries.keys() {
        let mut chain = Vec::new();
        let mut visited = HashSet::new();
        let mut cursor = Some(style_id.as_str());
        while let Some(current_id) = cursor {
            if !visited.insert(current_id) {
                break;
            }
            if let Some(entry) = raw_entries.get(current_id) {
                chain.push(entry);
                cursor = entry.based_on.as_deref();
            } else {
                break;
            }
        }
        chain.reverse();
        let Some(leaf) = chain.last() else {
            continue;
        };

        let mut merged_p = resolved.default_paragraph.clone();
        let mut merged_r = resolved.default_run.clone();
        for item in &chain {
            merged_p.overlay(&item.p_props);
            merged_r.overlay(&item.r_props);
        }

        if leaf.kind == "paragraph" {
            if let Some(level) =
                infer_heading_level(&leaf.style_id, &leaf.name, merged_p.outline_level)
            {
                resolved.heading_levels.insert(leaf.style_id.clone(), level);
            }
            let para_style = ParagraphStyle {
                id: leaf.style_id.clone(),
                name: leaf.name.clone(),
                font_family: merged_r
                    .family
                    .clone()
                    .unwrap_or_else(|| "Inter".to_string()),
                font_size_pt: merged_r.size_pt.unwrap_or(11.0).clamp(4.0, 200.0),
                line_spacing_mult: merged_p
                    .line_spacing_multiple
                    .unwrap_or(1.15)
                    .clamp(0.5, 5.0),
                space_before_pt: merged_p.space_before_pt.unwrap_or(0.0).max(0.0),
                space_after_pt: merged_p.space_after_pt.unwrap_or(6.0).max(0.0),
                alignment: merged_p.alignment.unwrap_or(Alignment::Left),
                first_line_indent_pt: merged_p.first_line_indent_pt.unwrap_or(0.0),
                keep_with_next: merged_p.keep_with_next.unwrap_or(false),
                keep_lines_together: merged_p.keep_lines_together.unwrap_or(false),
                orphan_control: merged_p.widow_control.unwrap_or(true),
            };
            resolved
                .stylesheet
                .paragraph_styles
                .insert(leaf.style_id.clone(), para_style);
            resolved
                .paragraph_props
                .insert(leaf.style_id.clone(), merged_p);
            resolved
                .paragraph_run_props
                .insert(leaf.style_id.clone(), merged_r);
        } else if leaf.kind == "character" {
            let char_style = CharacterStyle {
                id: leaf.style_id.clone(),
                name: leaf.name.clone(),
                font_family: merged_r.family.clone(),
                font_size_pt: merged_r.size_pt.map(|s| s.clamp(4.0, 200.0)),
                font_weight: merged_r.bold.map(|b| if b { 700 } else { 400 }),
                font_style: FontStyle {
                    bold: merged_r.bold.unwrap_or(false),
                    italic: merged_r.italic.unwrap_or(false),
                    underline: merged_r.underline.unwrap_or(false),
                    strikethrough: merged_r.strike.unwrap_or(false),
                    code: false,
                    subscript: merged_r.subscript.unwrap_or(false),
                    superscript: merged_r.superscript.unwrap_or(false),
                },
                color: merged_r.color.flatten(),
            };
            resolved
                .stylesheet
                .character_styles
                .insert(leaf.style_id.clone(), char_style);
            resolved
                .character_run_props
                .insert(leaf.style_id.clone(), merged_r);
        }
    }
    Ok(resolved)
}

fn infer_heading_level(style_id: &str, name: &str, outline_level: Option<u8>) -> Option<u8> {
    if let Some(level) = outline_level.filter(|l| *l < 9) {
        return Some(level + 1);
    }
    for candidate in [style_id, name] {
        let lower = candidate.trim().to_ascii_lowercase();
        for prefix in [
            "heading ",
            "heading",
            "überschrift ",
            "überschrift",
            "titre ",
        ] {
            if let Some(rest) = lower.strip_prefix(prefix)
                && let Ok(level) = rest.trim().parse::<u8>()
                && (1..=9).contains(&level)
            {
                return Some(level);
            }
        }
    }
    None
}

/// Resolved list level from `word/numbering.xml`.
#[derive(Debug, Clone)]
pub struct NumberingLevel {
    pub kind: ListKind,
    pub num_fmt: String,
}

/// Numbering definitions keyed by `(numId, ilvl)`.
#[derive(Debug, Clone, Default)]
pub struct NumberingMap {
    levels: HashMap<(String, u8), NumberingLevel>,
}

impl NumberingMap {
    pub fn get(&self, num_id: &str, ilvl: u8) -> NumberingLevel {
        self.levels
            .get(&(num_id.to_string(), ilvl))
            .cloned()
            .unwrap_or(NumberingLevel {
                kind: ListKind::Unordered,
                num_fmt: "bullet".to_string(),
            })
    }
}

/// Parses `word/numbering.xml`.
pub fn parse_numbering(
    package: &OfficePackage,
    part_path: &str,
    limits: &OfficeLimits,
) -> Result<NumberingMap, OfficeError> {
    let Some(bytes) = package.part(part_path) else {
        return Ok(NumberingMap::default());
    };
    let root = parse(bytes, part_path, limits)?;
    let mut abstract_nums: HashMap<String, HashMap<u8, NumberingLevel>> = HashMap::new();
    let mut num_to_abstract: HashMap<String, String> = HashMap::new();

    for child in root.child_elements() {
        if is_w(child, "abstractNum") {
            let Some(abs_id) = attr(child, "abstractNumId").map(str::to_string) else {
                continue;
            };
            let mut lvl_map = HashMap::new();
            for lvl in child.child_elements().filter(|el| is_w(el, "lvl")) {
                let ilvl = attr(lvl, "ilvl")
                    .and_then(|v| v.parse::<u8>().ok())
                    .unwrap_or(0);
                let num_fmt = child_w(lvl, "numFmt")
                    .and_then(val)
                    .unwrap_or("bullet")
                    .to_string();
                let kind = if matches!(num_fmt.as_str(), "bullet" | "none") {
                    ListKind::Unordered
                } else {
                    ListKind::Ordered
                };
                lvl_map.insert(ilvl, NumberingLevel { kind, num_fmt });
            }
            abstract_nums.insert(abs_id, lvl_map);
        } else if is_w(child, "num") {
            let Some(num_id) = attr(child, "numId").map(str::to_string) else {
                continue;
            };
            if let Some(abs_id) = child_w(child, "abstractNumId").and_then(val) {
                num_to_abstract.insert(num_id, abs_id.to_string());
            }
        }
    }

    let mut levels = HashMap::new();
    for (num_id, abs_id) in num_to_abstract {
        if let Some(lvl_map) = abstract_nums.get(&abs_id) {
            for (ilvl, info) in lvl_map {
                levels.insert((num_id.clone(), *ilvl), info.clone());
            }
        }
    }
    Ok(NumberingMap { levels })
}

/// Raw comment parsed from `word/comments.xml` + `word/commentsExtended.xml`.
#[derive(Debug, Clone)]
pub struct RawDocxComment {
    pub id: String,
    pub author: String,
    pub date: String,
    pub text: String,
    pub para_id: Option<String>,
    pub parent_para_id: Option<String>,
    pub done: bool,
}

/// Parses `word/comments.xml` and optional `word/commentsExtended.xml`.
pub fn parse_comments(
    package: &OfficePackage,
    comments_part: &str,
    extended_part: &str,
    limits: &OfficeLimits,
) -> Result<BTreeMap<String, RawDocxComment>, OfficeError> {
    let Some(bytes) = package.part(comments_part) else {
        return Ok(BTreeMap::new());
    };
    let root = parse(bytes, comments_part, limits)?;

    // Extended info keyed by w15:paraId -> (parent_para_id, done).
    let mut extended_by_para: HashMap<String, (Option<String>, bool)> = HashMap::new();
    if let Some(ext_bytes) = package.part(extended_part)
        && let Ok(ext_root) = parse(ext_bytes, extended_part, limits)
    {
        for child in ext_root.child_elements() {
            if child.local == "commentEx" || child.namespace == W15_NS {
                let Some(para_id) = child.attribute_any_namespace("paraId") else {
                    continue;
                };
                let parent = child
                    .attribute_any_namespace("paraIdParent")
                    .map(str::to_string);
                let done = matches!(
                    child.attribute_any_namespace("done"),
                    Some("1" | "true" | "on")
                );
                extended_by_para.insert(para_id.to_string(), (parent, done));
            }
        }
    }

    let mut map = BTreeMap::new();
    for child in root.child_elements().filter(|el| is_w(el, "comment")) {
        let Some(id) = attr(child, "id").map(str::to_string) else {
            continue;
        };
        let author = attr(child, "author").unwrap_or("Author").to_string();
        let date = attr(child, "date")
            .unwrap_or("1970-01-01T00:00:00Z")
            .to_string();
        let mut paragraphs = Vec::new();
        let mut last_para_id = None;
        for p in child.child_elements().filter(|el| is_w(el, "p")) {
            if let Some(pid) = p.attribute_any_namespace("paraId") {
                last_para_id = Some(pid.to_string());
            }
            let text = p.text_content();
            if !text.is_empty() {
                paragraphs.push(text);
            }
        }
        let (parent_para_id, done) = last_para_id
            .as_ref()
            .and_then(|pid| extended_by_para.get(pid).cloned())
            .unwrap_or((None, false));
        map.insert(
            id.clone(),
            RawDocxComment {
                id,
                author,
                date,
                text: paragraphs.join("\n"),
                para_id: last_para_id,
                parent_para_id,
                done,
            },
        );
    }
    Ok(map)
}

/// Parses `docProps/core.xml` and `docProps/custom.xml` into [`DocumentMetadata`].
pub fn parse_metadata(
    package: &OfficePackage,
    limits: &OfficeLimits,
) -> Result<DocumentMetadata, OfficeError> {
    let mut meta = DocumentMetadata::new("doc_docx", "Untitled Document");
    if let Some(bytes) = package.part("docProps/core.xml") {
        let root = parse(bytes, "docProps/core.xml", limits)?;
        for child in root.child_elements() {
            let text = child.text_content().trim().to_string();
            if text.is_empty() {
                continue;
            }
            match child.local.as_str() {
                "title" => meta.title = text,
                "subject" => meta.subtitle = Some(text),
                "creator" => meta.authors.push(Author::new(text)),
                "description" => meta.description = Some(text),
                "keywords" => {
                    meta.keywords = text
                        .split([',', ';'])
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }
                "language" => meta.language = text,
                "created" => meta.created_at_utc = text,
                "modified" => meta.modified_at_utc = text,
                "revision" => {
                    if let Ok(rev) = text.parse::<u64>() {
                        meta.revision = rev.max(1);
                    }
                }
                _ => {}
            }
        }
    }
    if let Some(bytes) = package.part("docProps/custom.xml")
        && let Ok(root) = parse(bytes, "docProps/custom.xml", limits)
    {
        for prop in root.child_elements().filter(|el| el.local == "property") {
            if let Some(name) = prop.attribute_any_namespace("name") {
                let value = prop.text_content();
                meta.custom_attributes.insert(name.to_string(), value);
            }
        }
    }
    Ok(meta)
}
