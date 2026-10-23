//! Integration tests for M0.6 Crash Recovery (`ROADMAP.md` M0.6, `ARCHITECTURE.md` §§11–12, `REQUIREMENTS.md` §34).
//!
//! Verifies:
//! - Per-document recovery container lifecycle
//! - Journaling and periodic checkpoints
//! - Crash detection via PID and `is_clean_shutdown`
//! - Recovery actions: Restore, Preview, Discard, and Open as New (`RECOVERY-001` Preset)
//! - **Required destructive test:**
//!   1. open document
//!   2. make unsaved edit
//!   3. hard-kill process (SIGKILL)
//!   4. restart
//!   5. detect recovery
//!   6. preview/restore
//!   7. confirm original remains intact until explicit save.

#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use vocument_core::file_broker::ScopedLocalFileBroker;
use vocument_core::identity::compute_sha256_hex;
use vocument_core::pdf_engine::{
    PdfCommentAnnotation, PdfDocumentSummary, PdfEngine, PdfEngineDocument, PdfEngineError,
    PdfHighlightAnnotation, PdfPageGeometry, PdfPageStructuredText, PdfRectMilliPt,
};
use vocument_core::recovery::{RecoveryAction, RecoveryStore};
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
        let p = std::env::temp_dir().join(format!("voc_rec_{name}_{}_{id}", std::process::id()));
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
    ) -> Result<Box<dyn vocument_core::pdf_engine::PdfEngineDisplayList>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "mock display list".to_owned(),
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
        }
        self.bytes.extend_from_slice(b"\n%mutated:rotated\n");
        Ok(self.bytes.clone())
    }

    fn add_highlight_annotation(
        &mut self,
        annot: &PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        self.highlights.push(annot.clone());
        self.bytes.extend_from_slice(b"\n%mutated:highlight\n");
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
        self.bytes.extend_from_slice(b"\n%mutated:removed\n");
        Ok((removed, self.bytes.clone()))
    }
}

#[derive(Debug, Default)]
struct MockEngine;

impl PdfEngine for MockEngine {
    fn engine_identifier(&self) -> &str {
        "mock-m0.6-engine"
    }

    fn open_pdf_bytes(&self, bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        if !bytes.starts_with(b"%PDF-") {
            return Err(PdfEngineError::InvalidPdf {
                details: "missing %PDF- header".to_owned(),
            });
        }
        Ok(Box::new(MockPdfDocument {
            bytes: bytes.to_vec(),
            rotations: vec![0, 0],
            highlights: vec![],
        }))
    }
}

fn sample_pdf() -> Vec<u8> {
    let mut b = b"%PDF-1.7\n1 0 obj\n<< /Type /Catalog >>\nendobj\ntrailer\n<<>>\n%%EOF\n".to_vec();
    b.resize(600, b' ');
    b
}

#[test]
fn test_recovery_container_lifecycle_dirty_session_and_clean_close() {
    let temp = TestDir::new("lifecycle");
    let rec_store = Arc::new(RecoveryStore::new(temp.path.join("recovery")));
    let broker = Arc::new(ScopedLocalFileBroker::new());
    let registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
        Arc::clone(&rec_store),
    );

    let doc_path = temp.path.join("doc.pdf");
    std::fs::write(&doc_path, sample_pdf()).unwrap();

    let session = registry
        .open_via_broker(broker.as_ref(), doc_path.to_str().unwrap())
        .unwrap();
    let doc_id = session.document_id.clone();

    // Mutate session:
    registry
        .execute_transaction(
            session.session_id.as_str(),
            TransactionCommand::RotatePages {
                page_indices: vec![0],
                delta_degrees: 90,
                allow_signature_invalidation: false,
            },
        )
        .unwrap();

    let container = rec_store.container_for(doc_id.as_str());
    assert!(container.exists());
    let manifest = container.read_manifest().unwrap().expect("manifest exists");
    assert!(!manifest.is_clean_shutdown);
    assert!(manifest.has_unsaved_changes);
    assert_eq!(manifest.uncommitted_revisions, 2);

    // Clean close:
    registry.close_session(session.session_id.as_str()).unwrap();
    // Re-check manifest:
    let manifest_after = container.read_manifest().unwrap().expect("manifest exists");
    assert!(manifest_after.is_clean_shutdown);
}

