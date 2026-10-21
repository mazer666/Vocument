//! Tests of the export, the conflict handling and the protection of the files
//! of the user (`SAVE-001`, `SAVE-002`, `FILELOCK-002`, `FILELOCK-004`).
//!
//! Three properties are under test:
//!
//! 1. A document that lives in a platform document provider must not be
//!    written through the local file broker — the URI would become a nonsense
//!    file name.
//! 2. The export hands the bytes of the document over, so that the platform
//!    shell can write them into a location that the user owns.
//! 3. The files that the application writes for the user — the staged output
//!    of a save and the recovery container — must not be readable for anyone
//!    else on the machine.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use vocument_core::file_broker::{BrokeredFileRead, FileAccessOrigin, FileBrokerError};
use vocument_core::pdf_engine::{
    PdfDocumentSummary, PdfEngine, PdfEngineDisplayList, PdfEngineDocument, PdfEngineError,
    PdfPageGeometry, PdfPageStructuredText,
};
use vocument_core::session::{SessionError, SessionRegistry};

/// A minimal engine that returns fixed bytes.
#[derive(Debug)]
struct StubEngine {
    bytes: Vec<u8>,
}

impl PdfEngine for StubEngine {
    fn open_pdf_bytes(&self, _bytes: &[u8]) -> Result<Box<dyn PdfEngineDocument>, PdfEngineError> {
        Ok(Box::new(StubDocument {
            bytes: self.bytes.clone(),
        }))
    }

    fn engine_identifier(&self) -> &str {
        "stub"
    }
}

#[derive(Debug)]
struct StubDocument {
    bytes: Vec<u8>,
}

impl PdfEngineDocument for StubDocument {
    fn summary(&self) -> &PdfDocumentSummary {
        static SUMMARY: std::sync::OnceLock<PdfDocumentSummary> = std::sync::OnceLock::new();
        SUMMARY.get_or_init(|| PdfDocumentSummary {
            page_count: 1,
            needs_password: false,
            format_version: Some("PDF 1.7".to_owned()),
            title: None,
            author: None,
            subject: None,
            keywords: None,
            creator: None,
            producer: None,
            creation_date: None,
            modification_date: None,
            engine_identifier: "stub".to_owned(),
        })
    }
    fn current_pdf_bytes(&self) -> &[u8] {
        &self.bytes
    }
    fn page_geometry(&self, _page_index: u32) -> Result<PdfPageGeometry, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
    fn page_rotation_degrees(&self, _page_index: u32) -> Result<u32, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
    fn page_highlight_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<vocument_core::pdf_engine::PdfHighlightAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
    /// The comments of the page (`ANNOT-001`). The test engine does not carry
    /// comments — it is a stand-in for the real engine, and it says so
    /// instead of pretending that it succeeded.
    fn page_comment_annotations(
        &self,
        _page_index: u32,
    ) -> Result<Vec<vocument_core::pdf_engine::PdfCommentAnnotation>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    /// Writes a comment (`ANNOT-001`).
    fn add_comment_annotation(
        &mut self,
        _annotation: &vocument_core::pdf_engine::PdfCommentAnnotation,
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
    ) -> Result<(vocument_core::pdf_engine::PdfCommentAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "the test engine does not carry comments".to_owned(),
        })
    }

    fn page_label(&self, _page_index: u32) -> Result<Option<String>, PdfEngineError> {
        Ok(None)
    }
    fn extract_page_text(&self, _page_index: u32) -> Result<PdfPageStructuredText, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
    fn create_display_list(
        &self,
        _page_index: u32,
    ) -> Result<Box<dyn PdfEngineDisplayList>, PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
    fn rotate_pages(
        &mut self,
        _page_indices: &[u32],
        _delta_degrees: i32,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Ok(self.bytes.clone())
    }
    fn add_highlight_annotation(
        &mut self,
        _annot: &vocument_core::pdf_engine::PdfHighlightAnnotation,
    ) -> Result<Vec<u8>, PdfEngineError> {
        Ok(self.bytes.clone())
    }
    fn remove_highlight_annotation(
        &mut self,
        _page_index: u32,
        _annotation_id: &str,
    ) -> Result<(vocument_core::pdf_engine::PdfHighlightAnnotation, Vec<u8>), PdfEngineError> {
        Err(PdfEngineError::EngineFailure {
            details: "not needed for this test".to_owned(),
        })
    }
}

