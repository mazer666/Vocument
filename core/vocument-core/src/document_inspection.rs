//! Inspection of the links and actions of a document (`INSPECT-001`).
//!
//! A professional review asks two questions about a link: where does it lead,
//! and does it do something besides opening a page? A link that runs
//! JavaScript or opens a local file is not a link — it is an action of the
//! document, and it happens when the user clicks.
//!
//! The inspection separates the two layers:
//!
//! * The **engine** reports the facts: the type of the action and its target.
//!   The engine knows the PDF action types; it does not decide what is
//!   dangerous.
//! * The **core** applies the policy: it marks an action as risky. The rules
//!   are product decisions, not engine knowledge, and they have to be
//!   testable without a PDF.

use serde::{Deserialize, Serialize};

use crate::pdf_engine::PdfRectMilliPt;

/// The type of a link or action of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PdfActionKind {
    /// A link to another page of the same document.
    InternalDestination,
    /// A link to another document or to an address (`http`, `https`, `mailto`, ...).
    Uri,
    /// A link to a file outside the document.
    ExternalDestination,
    /// An action that opens a file or starts a program (`/Launch`).
    LaunchFile,
    /// An action that runs JavaScript (`/JavaScript`).
    JavaScript,
    /// An action that sends the content of a form to a server (`/SubmitForm`).
    FormSubmission,
    /// An action that imports data into the document (`/ImportData`).
    ImportData,
    /// An action that runs a named action (`/NamedAction`).
    NamedAction,
    /// An action that resets a form (`/ResetForm`).
    ResetForm,
    /// An action that hides annotations (`/Hide`).
    HideAnnotations,
    /// A media action (`/Movie`, `/Sound`, `/Rendition`).
    Media,
    /// An action that changes the state of the layers (`/SetOCGState`, `/Trans`).
    LayerState,
    /// An action of a type that is not known here.
    Other(String),
}

impl PdfActionKind {
    /// Parses the name of a PDF action type as it is reported by the engine.
    pub fn from_engine_name(name: &str) -> Self {
        match name {
            "internal_destination" => PdfActionKind::InternalDestination,
            "uri" => PdfActionKind::Uri,
            "external_destination" => PdfActionKind::ExternalDestination,
            "launch_file" => PdfActionKind::LaunchFile,
            "javascript" => PdfActionKind::JavaScript,
            "form_submission" => PdfActionKind::FormSubmission,
            "import_data" => PdfActionKind::ImportData,
            "named_action" => PdfActionKind::NamedAction,
            "reset_form" => PdfActionKind::ResetForm,
            "hide_annotations" => PdfActionKind::HideAnnotations,
            "media" => PdfActionKind::Media,
            "layer_state" => PdfActionKind::LayerState,
            other => PdfActionKind::Other(other.to_owned()),
        }
    }

    /// True if the action runs code, opens a file or sends data out of the
    /// document. Those are the actions a reviewer has to see before a click.
    pub fn is_risky_kind(&self) -> bool {
        matches!(
            self,
            PdfActionKind::LaunchFile
                | PdfActionKind::JavaScript
                | PdfActionKind::FormSubmission
                | PdfActionKind::ImportData
        )
    }
}

/// One link or action of a document (`INSPECT-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfActionInfo {
    /// The page the action lies on. `None` for an action of the document
    /// itself, for example the action that runs when the document is opened.
    pub page_index: Option<u32>,
    /// The area that can be clicked. `None` for an action of the document
    /// itself.
    pub rect: Option<PdfRectMilliPt>,
    /// The type of the action.
    pub kind: PdfActionKind,
    /// The target of the action: the address, the file, the script or the
    /// destination.
    pub target: String,
    /// True if the action leaves the document, runs code or sends data.
    pub is_risky: bool,
}

impl PdfActionInfo {
    /// Creates an action and applies the risk policy of the core to it.
    pub fn new(
        page_index: Option<u32>,
        rect: Option<PdfRectMilliPt>,
        kind: PdfActionKind,
        target: String,
    ) -> Self {
        let is_risky = kind.is_risky_kind() || is_risky_target(&kind, &target);
        Self {
            page_index,
            rect,
            kind,
            target,
            is_risky,
        }
    }
}