#[test]
fn test_crash_recovery_detection_and_restore_action_gate_verified() {
    let temp = TestDir::new("restore_gate");
    let rec_dir = temp.path.join("recovery");
    let rec_store = Arc::new(RecoveryStore::new(rec_dir.clone()));
    let broker = Arc::new(ScopedLocalFileBroker::new());

    let doc_path = temp.path.join("contract.pdf");
    let initial_bytes = sample_pdf();
    std::fs::write(&doc_path, &initial_bytes).unwrap();
    let initial_hash = compute_sha256_hex(&initial_bytes);

    let doc_id = {
        let registry = SessionRegistry::with_broker_and_recovery(
            Box::new(MockEngine),
            Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
            Arc::clone(&rec_store),
        );
        let session = registry
            .open_via_broker(broker.as_ref(), doc_path.to_str().unwrap())
            .unwrap();

        // Perform 2 mutations:
        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::RotatePages {
                    page_indices: vec![0],
                    delta_degrees: 90,
                    allow_signature_invalidation: false,
                },
            )
            .unwrap();

        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::AddHighlightAnnotation {
                    annotation: PdfHighlightAnnotation {
                        annotation_id: "annot_rec_001".to_owned(),
                        page_index: 0,
                        rect: PdfRectMilliPt {
                            x0_milli_pt: 10,
                            y0_milli_pt: 10,
                            x1_milli_pt: 50,
                            y1_milli_pt: 50,
                        },
                        color_rgb: [255, 230, 80],
                        opacity_milli: 450,
                        author: Some("Tester".to_owned()),
                        contents: Some("Crash test highlight".to_owned()),
                    },
                },
            )
            .unwrap();

        session.document_id.clone()
        // Abruptly drop registry without saving or closing -> SIMULATES CRASH!
    };

    // M0.6 Gate verification 1: On-disk original file was NOT modified!
    let disk_bytes_after_crash = std::fs::read(&doc_path).unwrap();
    assert_eq!(
        compute_sha256_hex(&disk_bytes_after_crash),
        initial_hash,
        "GATE BREACH: Original file was modified before explicit save!"
    );

    // Restart application with new SessionRegistry:
    let new_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
        Arc::new(RecoveryStore::new(rec_dir)),
    );

    // Scan for unrecovered crashed documents:
    let recoverable = new_registry
        .recovery_store()
        .scan_recoverable_documents(broker.as_ref());
    assert_eq!(recoverable.len(), 1);
    let info = &recoverable[0];
    assert_eq!(info.document_id, doc_id.as_str());
    assert_eq!(info.uncommitted_revisions, 3);
    assert!(info.has_journal);

    // Execute RecoveryAction::Restore:
    let outcome = new_registry
        .execute_recovery_action(doc_id.as_str(), RecoveryAction::Restore)
        .unwrap();
    assert_eq!(outcome.action, RecoveryAction::Restore);
    let snap = outcome.session_snapshot.expect("restored snapshot");
    assert!(snap.is_dirty);
    assert_eq!(snap.revision, 3);

    // Verify mutations are present in restored session:
    assert_eq!(
        new_registry
            .page_rotation_degrees(snap.session_id.as_str(), 0)
            .unwrap(),
        90
    );
    let annots = new_registry
        .page_highlight_annotations(snap.session_id.as_str(), 0)
        .unwrap();
    assert_eq!(annots.len(), 1);
    assert_eq!(annots[0].annotation_id, "annot_rec_001");

    // M0.6 Gate verification 2: On-disk file STILL unchanged after restore!
    let disk_bytes_after_restore = std::fs::read(&doc_path).unwrap();
    assert_eq!(compute_sha256_hex(&disk_bytes_after_restore), initial_hash);

    // Explicit user save:
    let save_res = new_registry.save_session(snap.session_id.as_str()).unwrap();
    assert!(matches!(
        save_res,
        vocument_core::save::SaveResult::Success(_)
    ));

    // Now disk file contains our restored changes:
    let disk_bytes_final = std::fs::read(&doc_path).unwrap();
    assert_ne!(compute_sha256_hex(&disk_bytes_final), initial_hash);
    assert!(
        disk_bytes_final
            .windows(b"%mutated:highlight".len())
            .any(|w| w == b"%mutated:highlight")
    );
}

