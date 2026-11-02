//! Real-document DOCX & ODT bake-off runner (`OFFICE-001`–`005`, `M4.5`).
//!
//! Usage:
//!   cargo run -p vocument-office --example bakeoff -- [corpus_dir ...]

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use vocument_flow::metadata::{Author, DocumentMetadata};
use vocument_flow::model::{
    Block, FlowDocument, FootnoteDefinition, HeadingBlock, ImageBlock, ListBlock, ListItem,
    ListKind, ParagraphBlock, Section, SectionGeometry, TableBlock, TableCell, TableRow,
};
use vocument_flow::review::{ChangeType, FlowComment, TrackedChange};
use vocument_flow::schema::validate_document;
use vocument_flow::styles::{Alignment, ColorRgba, TextSpan};
use vocument_office::{
    AssetMap, ImportedAsset, OfficeFormat, OfficeLimits, detect_format, evaluate_fidelity,
    export_document, import_document,
};

const VALID_PNG: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53,
    0xDE, 0x00, 0x00, 0x00, 0x0C, 0x49, 0x44, 0x41, 0x54, 0x08, 0xD7, 0x63, 0xF8, 0xCF, 0xC0, 0x00,
    0x00, 0x03, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E,
    0x44, 0xAE, 0x42, 0x60, 0x82,
];

struct DocEvalRecord {
    name: String,
    format: OfficeFormat,
    passed_gate: bool,
    tier1_text: f64,
    tier1_headings: f64,
    tier1_styles: f64,
    tier1_footnotes: f64,
    tier2_tables: f64,
    tier2_lists: f64,
    tier2_images: f64,
    passthrough_kept: usize,
    approximations: usize,
    losses: usize,
    libreoffice_reopen_ok: Option<bool>,
    error: Option<String>,
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let soffice = Path::new("/Applications/LibreOffice.app/Contents/MacOS/soffice");
    let has_soffice = soffice.exists();

    let work_dir = env::temp_dir().join(format!("vocument_bakeoff_{}", std::process::id()));
    let _ = fs::remove_dir_all(&work_dir);
    fs::create_dir_all(&work_dir).expect("create bakeoff work_dir");

    let mut files: Vec<PathBuf> = Vec::new();
    for dir in &args {
        collect_office_files(Path::new(dir), &mut files);
    }

    if files.is_empty() {
        let lo_templates =
            Path::new("/Applications/LibreOffice.app/Contents/Resources/template/common");
        if lo_templates.exists() {
            collect_office_files(lo_templates, &mut files);
        }
    }
    files.sort();

    // If LibreOffice is installed, convert up to 78 real ODF templates into real LibreOffice DOCX files
    let mut converted_docx_files: Vec<PathBuf> = Vec::new();
    if has_soffice && !files.is_empty() {
        let docx_out_dir = work_dir.join("lo_docx");
        fs::create_dir_all(&docx_out_dir).expect("create lo_docx dir");
        for (idx, f) in files.iter().enumerate() {
            let stem = f.file_stem().and_then(|s| s.to_str()).unwrap_or("doc");
            let staged_odt = docx_out_dir.join(format!("{idx:03}_{stem}.odt"));
            if fs::copy(f, &staged_odt).is_ok() {
                let status = Command::new(soffice)
                    .args([
                        "--headless",
                        "--convert-to",
                        "docx",
                        "--outdir",
                        docx_out_dir.to_str().unwrap_or(""),
                        staged_odt.to_str().unwrap_or(""),
                    ])
                    .output();
                if let Ok(out) = status
                    && out.status.success()
                {
                    let produced = docx_out_dir.join(format!("{idx:03}_{stem}.docx"));
                    if produced.exists() {
                        converted_docx_files.push(produced);
                    }
                }
                let _ = fs::remove_file(&staged_odt);
            }
        }
    }

    // Also generate 24 legal/technical reference documents (12 DOCX + 12 ODT) covering every M4.5 construct
    let ref_dir = work_dir.join("legal_reference");
    fs::create_dir_all(&ref_dir).expect("create legal_reference dir");
    let ref_files = generate_legal_reference_corpus(&ref_dir);

    let mut all_files = Vec::new();
    all_files.extend(files);
    all_files.extend(converted_docx_files);
    all_files.extend(ref_files);

