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

/// The report of an inspection of a document (`INSPECT-001`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentInspection {
    /// All links and actions of the document.
    pub actions: Vec<PdfActionInfo>,
    /// The number of the actions that the policy marks as risky.
    pub risky_count: usize,
    /// The number of the actions that leave the document (an address or a
    /// file).
    pub external_count: usize,
}

impl DocumentInspection {
    /// Builds the report from the actions and counts the findings.
    pub fn from_actions(actions: Vec<PdfActionInfo>) -> Self {
        let risky_count = actions.iter().filter(|action| action.is_risky).count();
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
            risky_count,
            external_count,
        }
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
        let inspection = DocumentInspection::from_actions(actions);
        assert_eq!(inspection.risky_count, 2);
        assert_eq!(inspection.external_count, 1);
        assert!(inspection.has_risky_actions());
        assert_eq!(inspection.risky_actions().len(), 2);
    }
}