/// A broker that records the paths it was asked to write to.
#[derive(Debug, Default)]
struct RecordingBroker {
    written: Arc<std::sync::Mutex<Vec<String>>>,
    counter: AtomicUsize,
}

impl vocument_core::file_broker::FileBroker for RecordingBroker {
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError> {
        Err(FileBrokerError::NotFound {
            source_token: source_token.to_owned(),
            details: "not needed for this test".to_owned(),
        })
    }

    fn inspect_file_identity(
        &self,
        _source_token: &str,
    ) -> Result<Option<vocument_core::identity::FileIdentity>, FileBrokerError> {
        Ok(None)
    }

    fn stage_write(
        &self,
        target_token: &str,
        bytes: &[u8],
    ) -> Result<vocument_core::file_broker::BrokeredStagedWrite, FileBrokerError> {
        if let Ok(mut guard) = self.written.lock() {
            guard.push(target_token.to_owned());
        }
        let counter = self.counter.fetch_add(1, Ordering::SeqCst);
        Ok(vocument_core::file_broker::BrokeredStagedWrite {
            staging_token: format!("/tmp/staging-{counter}"),
            target_token: target_token.to_owned(),
            staging_path: std::path::PathBuf::from(format!("/tmp/staging-{counter}")),
            target_path: std::path::PathBuf::from(target_token),
            bytes_written: bytes.len() as u64,
        })
    }

    fn commit_staged_write(
        &self,
        staged: vocument_core::file_broker::BrokeredStagedWrite,
    ) -> Result<vocument_core::identity::FileIdentity, FileBrokerError> {
        Err(FileBrokerError::IoError {
            source_token: staged.target_token,
            details: "not needed for this test".to_owned(),
        })
    }

    fn discard_staged_write(
        &self,
        _staged: vocument_core::file_broker::BrokeredStagedWrite,
    ) -> Result<(), FileBrokerError> {
        Ok(())
    }

    /// The mock stands in for the local broker: it can only write local
    /// files, not the URI of a document provider.
    fn can_write_back(&self, source_token: &str) -> bool {
        vocument_core::file_broker::ScopedLocalFileBroker::is_local_token(source_token)
    }
}

fn temp_dir(name: &str) -> PathBuf {
    let directory = std::env::temp_dir().join(format!("vocument-export-{name}"));
    let _ = std::fs::remove_dir_all(&directory);
    std::fs::create_dir_all(&directory).expect("the temporary directory must be creatable");
    directory
}

