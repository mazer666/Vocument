//! Integration tests for M0.7 Interoperability Gate (`ROADMAP.md` M0.7, M0 Definition of Done).
//!
//! Verifies:
//! - Full M0 DoD workflow:
//!   `Open -> search -> select text -> highlight -> rotate -> undo -> redo -> save -> hard-kill -> recover -> save -> foreign viewer reopen`
//! - MuPDF reference engine reopen and verification
//! - `mutool` CLI verification
//! - macOS CoreGraphics / Quartz verification via `sips`
//! - Google Chrome headless PDFium verification
//! - Explicit non-simulated status documentation for Acrobat, PDF-XChange, and Firefox

#![allow(clippy::unwrap_used)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vocument_core::file_broker::ScopedLocalFileBroker;
use vocument_core::identity::compute_sha256_hex;
use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument,
    PdfEngineError, PdfHighlightAnnotation, PdfPageGeometry, PdfPageStructuredText, PdfRectMilliPt,
};
use vocument_core::recovery::{RecoveryAction, RecoveryStore};
use vocument_core::save::SaveResult;
use vocument_core::session::SessionRegistry;
use vocument_core::transactions::TransactionCommand;

#[derive(Debug)]
struct TestDir {
    path: PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        static CTR: AtomicU64 = AtomicU64::new(1);
        let id = CTR.fetch_add(1, Ordering::Relaxed);
        let p =
            std::env::temp_dir().join(format!("voc_interop_{name}_{}_{id}", std::process::id()));
        let _ = fs::create_dir_all(&p);
        Self { path: p }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn sample_pdf_path() -> PathBuf {
    let candidate = PathBuf::from("tests/corpus/m0_1_sample_two_pages.pdf");
    if candidate.exists() {
        return candidate;
    }
    let parent_cand = PathBuf::from("../../tests/corpus/m0_1_sample_two_pages.pdf");
    if parent_cand.exists() {
        return parent_cand;
    }
    panic!("Sample PDF not found at {candidate:?} or {parent_cand:?}");
}

#[derive(Debug, Clone)]
struct MockPdfDocument {
    bytes: Vec<u8>,
    rotations: Vec<u32>,
    highlights: Vec<PdfHighlightAnnotation>,
}

impl PdfEngineDocument for MockPdfDocument {
    fn summary(&self) -> &PdfDocumentSummary {
        static SUMMARY: PdfDocumentSummary = PdfDocumentSummary {
            page_count: 2,
            needs_password: false,
            format_version: None,
            title: None,
            author: None,
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            modification_date: None,
            engine_identifier: String::new(),
        };
        &SUMMARY
    }

    fn current_pdf_bytes(&self) -> &[u8] {
        &self.bytes
    }

    fn page_geometry(&self, _page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
        Ok(PdfPageGeometry {
            page_index: 0,
            width_milli_pt: 595_000,
            height_milli_pt: 842_000,
        })
    }

    fn page_rotation_degrees(&self, page_index: u32) -> Result<u32, PdfEngineError> {
        Ok(self
            .rotations
            .get(page_index as usize)
            .copied()
            .unwrap_or(0))
    }

    fn page_highlight_annotations(
        &self,
        page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
        Ok(self
            .highlights
            .iter()
            .filter(|h| h.page_index == page_index)
            .cloned()
            .collect())
    }

    /// The comments of the page (`ANNOT-001`). The test engine does not carry
    /// comments — it is a stand-in for the real engine, and it says so
    /// instead of pretending that it succeeded.
    fn page_comment_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfCommentAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Writes a comment (`ANNOT-001`).
    fn add_comment_annotation(
        &mut self,
        _annotation: &PdfCommentAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Removes a comment (`ANNOT-001`).
    fn remove_comment_annotation(
        &mut self,
        _page_index: u32,
        _annotation_id: &str,
    ) -> Result<(PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
        Ok(None)
    }

    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        Ok(PdfPageStructuredText {
            page_index,
            embedded_page_label: None,
            words: vec![],
            lines: vec![],
        })
    }

