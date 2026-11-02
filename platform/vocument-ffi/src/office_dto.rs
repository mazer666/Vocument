//! UniFFI types and exported bridge functions for M4.5 Office Interoperability (`OFFICE-001`–`005`, `ADR-0020`).
//!
//! Provides native `.docx` and `.odt` import into `FlowEditorSession`, export to `.docx` and `.odt`
//! with atomic file writes, transparent `FfiConversionReport` surfacing, and crash-safe `.pdoc`
//! checkpointing (`PdocRecoveryStore`) that preserves text, styles, comments, tracked changes,
//! footnotes, and binary image assets.

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::atomic::Ordering;

use vocument_core::config::PackageLimitsConfig;
use vocument_layout::editor::FlowEditorState;
use vocument_office::{
    AssetMap, ConversionReport, ImportedAsset, OfficeFormat, OfficeLimits, Severity, detect_format,
    export_document, import_document,
};
use vocument_pdoc::{PdocPackage, PdocRecoveryStore};

use crate::DocumentCoreError;
use crate::editor_dto::{
    EDITOR_COUNTER, FfiEditorSessionSnapshot, FlowEditorSession, make_snapshot, registry,
    with_session,
};

/// Severity classification of an Office conversion finding (`OFFICE-004`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiConversionSeverity {
    Info,
    PreservedUnrendered,
    Approximated,
    Loss,
    Security,
}

impl From<Severity> for FfiConversionSeverity {
    fn from(s: Severity) -> Self {
        match s {
            Severity::Info => Self::Info,
            Severity::PreservedUnrendered => Self::PreservedUnrendered,
            Severity::Approximated => Self::Approximated,
            Severity::Loss => Self::Loss,
            Severity::Security => Self::Security,
        }
    }
}

/// Individual conversion report entry surfaced to the native UI (`OFFICE-004`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiConversionEntry {
    pub severity: FfiConversionSeverity,
    pub code: String,
    pub message: String,
    pub count: u32,
    pub sample_location: Option<String>,
}

/// Aggregated conversion report returned by DOCX/ODT import and export (`OFFICE-004`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiConversionReport {
    pub entries: Vec<FfiConversionEntry>,
    pub has_losses: bool,
    pub has_security_findings: bool,
    pub loss_count: u32,
    pub approximation_count: u32,
    pub passthrough_count: u32,
    pub summary_text: String,
}

impl From<&ConversionReport> for FfiConversionReport {
    fn from(report: &ConversionReport) -> Self {
        let mut loss_count = 0u32;
        let mut approximation_count = 0u32;
        let mut passthrough_count = 0u32;
        let mut entries = Vec::with_capacity(report.entries.len());

        for e in &report.entries {
            let count = e.occurrences;
            match e.severity {
                Severity::Loss => loss_count = loss_count.saturating_add(count),
                Severity::Approximated => {
                    approximation_count = approximation_count.saturating_add(count)
                }
                Severity::PreservedUnrendered => {
                    passthrough_count = passthrough_count.saturating_add(count)
                }
                Severity::Info | Severity::Security => {}
            }
            entries.push(FfiConversionEntry {
                severity: e.severity.into(),
                code: e.code.clone(),
                message: e.message.clone(),
                count,
                sample_location: e.first_location.clone(),
            });
        }

        let summary_text = if entries.is_empty() {
            "Verlustfreie Konvertierung ohne Einschränkungen.".to_string()
        } else {
            format!(
                "Konvertierungsbericht: {} Verluste, {} Näherungen, {} durchgereichte Elemente.",
                loss_count, approximation_count, passthrough_count
            )
        };

        Self {
            entries,
            has_losses: report.has_losses(),
            has_security_findings: report.entries_with(Severity::Security).next().is_some(),
            loss_count,
            approximation_count,
            passthrough_count,
            summary_text,
        }
    }
}

/// Result of importing a `.docx` or `.odt` document into the native flow editor (`OFFICE-001`, `OFFICE-002`).
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct FfiOfficeImportResult {
    pub snapshot: FfiEditorSessionSnapshot,
    pub report: FfiConversionReport,
    pub detected_format: String,
}

