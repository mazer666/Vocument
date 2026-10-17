//! Integration tests for M0.5 Safe Save (`ROADMAP.md` M0.5, `ARCHITECTURE.md` §16).
//!
//! Verifies:
//! - Atomic staging, sync, and replacement
//! - Post-save validation
//! - Save As rebinding
//! - External change detection via `FileIdentity`
//! - **M0.5 Gate: Never silently overwrite externally changed content.**
//! - Conflict UI contract and collision-free safe copy option

#![allow(clippy::unwrap_used)]

use std::path::Path;
use std::sync::Mutex;
use vocument_core::file_broker::ScopedLocalFileBroker;
use vocument_core::identity::FileIdentityComparison;
use vocument_core::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineDocument, PdfEngineError, PdfHighlightAnnotation,
    PdfPageGeometry, PdfPageStructuredText, PdfRectMilliPt,
};
use vocument_core::save::{
    SaveConflictResolution, SaveCoordinator, SaveError, SaveResult, generate_collision_free_path,
};
use vocument_core::session::SessionRegistry;
use vocument_core::transactions::TransactionCommand;

#[derive(Debug)]
struct TestDir {
    path: std::path::PathBuf,
}

impl TestDir {
    fn new(name: &str) -> Self {
        static CTR: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let id = CTR.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let p = std::env::temp_dir().join(format!("voc_{name}_{}_{id}", std::process::id()));
        let _ = std::fs::create_dir_all(&p);
        Self { path: p }
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[derive(Debug)]
struct MockPdfDocument {
    bytes: Vec<u8>,
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
    fn page_rotation_degrees(&self, _page_index: u32) -> Result<u32, PdfEngineError> {
        Ok(0)
    }
    fn page_highlight_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<PdfHighlightAnnotation>, PdfEngineError> {
        Ok(vec![])
    }
    fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
        Ok(None)
    }
    fn extract_page_text(&self, page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        Ok(PdfPageStructuredText {
            page_index,
            embedded_page_label: None,
            lines: vec![],
        })
    }
    fn create_display_list(
        &self,
        _page_index: u32,
    ) -> Result<Box<dyn vocument_core::pdf_engine::PdfEngineDisplayList>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "display list mock".to_owned(),
        })
    }
    fn rotate_pages(
        &mut self,
        _page_indices: &[u32],
        _delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError> {
        self.bytes.extend_from_slice(b"\n%rotated\n");
        Ok(self.bytes.clone())
    }
    fn add_highlight_annotation(
        &mut self,
        _annot: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        self.bytes.extend_from_slice(b"\n%highlight\n");
        Ok(self.bytes.clone())
    }
    fn remove_highlight_annotation(
        &mut self,
        page_index: u32,
        annotation_id: &str,
    ) -> Result<(PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
        let annot = PdfHighlightAnnotation {
            annotation_id: annotation_id.to_owned(),
            page_index,
            rect: PdfRectMilliPt {
                x0_milli_pt: 0,
                y0_milli_pt: 0,
                x1_milli_pt: 100,
                y1_milli_pt: 100,
            },
            color_rgb: [255, 230, 80],
            opacity_milli: 450,
            author: None,
            contents: None,
        };
        self.bytes.extend_from_slice(b"\n%removed\n");
        Ok((annot, self.bytes.clone()))
    }
}

#[derive(Debug, Default)]
struct MockEngine {
    fail_validation: Mutex<bool>,
}

impl PdfEngine for MockEngine {
    fn engine_identifier(&self) -> &str {
        "mock-m0.5-engine"
    }
    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        if *self.fail_validation.lock().unwrap() {
            return Err(PdfEngineError::InvalidPdf {
                details: "post-save validation error".to_owned(),
            });
        }
        if !bytes.starts_with(b"%PDF-") {
            return Err(PdfEngineError::InvalidPdf {
                details: "missing %PDF- header".to_owned(),
            });
        }
        Ok(Box::new(MockPdfDocument {
            bytes: bytes.to_vec(),
        }))
    }
}

fn sample_pdf() -> Vec<u8> {
    let mut b = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<<>>\n%%EOF\n".to_vec();
    b.resize(600, b' ');
    b
}