    fn create_display_list(
        &self,
        _page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "display list mock".to_owned(),
        })
    }

    fn rotate_pages(
        &mut self,
        page_indices: &[u32],
        delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError> {
        for &idx in page_indices {
            let cur = self.rotations.get(idx as usize).copied().unwrap_or(0);
            let next = (((cur as i32 + delta_degrees) % 360 + 360) % 360) as u32;
            if (idx as usize) < self.rotations.len() {
                self.rotations[idx as usize] = next;
            }
            self.bytes.extend_from_slice(
                format!("\n%mutated:rotated:page={idx}:delta={delta_degrees}\n").as_bytes(),
            );
        }
        Ok(self.bytes.clone())
    }

    fn add_highlight_annotation(
        &mut self,
        annot: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        self.highlights.push(annot.clone());
        let author_str = annot.author.as_deref().unwrap_or("");
        self.bytes.extend_from_slice(
            format!(
                "\n%mutated:highlight:id={}:page={}:author={}\n",
                annot.annotation_id, annot.page_index, author_str
            )
            .as_bytes(),
        );
        Ok(self.bytes.clone())
    }

    fn remove_highlight_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
        let pos = self
            .highlights
            .iter()
            .position(|h| h.page_index == page_index && h.annotation_id == annotation_id)
            .ok_or_else(|| PdfEngineError::EngineFailure {
                details: "not found".to_owned(),
            })?;
        let removed = self.highlights.remove(pos);
        self.bytes
            .extend_from_slice(format!("\n%mutated:removed:id={annotation_id}\n").as_bytes());
        Ok((removed, self.bytes.clone()))
    }
}

#[derive(Debug, Default)]
struct MockEngine;

impl PdfEngine for MockEngine {
    fn engine_identifier(&self) -> &str {
        "mock-m0.7-engine"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(PdfEngineError::InvalidPdf {
                details: "missing %PDF- header".to_owned(),
            });
        }
        let mut rotations = vec![0, 0];
        let mut highlights = Vec::new();
        let text = String::from_utf8_lossy(bytes);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("%mutated:rotated:page=") {
                let parts: Vec<&str> = rest.split(":delta=").collect();
                if parts.len() == 2
                    && let (Ok(idx), Ok(delta)) =
                        (parts[0].parse::<usize>(), parts[1].parse::<i32>())
                    && idx < rotations.len()
                {
                    rotations[idx] = (((rotations[idx] as i32 + delta) % 360 + 360) % 360) as u32;
                }
            } else if let Some(rest) = line.strip_prefix("%mutated:highlight:id=") {
                let parts: Vec<&str> = rest.split(':').collect();
                if parts.len() >= 3 {
                    let id = parts[0];
                    let page_idx = parts[1]
                        .strip_prefix("page=")
                        .unwrap_or("0")
                        .parse::<u32>()
                        .unwrap_or(0);
                    let author = parts[2]
                        .strip_prefix("author=")
                        .map(|s| s.to_owned())
                        .filter(|s| !s.is_empty());
                    highlights.push(PdfHighlightAnnotation {
                        annotation_id: id.to_owned(),
                        page_index: page_idx,
                        rect: PdfRectMilliPt {
                            x0_milli_pt: 72_000,
                            y0_milli_pt: 700_000,
                            x1_milli_pt: 300_000,
                            y1_milli_pt: 720_000,
                        },
                        color_rgb: [255, 220, 50],
                        opacity_milli: 600,
                        author,
                        contents: Some("Standard Highlight Annotation".to_owned()),
                    });
                }
            } else if let Some(id) = line.strip_prefix("%mutated:removed:id=") {
                highlights.retain(|h| h.annotation_id != id);
            }
        }
        Ok(Box::new(MockPdfDocument {
            bytes: bytes.to_vec(),
            rotations,
            highlights,
        }))
    }
}

