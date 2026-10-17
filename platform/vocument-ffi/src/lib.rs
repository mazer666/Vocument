//! Thin UniFFI transport boundary (`DocumentCore` API) between `vocument-core` and native UI (`ARCHITECTURE.md` §§5–6, `ROADMAP.md` M0.1).
//!
//! Exposes only opaque session/document IDs and immutable DTO snapshots.
//! Never exposes raw MuPDF pointers or internal Rust references.

use std::sync::OnceLock;
use thiserror::Error;
use vocument_core::file_broker::{
    BrokeredFileRead, FileAccessOrigin, FileBrokerError, ScopedLocalFileBroker,
};
use vocument_core::identity::FileIdentity;
use vocument_core::pdf_engine::{PdfDocumentSummary, PdfEngineError};
use vocument_core::session::{DocumentSessionSnapshot, SessionError, SessionRegistry};
use vocument_core::{CoreBootstrapSnapshot, bootstrap_core_snapshot};
use vocument_pdf_mupdf::MuPdfEngineAdapter;

uniffi::setup_scaffolding!("vocument_ffi");

static GLOBAL_SESSION_REGISTRY: OnceLock<SessionRegistry> = OnceLock::new();

fn session_registry() -> &'static SessionRegistry {
    GLOBAL_SESSION_REGISTRY
        .get_or_init(|| SessionRegistry::new(Box::new(MuPdfEngineAdapter::new())))
}

/// Error type crossing the UniFFI boundary from `DocumentCore`.
#[derive(Debug, Error, uniffi::Error, PartialEq, Eq)]
pub enum DocumentCoreError {
    #[error("configuration error: {details}")]
    ConfigurationError { details: String },
    #[error("file broker error: {details}")]
    FileBrokerFailure { details: String },
    #[error("PDF requires an open password")]
    PasswordRequired,
    #[error("invalid or corrupted PDF document: {details}")]
    InvalidPdf { details: String },
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("internal core failure: {details}")]
    InternalFailure { details: String },
}

impl From<SessionError> for DocumentCoreError {
    fn from(err: SessionError) -> Self {
        match err {
            SessionError::FileBroker(fb) => Self::FileBrokerFailure {
                details: fb.to_string(),
            },
            SessionError::PdfEngine(PdfEngineError::PasswordRequired) => Self::PasswordRequired,
            SessionError::PdfEngine(PdfEngineError::InvalidPdf { details }) => {
                Self::InvalidPdf { details }
            }
            SessionError::PdfEngine(PdfEngineError::EngineFailure { details }) => {
                Self::InternalFailure { details }
            }
            SessionError::SessionNotFound { session_id } => Self::SessionNotFound { session_id },
            SessionError::RegistryPoisoned => Self::InternalFailure {
                details: "session registry lock poisoned".to_owned(),
            },
        }
    }
}

impl From<FileBrokerError> for DocumentCoreError {
    fn from(err: FileBrokerError) -> Self {
        Self::FileBrokerFailure {
            details: err.to_string(),
        }
    }
}

/// Immutable DTO snapshot returned across the UniFFI bridge in M0.0 to verify end-to-end linkage
/// and central configuration enforcement from both Swift (macOS) and Kotlin (Android).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct DocumentCoreHandshake {
    pub baseline_version: String,
    pub milestone: String,
    pub strict_offline_default: bool,
    pub telemetry_enabled: bool,
    pub target_fps: u32,
    pub default_german_variant: String,
    pub built_in_ocr_languages: Vec<String>,
}

impl From<CoreBootstrapSnapshot> for DocumentCoreHandshake {
    fn from(snapshot: CoreBootstrapSnapshot) -> Self {
        Self {
            baseline_version: snapshot.baseline_version,
            milestone: snapshot.milestone,
            strict_offline_default: snapshot.strict_offline_default,
            telemetry_enabled: snapshot.telemetry_enabled,
            target_fps: snapshot.target_fps,
            default_german_variant: snapshot.default_german_variant,
            built_in_ocr_languages: snapshot.built_in_ocr_languages,
        }
    }
}

/// Origin mechanism through which a file was opened (`ARCHITECTURE.md` §15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiFileAccessOrigin {
    MacOsScopedUrl,
    AndroidSafUri,
    InMemoryBuffer,
}

impl From<FileAccessOrigin> for FfiFileAccessOrigin {
    fn from(origin: FileAccessOrigin) -> Self {
        match origin {
            FileAccessOrigin::MacOsScopedUrl => Self::MacOsScopedUrl,
            FileAccessOrigin::AndroidSafUri => Self::AndroidSafUri,
            FileAccessOrigin::InMemoryBuffer => Self::InMemoryBuffer,
        }
    }
}