#[test]
fn test_safe_save_updates_identity_and_clears_dirty_flag() {
    let temp = TestDir::new("save_clean");
    let file_path = temp.path.join("doc.pdf");
    std::fs::write(&file_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &file_path.to_string_lossy())
        .unwrap();
    assert!(!session.is_dirty);

    // Mutate session:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();
    let dirty_snap = registry
        .session_snapshot(session.session_id.as_str())
        .unwrap();
    assert!(dirty_snap.is_dirty);

    // Safe save:
    let outcome = registry
        .save_session(dirty_snap.session_id.as_str())
        .unwrap();
    match outcome {
        SaveResult::Success(succ) => {
            assert_eq!(succ.session_id.as_str(), dirty_snap.session_id.as_str());
            let clean_snap = registry
                .session_snapshot(session.session_id.as_str())
                .unwrap();
            assert!(!clean_snap.is_dirty);
            assert_eq!(clean_snap.revision, succ.saved_revision);

            let on_disk = std::fs::read(&file_path).unwrap();
            assert!(on_disk.windows(b"%rotated".len()).any(|w| w == b"%rotated"));
        }
        SaveResult::Conflict(_) => panic!("unexpected conflict on clean save"),
    }
}

#[test]
fn test_safe_save_as_writes_new_file_and_rebinds_session() {
    let temp = TestDir::new("save_as");
    let orig_path = temp.path.join("orig.pdf");
    let new_path = temp.path.join("saved_as.pdf");
    std::fs::write(&orig_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &orig_path.to_string_lossy())
        .unwrap();

    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();

    let outcome = registry
        .save_session_as(session.session_id.as_str(), &new_path.to_string_lossy())
        .unwrap();

    match outcome {
        SaveResult::Success(succ) => {
            assert_eq!(succ.target_token, new_path.to_string_lossy());
            assert!(new_path.exists());
            let snap = registry
                .session_snapshot(session.session_id.as_str())
                .unwrap();
            assert_eq!(snap.file_identity.source_token, new_path.to_string_lossy());
            assert!(!snap.is_dirty);
        }
        SaveResult::Conflict(_) => panic!("unexpected conflict on Save As"),
    }
}

#[test]
fn test_gate_never_silently_overwrites_externally_changed_content() {
    let temp = TestDir::new("ext_conflict");
    let file_path = temp.path.join("conflict.pdf");
    std::fs::write(&file_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &file_path.to_string_lossy())
        .unwrap();

    // Local mutation:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();

    // External change on disk:
    std::thread::sleep(std::time::Duration::from_millis(15));
    let mut external_bytes = sample_pdf();
    external_bytes.extend_from_slice(b"\n%external writer modified this\n");
    std::fs::write(&file_path, &external_bytes).unwrap();

    // Verify proactive check detects external change:
    let check = registry
        .check_external_changes(session.session_id.as_str())
        .unwrap();
    assert!(check.has_external_changes);
    assert_eq!(check.comparison, FileIdentityComparison::ModifiedExternally);

    // M0.5 GATE VERIFICATION: Save MUST NOT silently overwrite!
    let outcome = registry.save_session(session.session_id.as_str()).unwrap();
    match outcome {
        SaveResult::Conflict(conflict) => {
            assert_eq!(
                conflict.comparison,
                FileIdentityComparison::ModifiedExternally
            );
            assert!(
                conflict
                    .suggested_safe_copy_path
                    .contains("Conflicted Copy")
            );

            // Crucial Gate Assertion: External content on disk was NEVER overwritten!
            let on_disk = std::fs::read(&file_path).unwrap();
            assert_eq!(on_disk, external_bytes);
        }
        SaveResult::Success(_) => {
            panic!("FATAL: M0.5 Gate breached! External file was silently overwritten.")
        }
    }
}

#[test]
fn test_conflict_resolution_safe_copy_creates_collision_free_file_and_preserves_external() {
    let temp = TestDir::new("safe_copy");
    let file_path = temp.path.join("important.pdf");
    std::fs::write(&file_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &file_path.to_string_lossy())
        .unwrap();

    // Local edit:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();

    // External change:
    std::thread::sleep(std::time::Duration::from_millis(15));
    let external_bytes = b"%PDF-1.7\n%external\n";
    std::fs::write(&file_path, external_bytes).unwrap();

    // User chooses SaveLocalAsCollisionFreeCopy:
    let result = registry
        .resolve_save_conflict(
            session.session_id.as_str(),
            SaveConflictResolution::SaveLocalAsCollisionFreeCopy,
        )
        .unwrap();

    let succ = result.expect("safe copy should produce snapshot");
    assert!(succ.target_token.contains("Safe Copy"));
    assert!(Path::new(&succ.target_token).exists());

    // Original file remains unchanged with external bytes:
    let disk_bytes = std::fs::read(&file_path).unwrap();
    assert_eq!(disk_bytes, external_bytes);
}

