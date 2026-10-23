//! UniFFI session, file identity, and handshake DTOs for M0.0–M0.1 (`ARCHITECTURE.md` §§5–6, 14–15).

use vocument_core::CoreBootstrapSnapshot;
use vocument_core::file_broker::{BrokeredFileRead, FileAccessOrigin};
use vocument_core::identity::FileIdentity;
use vocument_core::pdf_engine::PdfDocumentSummary;
use vocument_core::session::DocumentSessionSnapshot;

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
    pub recovery_degraded: bool,
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
            recovery_degraded: snap.recovery_degraded,
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