impl From<FfiFileAccessOrigin> for FileAccessOrigin {
    fn from(origin: FfiFileAccessOrigin) -> Self {
        match origin {
            FfiFileAccessOrigin::MacOsScopedUrl => Self::MacOsScopedUrl,
            FfiFileAccessOrigin::AndroidSafUri => Self::AndroidSafUri,
            FfiFileAccessOrigin::InMemoryBuffer => Self::InMemoryBuffer,
        }
    }
}

/// Multi-signal file identity DTO exposed across UniFFI (`ARCHITECTURE.md` §14).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiFileIdentity {
    pub source_token: String,
    pub display_name: String,
    pub os_file_id: Option<String>,
    pub size_bytes: u64,
    pub modified_unix_ms: Option<i64>,
    pub sha256_hex: String,
}

impl From<FileIdentity> for FfiFileIdentity {
    fn from(id: FileIdentity) -> Self {
        Self {
            source_token: id.source_token,
            display_name: id.display_name,
            os_file_id: id.os_file_id,
            size_bytes: id.size_bytes,
            modified_unix_ms: id.modified_unix_ms,
            sha256_hex: id.sha256_hex,
        }
    }
}

/// Basic PDF metadata and page count DTO exposed across UniFFI (`ROADMAP.md` M0.1).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfDocumentSummary {
    pub page_count: u32,
    pub format_version: Option<String>,
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
    pub creation_date: Option<String>,
    pub modification_date: Option<String>,
    pub needs_password: bool,
    pub engine_identifier: String,
}

impl From<PdfDocumentSummary> for FfiPdfDocumentSummary {
    fn from(s: PdfDocumentSummary) -> Self {
        Self {
            page_count: s.page_count,
            format_version: s.format_version,
            title: s.title,
            author: s.author,
            subject: s.subject,
            keywords: s.keywords,
            creator: s.creator,
            producer: s.producer,
            creation_date: s.creation_date,
            modification_date: s.modification_date,
            needs_password: s.needs_password,
            engine_identifier: s.engine_identifier,
        }
    }
}

/// Immutable snapshot of an opened `DocumentSession` returned to Swift and Kotlin (`ARCHITECTURE.md` §§5–6).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiDocumentSessionSnapshot {
    pub session_id: String,
    pub document_id: String,
    pub revision: u64,
    pub is_dirty: bool,
    pub access_origin: FfiFileAccessOrigin,
    pub file_identity: FfiFileIdentity,
    pub pdf_summary: FfiPdfDocumentSummary,
}

impl From<DocumentSessionSnapshot> for FfiDocumentSessionSnapshot {
    fn from(snap: DocumentSessionSnapshot) -> Self {
        Self {
            session_id: snap.session_id.as_str().to_owned(),
            document_id: snap.document_id.as_str().to_owned(),
            revision: snap.revision,
            is_dirty: snap.is_dirty,
            access_origin: snap.access_origin.into(),
            file_identity: snap.file_identity.into(),
            pdf_summary: snap.pdf_summary.into(),
        }
    }
}

/// Platform-brokered file descriptor passed into `DocumentCore` (e.g. from Android Storage Access Framework or macOS `FileBroker`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBrokeredFileDescriptor {
    pub source_token: String,
    pub display_name: String,
    pub access_origin: FfiFileAccessOrigin,
    pub os_file_id: Option<String>,
    pub modified_unix_ms: Option<i64>,
    pub bytes: Vec<u8>,
}

impl From<FfiBrokeredFileDescriptor> for BrokeredFileRead {
    fn from(desc: FfiBrokeredFileDescriptor) -> Self {
        Self {
            source_token: desc.source_token,
            display_name: desc.display_name,
            access_origin: desc.access_origin.into(),
            os_file_id: desc.os_file_id,
            modified_unix_ms: desc.modified_unix_ms,
            bytes: desc.bytes,
        }
    }
}

/// Performs the M0.0 `DocumentCore` bootstrap handshake for Swift and Kotlin callers.
#[uniffi::export]
pub fn document_core_handshake() -> Result<DocumentCoreHandshake, DocumentCoreError> {
    bootstrap_core_snapshot()
        .map(DocumentCoreHandshake::from)
        .map_err(|err| DocumentCoreError::ConfigurationError {
            details: err.to_string(),
        })
}

/// Opens a PDF document from a macOS Finder / Open With security-scoped file path or URL via `ScopedLocalFileBroker` (`ROADMAP.md` M0.1).
#[uniffi::export]
pub fn document_core_open_macos_scoped_file(
    scoped_path_or_url: String,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let broker = ScopedLocalFileBroker::new();
    let snapshot = session_registry().open_via_broker(&broker, &scoped_path_or_url)?;
    Ok(snapshot.into())
}

