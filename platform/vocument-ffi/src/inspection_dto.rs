//! UniFFI DTOs and exported functions for the inspection of the links and
//! actions of a document (`INSPECT-001`).
//!
//! A professional review asks two questions about a link: where does it lead,
//! and does it do something besides opening a page? A link that runs a script
//! or starts a file is not a link, it is an action of the document.
//!
//! The inspection therefore reports two things per action:
//!
//! - **what** the action is (its type and its target),
//! - **whether** it is risky, according to the policy of the core.
//!
//! The risk is decided in the core, not in the engine and not in the platform:
//! the platform shell only presents the finding to the user.

use crate::{DocumentCoreError, FfiPdfRectMilliPt, session_registry};
use vocument_core::document_inspection::{
    DocumentInspection, PdfActionInfo, PdfActionKind, PdfAttachmentInfo, PdfSignatureInfo,
};

/// The type of a link or action of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiPdfActionKind {
    /// A link to another page of the same document.
    InternalDestination,
    /// A link to an address (`http`, `https`, `mailto`, ...).
    Uri,
    /// A link to another document.
    ExternalDestination,
    /// An action that opens a file or starts a program.
    LaunchFile,
    /// An action that runs JavaScript.
    JavaScript,
    /// An action that sends the content of a form to a server.
    FormSubmission,
    /// An action that imports data into the document.
    ImportData,
    /// An action that runs a named action.
    NamedAction,
    /// An action that resets a form.
    ResetForm,
    /// An action that hides annotations.
    HideAnnotations,
    /// A media action (`/Movie`, `/Sound`, `/Rendition`).
    Media,
    /// An action that changes the state of the layers.
    LayerState,
    /// An action of an unknown type.
    Other(String),
}

impl From<PdfActionKind> for FfiPdfActionKind {
    fn from(kind: PdfActionKind) -> Self {
        match kind {
            PdfActionKind::InternalDestination => FfiPdfActionKind::InternalDestination,
            PdfActionKind::Uri => FfiPdfActionKind::Uri,
            PdfActionKind::ExternalDestination => FfiPdfActionKind::ExternalDestination,
            PdfActionKind::LaunchFile => FfiPdfActionKind::LaunchFile,
            PdfActionKind::JavaScript => FfiPdfActionKind::JavaScript,
            PdfActionKind::FormSubmission => FfiPdfActionKind::FormSubmission,
            PdfActionKind::ImportData => FfiPdfActionKind::ImportData,
            PdfActionKind::NamedAction => FfiPdfActionKind::NamedAction,
            PdfActionKind::ResetForm => FfiPdfActionKind::ResetForm,
            PdfActionKind::HideAnnotations => FfiPdfActionKind::HideAnnotations,
            PdfActionKind::Media => FfiPdfActionKind::Media,
            PdfActionKind::LayerState => FfiPdfActionKind::LayerState,
            PdfActionKind::Other(name) => FfiPdfActionKind::Other(name),
        }
    }
}

/// One link or action of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfActionInfo {
    /// The page the action lies on. `None` for an action of the document
    /// itself, for example the action that runs when the document is opened.
    pub page_index: Option<u32>,
    /// The area that can be clicked. `None` for an action of the document
    /// itself.
    pub rect: Option<FfiPdfRectMilliPt>,
    /// The type of the action.
    pub kind: FfiPdfActionKind,
    /// The target of the action: the address, the file, the script or the
    /// destination.
    pub target: String,
    /// True if the action leaves the document, runs code or sends data.
    pub is_risky: bool,
}

impl From<PdfActionInfo> for FfiPdfActionInfo {
    fn from(action: PdfActionInfo) -> Self {
        Self {
            page_index: action.page_index,
            rect: action.rect.map(|rect| FfiPdfRectMilliPt {
                x0_milli_pt: rect.x0_milli_pt,
                y0_milli_pt: rect.y0_milli_pt,
                x1_milli_pt: rect.x1_milli_pt,
                y1_milli_pt: rect.y1_milli_pt,
            }),
            kind: action.kind.into(),
            target: action.target,
            is_risky: action.is_risky,
        }
    }
}