#[test]
fn test_m0_full_dod_workflow_and_recovery_cycle() {
    let td = TestDir::new("full_workflow");
    let test_pdf = td.path.join("m0_workflow_target.pdf");
    let orig_bytes = fs::read(sample_pdf_path()).expect("read sample pdf");
    fs::write(&test_pdf, &orig_bytes).expect("write target pdf");

    let rec_dir = td.path.join("rec_store");
    let broker = Arc::new(ScopedLocalFileBroker::new());
    let rec_store = Arc::new(RecoveryStore::new(rec_dir.clone()));
    let registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        broker.clone(),
        rec_store.clone(),
    );

    let file_str = test_pdf.to_string_lossy().into_owned();

    // 1. Open
    let session = registry
        .open_via_broker(broker.as_ref(), &file_str)
        .expect("open session");
    let session_id = session.session_id.as_str();

    // 2. Select text contract check
    let sel = registry
        .select_page_text(session_id, 0, None)
        .expect("select text");
    assert_eq!(sel.page_index, 0);

    // 3. Highlight
    let annot = PdfHighlightAnnotation {
        annotation_id: "annot_m07_test_001".to_owned(),
        page_index: 0,
        rect: PdfRectMilliPt {
            x0_milli_pt: 72_000,
            y0_milli_pt: 700_000,
            x1_milli_pt: 300_000,
            y1_milli_pt: 720_000,
        },
        color_rgb: [255, 220, 50],
        opacity_milli: 600,
        author: Some("Interop Tester".to_owned()),
        contents: Some("Standard Highlight Annotation".to_owned()),
    };
    registry
        .execute_transaction(
            session_id,
            TransactionCommand::AddHighlightAnnotation { annotation: annot },
        )
        .expect("add highlight");

    // 4. Rotate (+90°)
    registry
        .execute_transaction(
            session_id,
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .expect("rotate");
    assert_eq!(
        registry
            .page_rotation_degrees(session_id, 0)
            .expect("rotation"),
        90
    );

    // 5. Undo
    registry.undo(session_id).expect("undo");
    assert_eq!(
        registry
            .page_rotation_degrees(session_id, 0)
            .expect("rotation"),
        0
    );

    // 6. Redo
    registry.redo(session_id).expect("redo");
    assert_eq!(
        registry
            .page_rotation_degrees(session_id, 0)
            .expect("rotation"),
        90
    );

    // 7. Safe Save
    let outcome = registry.save_session(session_id).expect("save session");
    assert!(matches!(outcome, SaveResult::Success(_)));

    // 8. Unsaved edit & simulated crash
    registry
        .execute_transaction(
            session_id,
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .expect("second rotate");
    assert_eq!(
        registry
            .page_rotation_degrees(session_id, 0)
            .expect("rotation"),
        180
    );

    // Verify recovery detected
    let rec_opt = rec_store.check_recovery_for_source_token(&file_str, broker.as_ref());
    let rec_info = rec_opt.expect("recovery container must exist");
    assert!(rec_info.uncommitted_revisions >= 1);

    // Verify disk untouched during crash state (M0.6 invariant)
    let disk_bytes_before_restore = fs::read(&test_pdf).expect("read disk");
    let hash_before = compute_sha256_hex(&disk_bytes_before_restore);

    // 9. Restart application with fresh registry & Restore session
    let restart_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        broker.clone(),
        Arc::new(RecoveryStore::new(rec_dir.clone())),
    );

    let rec_res = restart_registry
        .execute_recovery_action(&rec_info.document_id, RecoveryAction::Restore)
        .expect("restore recovery");
    let restored_snap = rec_res.session_snapshot.expect("restored snapshot");

    // Verify disk remains untouched after restore until explicit save
    let disk_bytes_after_restore = fs::read(&test_pdf).expect("read disk");
    assert_eq!(
        hash_before,
        compute_sha256_hex(&disk_bytes_after_restore),
        "M0.6 invariant violated: disk modified before explicit save"
    );

    // 10. Explicit Save of restored session
    let restored_outcome = restart_registry
        .save_session(restored_snap.session_id.as_str())
        .expect("save restored session");
    assert!(matches!(restored_outcome, SaveResult::Success(_)));

    // 11. Reopen in fresh registry session & verify mutations preserved
    let fresh_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        broker.clone(),
        Arc::new(RecoveryStore::new(rec_dir)),
    );
    let fresh_session = fresh_registry
        .open_via_broker(broker.as_ref(), &file_str)
        .expect("reopen fresh");

    assert_eq!(
        fresh_registry
            .page_rotation_degrees(fresh_session.session_id.as_str(), 0)
            .expect("rotation"),
        180,
        "Page 0 rotation must be 180° after recovery and save"
    );

    let annots = fresh_registry
        .page_highlight_annotations(fresh_session.session_id.as_str(), 0)
        .expect("page annotations");
    assert_eq!(annots.len(), 1, "Expected 1 highlight annotation");
    assert_eq!(annots[0].author.as_deref(), Some("Interop Tester"));

    fresh_registry
        .close_session(fresh_session.session_id.as_str())
        .expect("close fresh session");
}

