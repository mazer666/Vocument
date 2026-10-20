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
use vocument_core::document_inspection::{DocumentInspection, PdfActionInfo, PdfActionKind};

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

/// The report of an inspection of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiDocumentInspection {
    /// All links and actions of the document.
    pub actions: Vec<FfiPdfActionInfo>,
    /// The number of the actions that the policy marks as risky.
    pub risky_count: u32,
    /// The number of the actions that leave the document.
    pub external_count: u32,
}

impl From<DocumentInspection> for FfiDocumentInspection {
    fn from(inspection: DocumentInspection) -> Self {
        Self {
            actions: inspection.actions.into_iter().map(Into::into).collect(),
            risky_count: inspection.risky_count as u32,
            external_count: inspection.external_count as u32,
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