#[test]
fn test_crash_recovery_preview_action() {
    let temp = TestDir::new("preview");
    let rec_store = Arc::new(RecoveryStore::new(temp.path.join("recovery")));
    let broker = Arc::new(ScopedLocalFileBroker::new());

    let doc_path = temp.path.join("preview.pdf");
    std::fs::write(&doc_path, sample_pdf()).unwrap();

    let doc_id = {
        let registry = SessionRegistry::with_broker_and_recovery(
            Box::new(MockEngine),
            Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
            Arc::clone(&rec_store),
        );
        let session = registry
            .open_via_broker(broker.as_ref(), doc_path.to_str().unwrap())
            .unwrap();

        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::RotatePages {
                    page_indices: vec![0],
                    delta_degrees: 90,
                    allow_signature_invalidation: false,
                },
            )
            .unwrap();

        session.document_id.clone()
    };

    let new_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
        Arc::clone(&rec_store),
    );

    let res = new_registry
        .execute_recovery_action(doc_id.as_str(), RecoveryAction::Preview)
        .unwrap();
    assert_eq!(res.action, RecoveryAction::Preview);
    let snap = res.session_snapshot.expect("preview snapshot");
    assert_eq!(
        new_registry
            .page_rotation_degrees(snap.session_id.as_str(), 0)
            .unwrap(),
        90
    );

    // Recovery container remains present and unrecovered:
    assert!(rec_store.container_for(doc_id.as_str()).exists());
}

#[test]
fn test_crash_recovery_discard_action() {
    let temp = TestDir::new("discard");
    let rec_store = Arc::new(RecoveryStore::new(temp.path.join("recovery")));
    let broker = Arc::new(ScopedLocalFileBroker::new());

    let doc_path = temp.path.join("discard.pdf");
    std::fs::write(&doc_path, sample_pdf()).unwrap();

    let doc_id = {
        let registry = SessionRegistry::with_broker_and_recovery(
            Box::new(MockEngine),
            Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
            Arc::clone(&rec_store),
        );
        let session = registry
            .open_via_broker(broker.as_ref(), doc_path.to_str().unwrap())
            .unwrap();

        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::RotatePages {
                    page_indices: vec![0],
                    delta_degrees: 90,
                    allow_signature_invalidation: false,
                },
            )
            .unwrap();

        session.document_id.clone()
    };

    let new_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
        Arc::clone(&rec_store),
    );

    let res = new_registry
        .execute_recovery_action(doc_id.as_str(), RecoveryAction::Discard)
        .unwrap();
    assert_eq!(res.action, RecoveryAction::Discard);

    // Container is purged:
    assert!(!rec_store.container_for(doc_id.as_str()).exists());
}

#[test]
fn test_crash_recovery_open_as_new_action() {
    let temp = TestDir::new("open_as_new");
    let rec_store = Arc::new(RecoveryStore::new(temp.path.join("recovery")));
    let broker = Arc::new(ScopedLocalFileBroker::new());

    let doc_path = temp.path.join("original.pdf");
    let initial_bytes = sample_pdf();
    std::fs::write(&doc_path, &initial_bytes).unwrap();
    let initial_hash = compute_sha256_hex(&initial_bytes);

    let doc_id = {
        let registry = SessionRegistry::with_broker_and_recovery(
            Box::new(MockEngine),
            Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
            Arc::clone(&rec_store),
        );
        let session = registry
            .open_via_broker(broker.as_ref(), doc_path.to_str().unwrap())
            .unwrap();

        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::RotatePages {
                    page_indices: vec![0],
                    delta_degrees: 90,
                    allow_signature_invalidation: false,
                },
            )
            .unwrap();

        session.document_id.clone()
    };

    let new_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        Arc::clone(&broker) as Arc<dyn vocument_core::file_broker::FileBroker>,
        Arc::clone(&rec_store),
    );

    let res = new_registry
        .execute_recovery_action(doc_id.as_str(), RecoveryAction::OpenAsNew)
        .unwrap();
    assert_eq!(res.action, RecoveryAction::OpenAsNew);
    let target = res.target_token.expect("new target token");
    assert!(target.contains("Recovered"));
    assert!(Path::new(&target).exists());

    // Original file remains completely intact:
    let disk_orig = std::fs::read(&doc_path).unwrap();
    assert_eq!(compute_sha256_hex(&disk_orig), initial_hash);

    // Old recovery container is purged:
    assert!(!rec_store.container_for(doc_id.as_str()).exists());
}