/// Opens a PDF document from a platform `FileBroker` descriptor (e.g. Android Storage Access Framework `content://` URI) (`ROADMAP.md` M0.1).
#[uniffi::export]
pub fn document_core_open_brokered_file(
    descriptor: FfiBrokeredFileDescriptor,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let snapshot = session_registry().open_brokered_file(descriptor.into())?;
    Ok(snapshot.into())
}

/// Returns the current immutable `FfiDocumentSessionSnapshot` for an active `session_id`.
#[uniffi::export]
pub fn document_core_get_session_snapshot(
    session_id: String,
) -> Result<FfiDocumentSessionSnapshot, DocumentCoreError> {
    let snapshot = session_registry().session_snapshot(&session_id)?;
    Ok(snapshot.into())
}

/// Closes an active `DocumentSession` and releases its underlying `PdfEngine` resources.
#[uniffi::export]
pub fn document_core_close_session(session_id: String) -> Result<(), DocumentCoreError> {
    session_registry().close_session(&session_id)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn sample_corpus_pdf_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/corpus/m0_1_sample_two_pages.pdf")
    }

    #[test]
    fn ffi_handshake_returns_verified_core_snapshot() {
        let handshake = document_core_handshake().expect("handshake must succeed");
        assert_eq!(handshake.baseline_version, "0.1");
        assert_eq!(handshake.milestone, "M0");
        assert!(handshake.strict_offline_default);
        assert!(!handshake.telemetry_enabled);
        assert_eq!(handshake.target_fps, 60);
        assert_eq!(handshake.default_german_variant, "de-AT");
        assert_eq!(handshake.built_in_ocr_languages, vec!["deu", "eng"]);
    }

    #[test]
    fn ffi_opens_real_pdf_via_macos_scoped_broker_and_android_saf_broker() {
        let pdf_path = sample_corpus_pdf_path();
        let mac_snap =
            document_core_open_macos_scoped_file(pdf_path.to_string_lossy().into_owned())
                .expect("macOS scoped file open must succeed");

        assert!(mac_snap.session_id.starts_with("sess_"));
        assert_eq!(mac_snap.document_id, "doc_1a00192b75fb403b");
        assert_eq!(mac_snap.revision, 1);
        assert!(!mac_snap.is_dirty);
        assert_eq!(mac_snap.access_origin, FfiFileAccessOrigin::MacOsScopedUrl);
        assert_eq!(
            mac_snap.file_identity.display_name,
            "m0_1_sample_two_pages.pdf"
        );
        assert_eq!(mac_snap.file_identity.size_bytes, 697);
        assert_eq!(
            mac_snap.file_identity.sha256_hex,
            "1a00192b75fb403bf574c22afd1898a361e7f043d4f3489770c9560607370bee"
        );
        assert_eq!(mac_snap.pdf_summary.page_count, 2);
        assert_eq!(
            mac_snap.pdf_summary.title.as_deref(),
            Some("Vocument M0.1 Sample Document")
        );
        assert_eq!(
            mac_snap.pdf_summary.author.as_deref(),
            Some("Vocument Engineering")
        );

        // Open same PDF via Android SAF descriptor and verify identical stable DocumentId + metadata
        let bytes = std::fs::read(&pdf_path).expect("read fixture bytes");
        let saf_snap = document_core_open_brokered_file(FfiBrokeredFileDescriptor {
            source_token: "content://com.android.providers.downloads.documents/document/42"
                .to_owned(),
            display_name: "m0_1_sample_two_pages.pdf".to_owned(),
            access_origin: FfiFileAccessOrigin::AndroidSafUri,
            os_file_id: Some("saf:42".to_owned()),
            modified_unix_ms: Some(1_760_000_000_000),
            bytes,
        })
        .expect("Android SAF broker open must succeed");

        assert_eq!(saf_snap.document_id, mac_snap.document_id);
        assert_ne!(saf_snap.session_id, mac_snap.session_id);
        assert_eq!(saf_snap.access_origin, FfiFileAccessOrigin::AndroidSafUri);
        assert_eq!(saf_snap.pdf_summary.page_count, 2);

        // Fetch snapshot by session ID and close both sessions
        let fetched = document_core_get_session_snapshot(mac_snap.session_id.clone())
            .expect("session lookup must succeed");
        assert_eq!(fetched, mac_snap);

        document_core_close_session(mac_snap.session_id.clone()).expect("close mac session");
        document_core_close_session(saf_snap.session_id).expect("close saf session");

        let after_close = document_core_get_session_snapshot(mac_snap.session_id);
        assert!(matches!(
            after_close,
            Err(DocumentCoreError::SessionNotFound { .. })
        ));
    }
}