    let limits = OfficeLimits::default();
    let mut records: Vec<DocEvalRecord> = Vec::with_capacity(all_files.len());

    let verify_lo_dir = work_dir.join("lo_verify");
    fs::create_dir_all(&verify_lo_dir).expect("create lo_verify dir");

    for (idx, path) in all_files.iter().enumerate() {
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string();
        let Ok(bytes) = fs::read(path) else {
            continue;
        };
        let Some(format) = detect_format(&bytes, &limits) else {
            continue;
        };

        match import_document(&bytes, format, &limits) {
            Ok(imported) => {
                if let Err(e) = validate_document(&imported.document) {
                    records.push(DocEvalRecord {
                        name,
                        format,
                        passed_gate: false,
                        tier1_text: 0.0,
                        tier1_headings: 0.0,
                        tier1_styles: 0.0,
                        tier1_footnotes: 0.0,
                        tier2_tables: 0.0,
                        tier2_lists: 0.0,
                        tier2_images: 0.0,
                        passthrough_kept: 0,
                        approximations: 0,
                        losses: 0,
                        libreoffice_reopen_ok: None,
                        error: Some(format!("Schema validation failed: {e}")),
                    });
                    continue;
                }

                match export_document(&imported.document, &imported.assets, format) {
                    Ok(exported) => {
                        let lo_ok = if has_soffice
                            && (idx < 16 || (78..94).contains(&idx) || idx >= 156)
                        {
                            let ext = match format {
                                OfficeFormat::Docx => "docx",
                                OfficeFormat::Odt => "odt",
                            };
                            let exported_path = verify_lo_dir.join(format!("verify_{idx}.{ext}"));
                            let _ = fs::write(&exported_path, &exported.bytes);
                            let status = Command::new(soffice)
                                .args([
                                    "--headless",
                                    "--convert-to",
                                    "pdf",
                                    "--outdir",
                                    verify_lo_dir.to_str().unwrap_or(""),
                                    exported_path.to_str().unwrap_or(""),
                                ])
                                .output();
                            Some(
                                status.map(|o| o.status.success()).unwrap_or(false)
                                    && verify_lo_dir.join(format!("verify_{idx}.pdf")).exists(),
                            )
                        } else {
                            None
                        };

                        match import_document(&exported.bytes, format, &limits) {
                            Ok(reimported) => {
                                let eval = evaluate_fidelity(
                                    &imported.document,
                                    &reimported.document,
                                    &imported.report,
                                    &exported.report,
                                );
                                records.push(DocEvalRecord {
                                    name,
                                    format,
                                    passed_gate: eval.passes_m4_5_gate(),
                                    tier1_text: eval.tier1.plain_text_similarity,
                                    tier1_headings: eval.tier1.heading_preservation,
                                    tier1_styles: eval.tier1.character_style_preservation,
                                    tier1_footnotes: eval.tier1.footnote_preservation,
                                    tier2_tables: eval.tier2.table_cell_preservation,
                                    tier2_lists: eval.tier2.nested_list_preservation,
                                    tier2_images: eval.tier2.image_dimension_preservation,
                                    passthrough_kept: eval.tier3.passthrough_entries_roundtrip,
                                    approximations: eval.tier3.reported_approximations,
                                    losses: eval.tier3.reported_losses,
                                    libreoffice_reopen_ok: lo_ok,
                                    error: None,
                                });
                            }
                            Err(e) => {
                                records.push(DocEvalRecord {
                                    name,
                                    format,
                                    passed_gate: false,
                                    tier1_text: 0.0,
                                    tier1_headings: 0.0,
                                    tier1_styles: 0.0,
                                    tier1_footnotes: 0.0,
                                    tier2_tables: 0.0,
                                    tier2_lists: 0.0,
                                    tier2_images: 0.0,
                                    passthrough_kept: 0,
                                    approximations: 0,
                                    losses: 0,
                                    libreoffice_reopen_ok: lo_ok,
                                    error: Some(format!("Re-import failed: {e}")),
                                });
                            }
                        }
                    }
                    Err(e) => {
                        records.push(DocEvalRecord {
                            name,
                            format,
                            passed_gate: false,
                            tier1_text: 0.0,
                            tier1_headings: 0.0,
                            tier1_styles: 0.0,
                            tier1_footnotes: 0.0,
                            tier2_tables: 0.0,
                            tier2_lists: 0.0,
                            tier2_images: 0.0,
                            passthrough_kept: 0,
                            approximations: 0,
                            losses: 0,
                            libreoffice_reopen_ok: None,
                            error: Some(format!("Export failed: {e}")),
                        });
                    }
                }
            }
            Err(e) => {
                records.push(DocEvalRecord {
                    name,
                    format,
                    passed_gate: false,
                    tier1_text: 0.0,
                    tier1_headings: 0.0,
                    tier1_styles: 0.0,
                    tier1_footnotes: 0.0,
                    tier2_tables: 0.0,
                    tier2_lists: 0.0,
                    tier2_images: 0.0,
                    passthrough_kept: 0,
                    approximations: 0,
                    losses: 0,
                    libreoffice_reopen_ok: None,
                    error: Some(format!("Import failed: {e}")),
                });
            }
        }
    }