pub(crate) fn extract_assets_from_pdoc(pkg: &PdocPackage) -> AssetMap {
    let mut map = AssetMap::new();
    for (path, data) in pkg.assets.iter() {
        let media_type = pkg
            .manifest
            .assets
            .get(path)
            .and_then(|e| e.mime_type.clone())
            .unwrap_or_else(|| "application/octet-stream".to_string());
        map.insert(
            path.clone(),
            ImportedAsset {
                media_type,
                bytes: data.clone(),
            },
        );
    }
    map
}

pub(crate) fn build_pdoc_package_from_session(session: &FlowEditorSession) -> PdocPackage {
    let mut pkg = PdocPackage::from_flow_document(session.state.document.clone());
    for (path, asset) in &session.assets {
        let _ = pkg.add_asset(path, asset.bytes.clone(), Some(&asset.media_type));
    }
    pkg
}

fn pdoc_recovery_store() -> Result<PdocRecoveryStore, DocumentCoreError> {
    let dir = crate::data_directory().join("pdoc_recovery");
    PdocRecoveryStore::new(dir).map_err(|e| DocumentCoreError::RecoveryFailure {
        details: format!("Failed to initialize PdocRecoveryStore: {e}"),
    })
}

pub(crate) fn save_editor_recovery_checkpoint(
    session: &FlowEditorSession,
) -> Result<String, DocumentCoreError> {
    let store = pdoc_recovery_store()?;
    let pkg = build_pdoc_package_from_session(session);
    let path = store
        .save_checkpoint(&pkg)
        .map_err(|e| DocumentCoreError::RecoveryFailure {
            details: format!("Failed to save editor recovery checkpoint: {e}"),
        })?;
    Ok(path.to_string_lossy().to_string())
}

fn detect_office_format_for_path(
    path: &Path,
    bytes: &[u8],
    limits: &OfficeLimits,
) -> Option<OfficeFormat> {
    if let Some(fmt) = detect_format(bytes, limits) {
        return Some(fmt);
    }
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .as_deref()
    {
        Some("docx") => Some(OfficeFormat::Docx),
        Some("odt" | "ott") => Some(OfficeFormat::Odt),
        _ => None,
    }
}

fn atomic_write_bytes(target_path: &Path, bytes: &[u8]) -> Result<(), DocumentCoreError> {
    let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
    let file_name = target_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("export.office");
    let tmp_path = parent.join(format!(".tmp_{}_{}", std::process::id(), file_name));

    let mut file = fs::File::create(&tmp_path).map_err(|e| DocumentCoreError::SaveFailure {
        details: format!("Failed to create temporary export file: {e}"),
    })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| {
            let _ = fs::remove_file(&tmp_path);
            DocumentCoreError::SaveFailure {
                details: format!("Failed to write and fsync export file: {e}"),
            }
        })?;
    drop(file);

    fs::rename(&tmp_path, target_path).map_err(|e| {
        let _ = fs::remove_file(&tmp_path);
        DocumentCoreError::SaveFailure {
            details: format!("Failed to commit exported file into place: {e}"),
        }
    })
}