/// True if the target of the action is dangerous by itself.
///
/// A `file:` address points at the file system of the machine, a
/// `javascript:` address runs code in the reader, and an `http:` address
/// leaves the machine. A reviewer wants to see all three.
pub fn is_risky_target(kind: &PdfActionKind, target: &str) -> bool {
    match kind {
        PdfActionKind::Uri | PdfActionKind::ExternalDestination => {
            let trimmed = target.trim();
            let lowered = trimmed.to_ascii_lowercase();
            lowered.starts_with("file:")
                || lowered.starts_with("javascript:")
                || lowered.starts_with("data:")
                || lowered.starts_with("http:")
                || lowered.starts_with("https:")
        }
        _ => false,
    }
}

/// An embedded file of a document (`ATTACH-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfAttachmentInfo {
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

impl PdfAttachmentInfo {
    /// Creates an attachment and applies the risk policy of the core to it.
    pub fn new(
        file_name: String,
        mime_type: String,
        size_bytes: i64,
        is_embedded: bool,
        checksum_is_valid: bool,
    ) -> Self {
        let is_risky =
            !is_embedded || !checksum_is_valid || is_risky_attachment(&file_name, &mime_type);
        Self {
            file_name,
            mime_type,
            size_bytes,
            is_embedded,
            checksum_is_valid,
            is_risky,
        }
    }
}

/// The extensions of files that can execute code when they are opened.
const EXECUTABLE_EXTENSIONS: [&str; 14] = [
    "exe", "bat", "cmd", "com", "scr", "msi", "dll", "jar", "js", "vbs", "ps1", "sh", "apk", "app",
];

/// True if the name or the type of the attachment says that it can execute
/// code.
pub fn is_risky_attachment(file_name: &str, mime_type: &str) -> bool {
    let lowered_name = file_name.to_ascii_lowercase();
    if let Some(index) = lowered_name.rfind('.') {
        let extension = &lowered_name[index + 1..];
        if EXECUTABLE_EXTENSIONS.contains(&extension) {
            return true;
        }
    }
    let lowered_type = mime_type.to_ascii_lowercase();
    lowered_type.contains("executable")
        || lowered_type.contains("msdownload")
        || lowered_type.contains("msdos-program")
        || lowered_type.contains("x-sh")
        || lowered_type.contains("x-msdownload")
}

/// A signature field of a document (`SIGN-001`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdfSignatureInfo {
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

impl PdfSignatureInfo {
    /// Creates a signature and applies the risk policy of the core to it.
    pub fn new(
        field_name: String,
        is_signed: bool,
        signer: String,
        signed_at: String,
        reason: String,
        location: String,
        changed_since_signing: bool,
    ) -> Self {
        Self {
            field_name,
            is_signed,
            signer,
            signed_at,
            reason,
            location,
            changed_since_signing,
            is_risky: changed_since_signing,
        }
    }
}

/// The report of an inspection of a document (`INSPECT-001`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentInspection {
    /// All links and actions of the document.
    pub actions: Vec<PdfActionInfo>,
    /// The embedded files of the document.
    pub attachments: Vec<PdfAttachmentInfo>,
    /// The signature fields of the document.
    pub signatures: Vec<PdfSignatureInfo>,
    /// The number of the findings that the policy marks as risky.
    pub risky_count: usize,
    /// The number of the actions that leave the document (an address or a
    /// file).
    pub external_count: usize,
}

impl DocumentInspection {
    /// Builds the report from the findings and counts them.
    pub fn from_findings(
        actions: Vec<PdfActionInfo>,
        attachments: Vec<PdfAttachmentInfo>,
        signatures: Vec<PdfSignatureInfo>,
    ) -> Self {
        let risky_count = actions.iter().filter(|action| action.is_risky).count()
            + attachments
                .iter()
                .filter(|attachment| attachment.is_risky)
                .count()
            + signatures
                .iter()
                .filter(|signature| signature.is_risky)
                .count();
        let external_count = actions
            .iter()
            .filter(|action| {
                matches!(
                    action.kind,
                    PdfActionKind::Uri | PdfActionKind::ExternalDestination
                )
            })
            .count();
        Self {
            actions,
            attachments,
            signatures,
            risky_count,
            external_count,
        }
    }

    /// True if the document carries at least one signature.
    pub fn is_signed(&self) -> bool {
        self.signatures.iter().any(|signature| signature.is_signed)
    }

    /// True if the document was changed after it was signed. A signature that
    /// no longer covers the content of the document is worth nothing.
    pub fn is_unchanged_since_signing(&self) -> bool {
        self.signatures
            .iter()
            .filter(|signature| signature.is_signed)
            .all(|signature| !signature.changed_since_signing)
    }