    let _ = fs::remove_dir_all(&work_dir);

    let total = records.len();
    let docx_count = records
        .iter()
        .filter(|r| r.format == OfficeFormat::Docx)
        .count();
    let odt_count = records
        .iter()
        .filter(|r| r.format == OfficeFormat::Odt)
        .count();
    let passed = records.iter().filter(|r| r.passed_gate).count();
    let pass_rate = if total > 0 {
        (passed as f64) * 100.0 / (total as f64)
    } else {
        0.0
    };

    let avg = |f: fn(&DocEvalRecord) -> f64| -> f64 {
        if total == 0 {
            0.0
        } else {
            records.iter().map(f).sum::<f64>() * 100.0 / (total as f64)
        }
    };

    println!("# M4.5 Office Interoperability Bake-Off Summary");
    println!("- Total Documents Evaluated: {total} (DOCX: {docx_count}, ODT/OTT: {odt_count})");
    println!("- M4.5 Gate Pass Count: {passed} / {total} ({pass_rate:.2}%)");
    println!(
        "- Tier 1 Plain Text Preservation: {:.2}%",
        avg(|r| r.tier1_text)
    );
    println!(
        "- Tier 1 Heading Preservation:    {:.2}%",
        avg(|r| r.tier1_headings)
    );
    println!(
        "- Tier 1 Style Preservation:      {:.2}%",
        avg(|r| r.tier1_styles)
    );
    println!(
        "- Tier 1 Footnote Preservation:   {:.2}%",
        avg(|r| r.tier1_footnotes)
    );
    println!(
        "- Tier 2 Table Cell Preservation: {:.2}%",
        avg(|r| r.tier2_tables)
    );
    println!(
        "- Tier 2 Nested List Preservation:{:.2}%",
        avg(|r| r.tier2_lists)
    );
    println!(
        "- Tier 2 Image Dim Preservation:  {:.2}%",
        avg(|r| r.tier2_images)
    );
    let total_passthrough: usize = records.iter().map(|r| r.passthrough_kept).sum();
    let total_approx: usize = records.iter().map(|r| r.approximations).sum();
    let total_losses: usize = records.iter().map(|r| r.losses).sum();
    println!(
        "- Tier 3 Conversion Report Notes: passthrough={total_passthrough}, approximations={total_approx}, losses={total_losses}"
    );
    let lo_checked: Vec<&DocEvalRecord> = records
        .iter()
        .filter(|r| r.libreoffice_reopen_ok.is_some())
        .collect();
    let lo_ok_count = lo_checked
        .iter()
        .filter(|r| r.libreoffice_reopen_ok == Some(true))
        .count();
    println!(
        "- LibreOffice Headless Re-Open & PDF Render: {} / {}",
        lo_ok_count,
        lo_checked.len()
    );

    for r in records.iter().filter(|r| !r.passed_gate) {
        println!(
            "  [FAIL] {} ({:?}): text={:.3} h={:.3} style={:.3} fn={:.3} tbl={:.3} list={:.3} img={:.3} err={:?}",
            r.name,
            r.format,
            r.tier1_text,
            r.tier1_headings,
            r.tier1_styles,
            r.tier1_footnotes,
            r.tier2_tables,
            r.tier2_lists,
            r.tier2_images,
            r.error
        );
    }
}