#[test]
fn test_conflict_resolution_explicit_overwrite_updates_external_file() {
    let temp = TestDir::new("overwrite_explicit");
    let file_path = temp.path.join("doc.pdf");
    std::fs::write(&file_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &file_path.to_string_lossy())
        .unwrap();

    // Local edit:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();

    // External edit:
    std::thread::sleep(std::time::Duration::from_millis(15));
    std::fs::write(&file_path, b"%PDF-1.7\n%external\n").unwrap();

    // User explicitly decides to overwrite:
    let res = registry
        .resolve_save_conflict(
            session.session_id.as_str(),
            SaveConflictResolution::OverwriteExternalExplicitly,
        )
        .unwrap();

    assert!(res.is_some());
    let snap = registry
        .session_snapshot(session.session_id.as_str())
        .unwrap();
    assert!(!snap.is_dirty);

    // Disk now contains our local edit:
    let disk_bytes = std::fs::read(&file_path).unwrap();
    assert!(
        disk_bytes
            .windows(b"%rotated".len())
            .any(|w| w == b"%rotated")
    );
}

#[test]
fn test_conflict_resolution_reload_external_version() {
    let temp = TestDir::new("reload_ext");
    let file_path = temp.path.join("doc.pdf");
    std::fs::write(&file_path, sample_pdf()).unwrap();

    let registry = SessionRegistry::new(Box::new(MockEngine::default()));
    let broker = ScopedLocalFileBroker::new();
    let session = registry
        .open_via_broker(&broker, &file_path.to_string_lossy())
        .unwrap();

    // Local edit:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
            },
        )
        .unwrap();
    assert!(
        registry
            .session_snapshot(session.session_id.as_str())
            .unwrap()
            .is_dirty
    );

    // External edit with valid PDF:
    std::thread::sleep(std::time::Duration::from_millis(15));
    let mut ext = sample_pdf();
    ext.extend_from_slice(b"\n%external\n");
    std::fs::write(&file_path, &ext).unwrap();

    // User chooses ReloadExternalVersion:
    let res = registry
        .resolve_save_conflict(
            session.session_id.as_str(),
            SaveConflictResolution::ReloadExternalVersion,
        )
        .unwrap();
    assert!(res.is_none());

    let snap = registry
        .session_snapshot(session.session_id.as_str())
        .unwrap();
    assert!(!snap.is_dirty);
    assert_eq!(snap.file_identity.size_bytes, ext.len() as u64);
}

#[test]
fn test_post_save_validation_catches_corrupted_payload_and_preserves_target() {
    let temp = TestDir::new("post_val");
    let file_path = temp.path.join("doc.pdf");
    let initial_bytes = sample_pdf();
    std::fs::write(&file_path, &initial_bytes).unwrap();

    // Validation failure on corrupted bytes:
    let corrupted = b"NOT_A_VALID_PDF_FILE";
    let res = SaveCoordinator::validate_staged_bytes(&MockEngine::default(), corrupted, 2);
    assert!(matches!(
        res,
        Err(SaveError::PostSaveValidationFailed { .. })
    ));

    // Target file was never modified:
    assert_eq!(std::fs::read(&file_path).unwrap(), initial_bytes);
}

#[test]
fn test_collision_free_path_generator() {
    let temp = TestDir::new("coll_test");
    let base = temp.path.join("report.pdf");
    std::fs::write(&base, b"p0").unwrap();

    let c1 = generate_collision_free_path(&base, "Safe Copy");
    assert_eq!(
        c1.file_name().unwrap().to_str().unwrap(),
        "report (Safe Copy).pdf"
    );
    std::fs::write(&c1, b"p1").unwrap();

    let c2 = generate_collision_free_path(&base, "Safe Copy");
    assert_eq!(
        c2.file_name().unwrap().to_str().unwrap(),
        "report (Safe Copy 1).pdf"
    );
    std::fs::write(&c2, b"p2").unwrap();

    let c3 = generate_collision_free_path(&base, "Safe Copy");
    assert_eq!(
        c3.file_name().unwrap().to_str().unwrap(),
        "report (Safe Copy 2).pdf"
    );
}