#[test]
fn test_hard_kill_recovery_with_subprocess_sigkill() {
    // 1. Setup paths:
    let temp = TestDir::new("hard_kill");
    let rec_dir = temp.path.join("recovery");
    let doc_path = temp.path.join("kill_test.pdf");
    let ready_marker = temp.path.join("child_ready.marker");
    let initial_bytes = sample_pdf();
    std::fs::write(&doc_path, &initial_bytes).unwrap();
    let initial_hash = compute_sha256_hex(&initial_bytes);

    // If we are the child process, run the crash victim routine:
    if std::env::var("VOC_TEST_RUN_CRASH_VICTIM").is_ok() {
        let rec_path = PathBuf::from(std::env::var("VOC_REC_DIR").unwrap());
        let test_doc = PathBuf::from(std::env::var("VOC_DOC_PATH").unwrap());
        let marker = PathBuf::from(std::env::var("VOC_MARKER").unwrap());

        let rec_store = Arc::new(RecoveryStore::new(rec_path));
        let broker = Arc::new(ScopedLocalFileBroker::new());
        let registry = SessionRegistry::with_broker_and_recovery(
            Box::new(MockEngine),
            broker.clone(),
            rec_store,
        );

        let session = registry
            .open_via_broker(broker.as_ref(), test_doc.to_str().unwrap())
            .unwrap();

        // Perform edit:
        registry
            .execute_transaction(
                session.session_id.as_str(),
                TransactionCommand::RotatePages {
                    page_indices: vec![0],
                    delta_degrees: 90,
                    allow_signature_invalidation: false,
                },
            )
            .unwrap();

        // Signal readiness:
        std::fs::write(&marker, b"READY").unwrap();

        // Hang indefinitely awaiting SIGKILL:
        loop {
            std::thread::sleep(std::time::Duration::from_millis(50));
        }
    }

    // 2. Parent process: spawn child process running this test binary with victim flag:
    let current_exe = std::env::current_exe().unwrap();
    let mut child = std::process::Command::new(current_exe)
        .arg("--nocapture")
        .arg("test_hard_kill_recovery_with_subprocess_sigkill")
        .env("VOC_TEST_RUN_CRASH_VICTIM", "1")
        .env("VOC_REC_DIR", &rec_dir)
        .env("VOC_DOC_PATH", &doc_path)
        .env("VOC_MARKER", &ready_marker)
        .spawn()
        .unwrap();

    // Wait until child signals readiness:
    let start = std::time::Instant::now();
    while !ready_marker.exists() && start.elapsed() < std::time::Duration::from_secs(10) {
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
    assert!(ready_marker.exists(), "child failed to reach ready state");

    // 3. HARD-KILL child process via SIGKILL!
    child.kill().unwrap();
    let _ = child.wait();

    // 4. Verify original file on disk remained completely intact:
    let disk_bytes = std::fs::read(&doc_path).unwrap();
    assert_eq!(
        compute_sha256_hex(&disk_bytes),
        initial_hash,
        "GATE BREACH: Original file was corrupted during hard-kill!"
    );

    // 5. Restart: scan recovery store:
    let broker = Arc::new(ScopedLocalFileBroker::new());
    let rec_store = Arc::new(RecoveryStore::new(rec_dir));
    let new_registry = SessionRegistry::with_broker_and_recovery(
        Box::new(MockEngine),
        broker.clone(),
        rec_store.clone(),
    );

    let recoverable = rec_store.scan_recoverable_documents(broker.as_ref());
    assert_eq!(
        recoverable.len(),
        1,
        "Expected 1 recoverable crashed document"
    );
    let info = &recoverable[0];
    assert_eq!(info.source_token, doc_path.to_string_lossy());
    assert_eq!(info.uncommitted_revisions, 2);

    // 6. Restore recovered document:
    let restore_res = new_registry
        .execute_recovery_action(&info.document_id, RecoveryAction::Restore)
        .unwrap();
    let restored_snap = restore_res.session_snapshot.expect("restored snapshot");
    assert_eq!(restored_snap.revision, 2);
    assert_eq!(
        new_registry
            .page_rotation_degrees(restored_snap.session_id.as_str(), 0)
            .unwrap(),
        90
    );

    // Original file STILL intact on disk before save:
    assert_eq!(
        compute_sha256_hex(&std::fs::read(&doc_path).unwrap()),
        initial_hash
    );

    // Explicit user save:
    new_registry
        .save_session(restored_snap.session_id.as_str())
        .unwrap();

    // Now disk file contains changes:
    assert_ne!(
        compute_sha256_hex(&std::fs::read(&doc_path).unwrap()),
        initial_hash
    );
}