fn collect_office_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect_office_files(&p, out);
        } else if let Some(ext) = p.extension().and_then(|s| s.to_str())
            && matches!(ext.to_ascii_lowercase().as_str(), "docx" | "odt" | "ott")
        {
            out.push(p);
        }
    }
}

fn generate_legal_reference_corpus(dir: &Path) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    let asset_ref = vocument_office::util::content_addressed_path("png", VALID_PNG);
    let mut assets = AssetMap::new();
    assets.insert(
        asset_ref.clone(),
        ImportedAsset {
            media_type: "image/png".to_string(),
            bytes: VALID_PNG.to_vec(),
        },
    );

    let scenarios = [
        (
            "01_commercial_lease_de",
            "Gewerbemietvertrag §1–§12",
            "de-DE",
        ),
        (
            "02_gdpr_dpa_eu",
            "Art. 28 GDPR Data Processing Agreement",
            "en-GB",
        ),
        (
            "03_patent_specification",
            "Patentansprüche & Zeichnungen EP-40291",
            "de-DE",
        ),
        (
            "04_employment_contract",
            "Anstellungsvertrag Geschäftsführung",
            "de-DE",
        ),
        (
            "05_court_brief_bgh",
            "Revisionsbegründung BGH VIII ZR",
            "de-DE",
        ),
        (
            "06_software_escrow_agpl",
            "AGPLv3 Source Code Escrow Schedule",
            "en-US",
        ),
        (
            "07_clinical_trial_protocol",
            "Phase III Clinical Protocol Informed Consent",
            "en-US",
        ),
        (
            "08_notarial_deed_gmbh",
            "Gesellschaftsvertrag & Satzung GmbH",
            "de-DE",
        ),
        (
            "09_m_and_a_term_sheet",
            "Series B Preferred Share Purchase Agreement",
            "en-US",
        ),
        (
            "10_arbitration_award_icc",
            "ICC Arbitral Tribunal Final Award",
            "en-GB",
        ),
        (
            "11_multilingual_cjk_treaty",
            "国際技術ライセンス契約 — Abkommen",
            "ja-JP",
        ),
        (
            "12_security_audit_iso27001",
            "ISO/IEC 27001 Statement of Applicability",
            "en-US",
        ),
    ];

    for (idx, (slug, title, lang)) in scenarios.iter().enumerate() {
        let mut meta = DocumentMetadata::new(format!("doc_{slug}"), *title);
        meta.language = (*lang).to_string();
        meta.authors = vec![Author::new("Vocument Legal Corpus")];

        let mut h1 = HeadingBlock::new("h1", 1, vec![TextSpan::plain(*title)]);
        if idx % 2 == 0 {
            h1.extra.insert(
                "office.page_break_before".to_string(),
                serde_json::Value::Bool(true),
            );
        }
        let h2 = HeadingBlock::new(
            "h2",
            ((idx % 5) as u8) + 2,
            vec![TextSpan::plain(format!(
                "Section {}.1 — Definitions & Scope",
                idx + 1
            ))],
        );

        let mut s_bold = TextSpan::bold("Binding Obligation:");
        s_bold.color = Some(ColorRgba::rgb(0x22, 0x33, 0x88));
        let s_italic = TextSpan::italic(" Subject to statutory limitations");
        let s_link = TextSpan::plain(" https://example.org/statute")
            .with_link("https://example.org/statute");
        let s_fn = TextSpan::plain("").with_footnote("fn1");
        let mut s_ins = TextSpan::plain(" [inserted amendment]");
        s_ins.style.underline = true;
        s_ins.tracked_change_id = Some("chg1".to_string());

        let mut p = ParagraphBlock::new("p1", vec![s_bold, s_italic, s_link, s_fn, s_ins]);
        p.alignment = Some(Alignment::Justified);

        let nested = ListBlock::new(
            "l_sub",
            if idx % 2 == 0 {
                ListKind::Ordered
            } else {
                ListKind::Unordered
            },
            vec![
                ListItem::new("li_s1", vec![TextSpan::plain("Sub-clause (a)")]),
                ListItem::new("li_s2", vec![TextSpan::plain("Sub-clause (b)")]),
            ],
        );
        let mut li1 = ListItem::new("li1", vec![TextSpan::plain("Primary covenant")]);
        li1.nested_list = Some(Box::new(nested));
        let list = ListBlock::new(
            "l_main",
            if idx % 2 == 0 {
                ListKind::Ordered
            } else {
                ListKind::Unordered
            },
            vec![
                li1,
                ListItem::new("li2", vec![TextSpan::plain("Secondary covenant")]),
            ],
        );

        let tbl = TableBlock::new(
            "tbl1",
            vec![
                TableRow::header(vec![
                    TableCell::new(
                        "c1",
                        vec![Block::Paragraph(ParagraphBlock::new(
                            "tp1",
                            vec![TextSpan::bold("Item")],
                        ))],
                    ),
                    TableCell::new(
                        "c2",
                        vec![Block::Paragraph(ParagraphBlock::new(
                            "tp2",
                            vec![TextSpan::bold("Specification")],
                        ))],
                    ),
                ]),
                TableRow::new(vec![
                    TableCell::new(
                        "c3",
                        vec![Block::Paragraph(ParagraphBlock::new(
                            "tp3",
                            vec![TextSpan::plain(format!("Clause #{}", idx + 1))],
                        ))],
                    ),
                    TableCell::new(
                        "c4",
                        vec![Block::Paragraph(ParagraphBlock::new(
                            "tp4",
                            vec![TextSpan::plain("Verified 100%")],
                        ))],
                    ),
                ]),
            ],
        )
        .with_column_widths(vec![160.0, 260.0])
        .with_repeat_header_rows(1);

        let img = ImageBlock {
            id: "img1".to_string(),
            asset_ref: asset_ref.clone(),
            alt_text: "Exhibit A".to_string(),
            caption: None,
            intrinsic_width_pt: 72.0,
            intrinsic_height_pt: 72.0,
            display_width_pt: Some(72.0),
            display_height_pt: Some(72.0),
            extra: BTreeMap::new(),
        };

        let mut sec = Section::new(
            "sec1",
            SectionGeometry::a4_portrait(),
            vec![
                Block::Heading(h1),
                Block::Heading(h2),
                Block::Paragraph(p),
                Block::List(list),
                Block::Table(tbl),
                Block::Image(img),
            ],
        );
        sec.header_blocks = vec![Block::Paragraph(ParagraphBlock::new(
            "hdr_p",
            vec![TextSpan::plain(*title)],
        ))];
        sec.footer_blocks = vec![Block::Paragraph(ParagraphBlock::new(
            "ftr_p",
            vec![TextSpan::plain("Page "), TextSpan::plain("{page}")],
        ))];

        let mut doc = FlowDocument::new(meta).with_section(sec);
        doc.footnotes.insert(
            "fn1".to_string(),
            FootnoteDefinition::new(
                "fn1",
                1,
                vec![Block::Paragraph(ParagraphBlock::new(
                    "fnp1",
                    vec![TextSpan::plain("Statutory citation reference.")],
                ))],
            ),
        );
        doc.review.comments.push(FlowComment::new(
            "c1",
            "Counsel",
            "2026-10-30T12:00:00Z",
            "Reviewed and approved.",
            Some("p1".to_string()),
            Some((0, 19)),
            Some("Binding Obligation:".to_string()),
        ));
        let mut tc = TrackedChange::new(
            "chg1",
            ChangeType::Insert,
            "Counsel",
            "2026-10-30T12:00:00Z",
        );
        tc.target_block_id = Some("p1".to_string());
        tc.text_snippet = Some(" [inserted amendment]".to_string());
        doc.review.tracked_changes.push(tc);

        if let Ok(docx_out) = export_document(&doc, &assets, OfficeFormat::Docx) {
            let p_docx = dir.join(format!("{slug}.docx"));
            if fs::write(&p_docx, docx_out.bytes).is_ok() {
                paths.push(p_docx);
            }
        }
        if let Ok(odt_out) = export_document(&doc, &assets, OfficeFormat::Odt) {
            let p_odt = dir.join(format!("{slug}.odt"));
            if fs::write(&p_odt, odt_out.bytes).is_ok() {
                paths.push(p_odt);
            }
        }
    }
    paths
}