#[test]
fn a_document_of_a_platform_provider_is_not_written_through_the_local_broker() {
    let broker = Arc::new(RecordingBroker::default());
    let registry = SessionRegistry::with_broker(
        Box::new(StubEngine {
            bytes: b"%PDF-1.7\n".to_vec(),
        }),
        broker.clone(),
    );

    // A document that the user opened from the document provider of Android.
    let brokered = BrokeredFileRead {
        source_token: "content://com.android.providers/document/1".to_owned(),
        display_name: "Vertrag.pdf".to_owned(),
        access_origin: FileAccessOrigin::AndroidSafUri,
        os_file_id: Some("doc-1".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: b"%PDF-1.7\n".to_vec(),
    };
    let snapshot = registry
        .open_brokered_file(brokered)
        .expect("the document must be openable");
    let session_id = snapshot.session_id.as_str().to_owned();

    let result = registry.save_session(&session_id);
    match result {
        Err(SessionError::NotLocallyWritable { .. }) => {}
        other => panic!(
            "a document of a platform provider must not be saved through the local broker, was: {other:?}"
        ),
    }
    assert!(
        broker
            .written
            .lock()
            .map(|guard| guard.is_empty())
            .unwrap_or(false),
        "the broker must not have been asked to write anything"
    );
}

#[test]
fn the_export_hands_the_bytes_of_the_document_over() {
    let payload = b"%PDF-1.7\nexport payload\n%%EOF\n".to_vec();
    let broker = Arc::new(RecordingBroker::default());
    let registry = SessionRegistry::with_broker(
        Box::new(StubEngine {
            bytes: payload.clone(),
        }),
        broker,
    );

    let brokered = BrokeredFileRead {
        source_token: "content://com.android.providers/document/2".to_owned(),
        display_name: "Vertrag.pdf".to_owned(),
        access_origin: FileAccessOrigin::AndroidSafUri,
        os_file_id: Some("doc-2".to_owned()),
        modified_unix_ms: Some(1_700_000_000_000),
        bytes: payload.clone(),
    };
    let snapshot = registry
        .open_brokered_file(brokered)
        .expect("the document must be openable");
    let session_id = snapshot.session_id.as_str().to_owned();

    let exported = registry
        .export_session_bytes(&session_id)
        .expect("the export must succeed");
    assert_eq!(
        exported, payload,
        "the export must hand over the bytes of the document unchanged"
    );
    assert!(
        exported.starts_with(b"%PDF-"),
        "the exported bytes must be a PDF"
    );
}

#[test]
fn the_staged_output_of_a_save_is_readable_for_the_owner_only() {
    let directory = temp_dir("staging");
    let target = directory.join("document.pdf");
    let broker = vocument_core::file_broker::ScopedLocalFileBroker::new();

    let staged = vocument_core::file_broker::FileBroker::stage_write(
        &broker,
        target.to_string_lossy().as_ref(),
        b"%PDF-1.7\nsecret\n",
    )
    .expect("the staging must succeed");
    let staging_path = Path::new(&staged.staging_path);
    assert!(
        staging_path.exists(),
        "the staged file must exist while the save runs"
    );

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(staging_path)
            .expect("the staged file must be readable")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            mode, 0o600,
            "the staged file contains the document of the user and must not be readable for others, mode was {mode:o}"
        );
    }

    let _ = vocument_core::file_broker::FileBroker::discard_staged_write(&broker, staged);
    let _ = std::fs::remove_dir_all(&directory);
}

#[test]
fn the_recovery_container_is_readable_for_the_owner_only() {
    use vocument_core::recovery::{RecoveryContainer, RecoveryManifest};

    let directory = temp_dir("recovery");
    let container = RecoveryContainer::new(&directory, "document-1");
    let manifest = RecoveryManifest {
        schema_version: 1,
        document_id: "document-1".to_owned(),
        source_token: "/home/user/Vertraulichkeit.pdf".to_owned(),
        display_name: "Vertraulichkeit.pdf".to_owned(),
        original_file_identity: vocument_core::identity::FileIdentity {
            source_token: "/home/user/Vertraulichkeit.pdf".to_owned(),
            display_name: "Vertraulichkeit.pdf".to_owned(),
            os_file_id: None,
            size_bytes: 12,
            modified_unix_ms: Some(0),
            sha256_hex: String::new(),
        },
        active_pid: std::process::id(),
        is_clean_shutdown: false,
        has_unsaved_changes: true,
        uncommitted_revisions: 1,
        last_saved_revision: 0,
        created_unix_ms: 0,
        updated_unix_ms: 0,
        last_checkpoint_file: None,
        last_checkpoint_sha256: None,
        manifest_checksum_sha256: String::new(),
    };
    container
        .write_manifest(&manifest)
        .expect("the manifest must be writable");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_mode = std::fs::metadata(container.container_dir())
            .expect("the container directory must exist")
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(
            dir_mode, 0o700,
            "the recovery container holds the path of the document of the user and must not be readable for others, mode was {dir_mode:o}"
        );
    }

    let _ = std::fs::remove_dir_all(&directory);
}