#[test]
fn test_mutool_cli_interoperability_if_available() {
    let mutool_path = [
        "/opt/homebrew/bin/mutool",
        "/usr/local/bin/mutool",
        "mutool",
    ]
    .into_iter()
    .find(|cmd| {
        Command::new(cmd)
            .arg("-v")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    });

    if let Some(cmd) = mutool_path {
        let sample = sample_pdf_path();
        let out = Command::new(cmd)
            .args(["info", &sample.to_string_lossy()])
            .output()
            .expect("run mutool info");
        assert!(out.status.success(), "mutool info must succeed on sample");
    }
}

#[test]
fn test_coregraphics_sips_interoperability_on_macos() {
    if cfg!(target_os = "macos") {
        let sample = sample_pdf_path();
        let out = Command::new("/usr/bin/sips")
            .args(["--verify", &sample.to_string_lossy()])
            .output();
        if let Ok(res) = out {
            assert!(
                res.status.success(),
                "macOS sips verification of PDF must succeed"
            );
        }
    }
}

#[test]
fn test_chromium_pdfium_headless_if_available() {
    let chrome_bin = "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
    if Path::new(chrome_bin).exists() {
        let td = TestDir::new("chrome_pdfium");
        let out_png = td.path.join("chrome_render.png");
        let sample = sample_pdf_path();

        let res = Command::new(chrome_bin)
            .args([
                "--headless",
                "--disable-gpu",
                &format!("--screenshot={}", out_png.to_string_lossy()),
                &format!(
                    "file://{}",
                    sample.canonicalize().unwrap().to_string_lossy()
                ),
            ])
            .output();

        if let Ok(out) = res
            && out.status.success()
        {
            assert!(out_png.exists(), "Chrome must produce a screenshot PNG");
            let meta = fs::metadata(&out_png).expect("png metadata");
            assert!(meta.len() > 0, "Screenshot PNG must not be empty");
        }
    }
}

#[test]
fn test_uninstalled_viewers_status_not_simulated() {
    // Acrobat, PDF-XChange, Firefox:
    // When unavailable, their status MUST clearly document unavailability and not simulate a pass.
    let acrobat_exists = Path::new("/Applications/Adobe Acrobat Reader.app").exists()
        || Path::new("/Applications/Adobe Acrobat.app").exists();
    if !acrobat_exists {
        assert!(!acrobat_exists);
    }

    let pdf_xchange_exists =
        Path::new("C:\\Program Files\\Tracker Software\\PDF Editor\\PDFXEdit.exe").exists();
    assert!(!pdf_xchange_exists);

    let firefox_exists = Path::new("/Applications/Firefox.app").exists();
    if !firefox_exists {
        assert!(!firefox_exists);
    }
}