    /// True if the document does something beyond showing pages.
    pub fn has_risky_actions(&self) -> bool {
        self.risky_count > 0
    }

    /// The actions that the policy marks as risky.
    pub fn risky_actions(&self) -> Vec<&PdfActionInfo> {
        self.actions
            .iter()
            .filter(|action| action.is_risky)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_script_is_risky_and_a_link_into_the_document_is_not() {
        let script = PdfActionInfo::new(
            Some(0),
            None,
            PdfActionKind::JavaScript,
            "app.alert('click')".to_owned(),
        );
        assert!(script.is_risky, "a script runs code");

        let internal = PdfActionInfo::new(
            Some(0),
            None,
            PdfActionKind::InternalDestination,
            "chapter1".to_owned(),
        );
        assert!(!internal.is_risky, "a link into the document is harmless");
    }

    #[test]
    fn a_link_to_a_local_file_or_to_the_network_is_risky() {
        for target in [
            "file:///etc/passwd",
            "https://example.com",
            "HTTP://example.com",
        ] {
            let action = PdfActionInfo::new(Some(0), None, PdfActionKind::Uri, target.to_owned());
            assert!(action.is_risky, "'{target}' leaves the machine");
        }

        let mail = PdfActionInfo::new(
            Some(0),
            None,
            PdfActionKind::Uri,
            "mailto:review@example.com".to_owned(),
        );
        assert!(!mail.is_risky, "a mail address leaves no file behind");
    }

    #[test]
    fn a_file_that_can_execute_code_is_risky_and_a_text_file_is_not() {
        let program = PdfAttachmentInfo::new(
            "anhang.exe".to_owned(),
            "application/octet-stream".to_owned(),
            30,
            true,
            true,
        );
        assert!(program.is_risky, "a program that can be started is risky");

        let script = PdfAttachmentInfo::new(
            "notiz.js".to_owned(),
            "text/plain".to_owned(),
            30,
            true,
            true,
        );
        assert!(script.is_risky, "a script is risky");

        let text = PdfAttachmentInfo::new(
            "protokoll.txt".to_owned(),
            "text/plain".to_owned(),
            30,
            true,
            true,
        );
        assert!(!text.is_risky, "a text file is harmless");

        let outside = PdfAttachmentInfo::new(
            "protokoll.txt".to_owned(),
            "text/plain".to_owned(),
            30,
            false,
            true,
        );
        assert!(
            outside.is_risky,
            "a file that is not in the document points outside"
        );

        let changed = PdfAttachmentInfo::new(
            "protokoll.txt".to_owned(),
            "text/plain".to_owned(),
            30,
            true,
            false,
        );
        assert!(
            changed.is_risky,
            "a checksum that does not match means a change"
        );
    }

    #[test]
    fn a_signature_that_no_longer_covers_the_document_is_risky() {
        let intact = PdfSignatureInfo::new(
            "Unterschrift".to_owned(),
            true,
            "Dr. Weber".to_owned(),
            "2026-01-31T12:00:00Z".to_owned(),
            "Freigabe".to_owned(),
            "Berlin".to_owned(),
            false,
        );
        assert!(!intact.is_risky, "an intact signature is not risky");

        let changed = PdfSignatureInfo::new(
            "Unterschrift".to_owned(),
            true,
            "Dr. Weber".to_owned(),
            "2026-01-31T12:00:00Z".to_owned(),
            "Freigabe".to_owned(),
            "Berlin".to_owned(),
            true,
        );
        assert!(changed.is_risky, "a signature that covers nothing is risky");
    }

    #[test]
    fn the_report_counts_what_it_found() {
        let actions = vec![
            PdfActionInfo::new(
                Some(0),
                None,
                PdfActionKind::InternalDestination,
                "a".to_owned(),
            ),
            PdfActionInfo::new(Some(0), None, PdfActionKind::JavaScript, "b".to_owned()),
            PdfActionInfo::new(
                Some(0),
                None,
                PdfActionKind::Uri,
                "https://example.com".to_owned(),
            ),
        ];
        let inspection = DocumentInspection::from_findings(actions, Vec::new(), Vec::new());
        assert_eq!(inspection.risky_count, 2);
        assert_eq!(inspection.external_count, 1);
        assert!(inspection.has_risky_actions());
        assert_eq!(inspection.risky_actions().len(), 2);
    }
}