/// Imports a `.docx` or `.odt` file into a native flow editor session (`OFFICE-001`, `OFFICE-002`, `OFFICE-004`, `OFFICE-005`).
#[uniffi::export]
pub fn document_core_open_office_editor_session(
    file_path: String,
) -> Result<FfiOfficeImportResult, DocumentCoreError> {
    let path = Path::new(&file_path);
    let bytes = fs::read(path).map_err(|e| DocumentCoreError::FileBrokerFailure {
        details: format!("Failed to read Office document '{file_path}': {e}"),
    })?;
    let limits = OfficeLimits::default();
    let format = detect_office_format_for_path(path, &bytes, &limits).ok_or_else(|| {
        DocumentCoreError::InvalidPdf {
            details: format!("Unsupported or unrecognized Office package format: {file_path}"),
        }
    })?;

    let imported =
        import_document(&bytes, format, &limits).map_err(|e| DocumentCoreError::InvalidPdf {
            details: format!("Office import failed: {e}"),
        })?;

    let mut doc = imported.document;
    if (doc.metadata.title.trim().is_empty() || doc.metadata.title == "Untitled")
        && let Some(stem) = path.file_stem().and_then(|s| s.to_str())
    {
        doc.metadata.title = stem.to_string();
    }
    let title = doc.metadata.title.clone();
    let state = FlowEditorState::from_document(doc);
    let id = format!(
        "editor_sess_{}",
        EDITOR_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let session = FlowEditorSession {
        id: id.clone(),
        title,
        file_path: None,
        fingerprint: None,
        assets: imported.assets,
        state,
    };

    let _ = save_editor_recovery_checkpoint(&session);
    let snapshot = make_snapshot(&session);
    let ffi_report = FfiConversionReport::from(&imported.report);
    let detected_format = match format {
        OfficeFormat::Docx => "docx".to_string(),
        OfficeFormat::Odt => "odt".to_string(),
    };

    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    reg.insert(id, session);

    Ok(FfiOfficeImportResult {
        snapshot,
        report: ffi_report,
        detected_format,
    })
}

/// Exports the active flow editor session to an OOXML `.docx` file (`OFFICE-001`, `OFFICE-004`).
#[uniffi::export]
pub fn document_core_editor_export_docx(
    editor_id: String,
    target_path: String,
) -> Result<FfiConversionReport, DocumentCoreError> {
    with_session(&editor_id, |s| {
        let exported =
            export_document(&s.state.document, &s.assets, OfficeFormat::Docx).map_err(|e| {
                DocumentCoreError::SaveFailure {
                    details: format!("DOCX export failed: {e}"),
                }
            })?;
        atomic_write_bytes(Path::new(&target_path), &exported.bytes)?;
        Ok(FfiConversionReport::from(&exported.report))
    })
}

/// Exports the active flow editor session to an ODF Text `.odt` file (`OFFICE-002`, `OFFICE-004`).
#[uniffi::export]
pub fn document_core_editor_export_odt(
    editor_id: String,
    target_path: String,
) -> Result<FfiConversionReport, DocumentCoreError> {
    with_session(&editor_id, |s| {
        let exported =
            export_document(&s.state.document, &s.assets, OfficeFormat::Odt).map_err(|e| {
                DocumentCoreError::SaveFailure {
                    details: format!("ODT export failed: {e}"),
                }
            })?;
        atomic_write_bytes(Path::new(&target_path), &exported.bytes)?;
        Ok(FfiConversionReport::from(&exported.report))
    })
}

/// Explicitly saves an atomic `.pdoc` crash-recovery checkpoint for an active editor session.
#[uniffi::export]
pub fn document_core_editor_save_recovery_checkpoint(
    editor_id: String,
) -> Result<String, DocumentCoreError> {
    with_session(&editor_id, |s| save_editor_recovery_checkpoint(s))
}

/// Restores a flow editor session from a `.pdoc` crash-recovery checkpoint by document ID.
#[uniffi::export]
pub fn document_core_editor_recover_session(
    document_id: String,
) -> Result<Option<FfiEditorSessionSnapshot>, DocumentCoreError> {
    let store = pdoc_recovery_store()?;
    let limits = PackageLimitsConfig::default();
    let Some(pkg) = store.load_checkpoint(&document_id, &limits).map_err(|e| {
        DocumentCoreError::RecoveryFailure {
            details: format!("Failed to load editor recovery checkpoint: {e}"),
        }
    })?
    else {
        return Ok(None);
    };

    let title = pkg.document.metadata.title.clone();
    let assets = extract_assets_from_pdoc(&pkg);
    let state = FlowEditorState::from_document(pkg.into_flow_document());
    let id = format!(
        "editor_sess_{}",
        EDITOR_COUNTER.fetch_add(1, Ordering::Relaxed)
    );
    let session = FlowEditorSession {
        id: id.clone(),
        title,
        file_path: None,
        fingerprint: None,
        assets,
        state,
    };
    let snap = make_snapshot(&session);
    let mut reg = registry()
        .lock()
        .map_err(|e| DocumentCoreError::InternalFailure {
            details: e.to_string(),
        })?;
    reg.insert(id, session);
    Ok(Some(snap))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::editor_dto::*;

    #[test]
    fn ffi_office_docx_odt_roundtrip_preserves_comments_tracking_undo_and_recovery() {
        let tmp_dir =
            std::env::temp_dir().join(format!("vocument_ffi_office_{}", std::process::id()));
        let _ = fs::remove_dir_all(&tmp_dir);
        fs::create_dir_all(&tmp_dir).expect("create tmp_dir");

        let snap = document_core_create_editor_session("Vertrag M4.5".to_string())
            .expect("create session");
        let eid = snap.editor_id.clone();

        let snap = document_core_editor_insert_text(
            eid.clone(),
            "§ 1 Haftung und Gewährleistung.".to_string(),
        )
        .expect("insert text");
        assert_eq!(snap.active_block_text, "§ 1 Haftung und Gewährleistung.");

        let snap = document_core_editor_add_comment(
            eid.clone(),
            Some(0),
            Some(11),
            "Bitte Haftungsobergrenze prüfen.".to_string(),
        )
        .expect("add comment");
        assert_eq!(snap.comments.len(), 1);

        let snap = document_core_editor_enable_tracking(eid.clone(), "RA Dr. Müller".to_string())
            .expect("enable tracking");
        assert!(snap.is_tracking);

        let snap =
            document_core_editor_insert_text(eid.clone(), " Höchstbetrag: 1 Mio. EUR.".to_string())
                .expect("insert tracked text");
        assert_eq!(snap.tracked_changes.len(), 1);

        let docx_path = tmp_dir.join("vertrag.docx");
        let docx_report =
            document_core_editor_export_docx(eid.clone(), docx_path.to_string_lossy().to_string())
                .expect("export docx");
        assert!(!docx_report.has_losses);
        assert!(docx_path.exists());

        let odt_path = tmp_dir.join("vertrag.odt");
        let odt_report =
            document_core_editor_export_odt(eid.clone(), odt_path.to_string_lossy().to_string())
                .expect("export odt");
        assert!(!odt_report.has_losses);
        assert!(odt_path.exists());

        // Import DOCX into a new editor session and verify comments, tracked changes, undo/redo, and recovery
        let imported_docx =
            document_core_open_office_editor_session(docx_path.to_string_lossy().to_string())
                .expect("import docx");
        assert_eq!(imported_docx.detected_format, "docx");
        assert!(imported_docx.snapshot.plain_text.contains("Haftung"));
        assert_eq!(imported_docx.snapshot.comments.len(), 1);
        assert_eq!(imported_docx.snapshot.tracked_changes.len(), 1);

        // Edit imported DOCX session and test undo/redo
        let docx_eid = imported_docx.snapshot.editor_id.clone();
        let edited = document_core_editor_insert_text(docx_eid.clone(), " Zusatz.".to_string())
            .expect("edit imported docx");
        assert!(edited.can_undo);
        let undone = document_core_editor_undo(docx_eid.clone()).expect("undo");
        assert!(undone.can_redo);
        let redone = document_core_editor_redo(docx_eid.clone()).expect("redo");
        assert!(redone.plain_text.contains("Zusatz."));

        // Save recovery checkpoint and verify recovery preserves everything
        let cp_path = document_core_editor_save_recovery_checkpoint(docx_eid.clone())
            .expect("save recovery checkpoint");
        assert!(Path::new(&cp_path).exists());

        // Import ODT into a new editor session and verify comments and tracked changes
        let imported_odt =
            document_core_open_office_editor_session(odt_path.to_string_lossy().to_string())
                .expect("import odt");
        assert_eq!(imported_odt.detected_format, "odt");
        assert!(imported_odt.snapshot.plain_text.contains("Haftung"));
        assert_eq!(imported_odt.snapshot.comments.len(), 1);
        assert_eq!(imported_odt.snapshot.tracked_changes.len(), 1);

        let _ = document_core_close_editor_session(eid);
        let _ = document_core_close_editor_session(docx_eid);
        let _ = document_core_close_editor_session(imported_odt.snapshot.editor_id);
        let _ = fs::remove_dir_all(&tmp_dir);
    }
}