/// An embedded file of a document (`ATTACH-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfAttachmentInfo {
    /// The name of the file.
    pub file_name: String,
    /// The declared type of the file.
    pub mime_type: String,
    /// The size of the file in bytes.
    pub size_bytes: i64,
    /// True if the content of the file is really in the document. A file
    /// specification that only names a path points outside the document.
    pub is_embedded: bool,
    /// True if the checksum of the embedded file matches its content.
    pub checksum_is_valid: bool,
    /// True if the file is dangerous to open.
    pub is_risky: bool,
}

impl From<PdfAttachmentInfo> for FfiPdfAttachmentInfo {
    fn from(attachment: PdfAttachmentInfo) -> Self {
        Self {
            file_name: attachment.file_name,
            mime_type: attachment.mime_type,
            size_bytes: attachment.size_bytes,
            is_embedded: attachment.is_embedded,
            checksum_is_valid: attachment.checksum_is_valid,
            is_risky: attachment.is_risky,
        }
    }
}

/// A signature field of a document (`SIGN-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiPdfSignatureInfo {
    /// The name of the signature field.
    pub field_name: String,
    /// True if the field carries a signature.
    pub is_signed: bool,
    /// The name of the signer.
    pub signer: String,
    /// The time of the signing, in UTC (`2026-01-31T12:00:00Z`).
    pub signed_at: String,
    /// The reason of the signing.
    pub reason: String,
    /// The place of the signing.
    pub location: String,
    /// True if the document was changed after it was signed. The signature
    /// then no longer covers the content of the document.
    pub changed_since_signing: bool,
    /// True if the signature is worth nothing: the document was changed after
    /// it was signed.
    pub is_risky: bool,
}

impl From<PdfSignatureInfo> for FfiPdfSignatureInfo {
    fn from(signature: PdfSignatureInfo) -> Self {
        Self {
            field_name: signature.field_name,
            is_signed: signature.is_signed,
            signer: signature.signer,
            signed_at: signature.signed_at,
            reason: signature.reason,
            location: signature.location,
            changed_since_signing: signature.changed_since_signing,
            is_risky: signature.is_risky,
        }
    }
}

/// The report of an inspection of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiDocumentInspection {
    /// All links and actions of the document.
    pub actions: Vec<FfiPdfActionInfo>,
    /// The embedded files of the document.
    pub attachments: Vec<FfiPdfAttachmentInfo>,
    /// The signature fields of the document.
    pub signatures: Vec<FfiPdfSignatureInfo>,
    /// The number of the findings that the policy marks as risky.
    pub risky_count: u32,
    /// The number of the actions that leave the document.
    pub external_count: u32,
    /// True if the document carries at least one signature.
    pub is_signed: bool,
    /// True if the document was not changed after it was signed.
    pub is_unchanged_since_signing: bool,
}

impl From<DocumentInspection> for FfiDocumentInspection {
    fn from(inspection: DocumentInspection) -> Self {
        let is_signed = inspection.is_signed();
        let is_unchanged_since_signing = inspection.is_unchanged_since_signing();
        Self {
            actions: inspection.actions.into_iter().map(Into::into).collect(),
            attachments: inspection.attachments.into_iter().map(Into::into).collect(),
            signatures: inspection.signatures.into_iter().map(Into::into).collect(),
            risky_count: inspection.risky_count as u32,
            external_count: inspection.external_count as u32,
            is_signed,
            is_unchanged_since_signing,
        }
    }
}

/// Inspects the links and actions of the document of `session_id`
/// (`INSPECT-001`).
///
/// The report is the basis of the "before you click" view of the professional
/// review: the user sees where a link leads, and the shell can mark the
/// actions that run code, open a file or send data out of the document.
#[uniffi::export]
pub fn document_core_inspect_document(
    session_id: String,
) -> Result<FfiDocumentInspection, DocumentCoreError> {
    let inspection = session_registry().inspect_document(&session_id)?;
    Ok(inspection.into())
}
