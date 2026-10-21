//! UniFFI DTOs and exported functions for M0.6 Recovery (`ROADMAP.md` M0.6, `ARCHITECTURE.md` §§11–12, `REQUIREMENTS.md` §34).
//!
//! Exposes:
//! - Recoverable document scanning and crash detection
//! - Document recovery checks
//! - Execution of recovery actions: Restore, Preview, Discard, and Open as New (`RECOVERY-001` Preset)
//! - Gate invariant: **Original file on disk remains completely intact until explicit user save.**

use vocument_core::file_broker::ScopedLocalFileBroker;
use vocument_core::recovery::{RecoverableDocumentInfo, RecoveryAction, RecoveryActionResult};

use crate::session_dto::FfiDocumentSessionSnapshot;
use crate::{DocumentCoreError, session_registry};

/// Information about a recoverable document detected after an unclean shutdown (`ROADMAP.md` M0.6).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRecoverableDocumentInfo {
    pub document_id: String,
    pub source_token: String,
    pub display_name: String,
    pub uncommitted_revisions: u64,
    pub last_updated_unix_ms: u64,
    pub has_checkpoint: bool,
    pub has_journal: bool,
    pub original_file_exists: bool,
    pub original_modified_externally: bool,
}

impl From<RecoverableDocumentInfo> for FfiRecoverableDocumentInfo {
    fn from(info: RecoverableDocumentInfo) -> Self {
        Self {
            document_id: info.document_id,
            source_token: info.source_token,
            display_name: info.display_name,
            uncommitted_revisions: info.uncommitted_revisions,
            last_updated_unix_ms: info.last_updated_unix_ms,
            has_checkpoint: info.has_checkpoint,
            has_journal: info.has_journal,
            original_file_exists: info.original_file_exists,
            original_modified_externally: info.original_modified_externally,
        }
    }
}

/// Recovery action choices crossing the UniFFI boundary (`ROADMAP.md` M0.6, `RECOVERY-001` Preset).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiRecoveryAction {
    /// Restores journal and checkpoints into active session; disk file remains untouched until explicit save.
    Restore,
    /// Opens read-only preview session of uncommitted recovered state.
    Preview,
    /// Discards recovery container; leaves disk file intact.
    Discard,
    /// Opens recovered state as an unsaved new document; leaves original disk file intact.
    OpenAsNew,
}

impl From<FfiRecoveryAction> for RecoveryAction {
    fn from(action: FfiRecoveryAction) -> Self {
        match action {
            FfiRecoveryAction::Restore => Self::Restore,
            FfiRecoveryAction::Preview => Self::Preview,
            FfiRecoveryAction::Discard => Self::Discard,
            FfiRecoveryAction::OpenAsNew => Self::OpenAsNew,
        }
    }
}

impl From<RecoveryAction> for FfiRecoveryAction {
    fn from(action: RecoveryAction) -> Self {
        match action {
            RecoveryAction::Restore => Self::Restore,
            RecoveryAction::Preview => Self::Preview,
            RecoveryAction::Discard => Self::Discard,
            RecoveryAction::OpenAsNew => Self::OpenAsNew,
        }
    }
}

/// Outcome of executing a recovery action crossing UniFFI (`ROADMAP.md` M0.6).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRecoveryActionResult {
    pub document_id: String,
    pub action: FfiRecoveryAction,
    pub session_snapshot: Option<FfiDocumentSessionSnapshot>,
    pub target_token: Option<String>,
    pub message: String,
}

impl From<RecoveryActionResult> for FfiRecoveryActionResult {
    fn from(res: RecoveryActionResult) -> Self {
        Self {
            document_id: res.document_id,
            action: res.action.into(),
            session_snapshot: res.session_snapshot.map(Into::into),
            target_token: res.target_token,
            message: res.message,
        }
    }
}

/// Whether the crash recovery of the documents is available in this run.
///
/// The recovery store holds the document of the user in plain text. It
/// therefore only writes into a directory that belongs to the user alone. If
/// that directory cannot be made private, the store refuses every write — and
/// the user has to learn about it, because the documents are then not
/// protected against a crash.
///
/// The fourth round of the red team found the error of the private root being
/// printed to the standard error and then forgotten: the run continued as if
/// the recovery were protected. The reason now travels to the application,
/// which shows it as a warning.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiRecoveryStatus {
    /// Whether the recovery store may write.
    pub available: bool,
    /// Why the recovery is not available — empty when it is.
    pub details: String,
}

/// Returns the state of the crash recovery of the documents.
///
/// The application calls it once at the start and shows `details` as a
/// warning when the recovery is not available.
#[uniffi::export]
pub fn document_core_recovery_status() -> FfiRecoveryStatus {
    crate::recovery_status()
}

/// Scans the recovery directory for documents that suffered an unclean shutdown with unsaved changes.
#[uniffi::export]
pub fn document_core_scan_recoverable_documents()
-> Result<Vec<FfiRecoverableDocumentInfo>, DocumentCoreError> {
    let broker = ScopedLocalFileBroker::new();
    let registry = session_registry();
    let infos = registry
        .recovery_store()
        .scan_recoverable_documents(&broker);
    Ok(infos.into_iter().map(Into::into).collect())
}

/// Checks if a specific document by source path or token has an active crash recovery container.
#[uniffi::export]
pub fn document_core_check_document_recovery(
    source_token: String,
) -> Result<Option<FfiRecoverableDocumentInfo>, DocumentCoreError> {
    let broker = ScopedLocalFileBroker::new();
    let registry = session_registry();
    let info = registry
        .recovery_store()
        .check_recovery_for_source_token(&source_token, &broker);
    Ok(info.map(Into::into))
}

/// Executes a recovery action (`Restore`, `Preview`, `Discard`, `OpenAsNew`) on a recovered document.
///
/// **M0.6 Gate Invariant:** Disk file remains 100% intact until explicit user save.
#[uniffi::export]
pub fn document_core_execute_recovery_action(
    document_id: String,
    action: FfiRecoveryAction,
) -> Result<FfiRecoveryActionResult, DocumentCoreError> {
    let registry = session_registry();
    let result = registry.execute_recovery_action(&document_id, action.into())?;
    Ok(result.into())
}

/// Marks a document's recovery container clean shutdown.
#[uniffi::export]
pub fn document_core_mark_recovery_clean_shutdown(
    document_id: String,
) -> Result<(), DocumentCoreError> {
    let registry = session_registry();
    registry
        .recovery_store()
        .container_for(&document_id)
        .mark_clean_shutdown()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document_core_close_session, document_core_open_macos_scoped_file,
        document_core_rotate_pages,
    };
    use std::fs;
    use vocument_core::identity::compute_sha256_hex;

    fn sample_pdf() -> Vec<u8> {
        b"%PDF-1.4\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [3 0 R] /Count 1 >>\nendobj\n3 0 obj\n<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>\nendobj\nxref\n0 4\n0000000000 65535 f \n0000000010 00000 n \n0000000060 00000 n \n0000000119 00000 n \ntrailer\n<< /Size 4 /Root 1 0 R >>\nstartxref\n193\n%%EOF\n".to_vec()
    }

    #[test]
    fn test_recovery_ffi_scan_and_actions_gate_verified() {
        let temp_dir = std::env::temp_dir().join(format!("voc_ffi_rec_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let test_pdf = temp_dir.join("ffi_rec_doc.pdf");
        let initial_bytes = sample_pdf();
        fs::write(&test_pdf, &initial_bytes).expect("write initial pdf");
        let initial_hash = compute_sha256_hex(&initial_bytes);

        // Open session via FFI:
        let session = document_core_open_macos_scoped_file(test_pdf.to_string_lossy().into_owned())
            .expect("open session");
        let doc_id = session.document_id.clone();

        // Mutate session via FFI:
        let _ = document_core_rotate_pages(session.session_id.clone(), vec![0], 90)
            .expect("rotate page");

        // Simulate crash: we do NOT call close_session or save_session.
        // Check document recovery via FFI:
        let check = document_core_check_document_recovery(test_pdf.to_string_lossy().into_owned())
            .expect("check recovery")
            .expect("recovery info found");
        assert_eq!(check.document_id, doc_id);
        assert_eq!(check.uncommitted_revisions, 2);

        // Scan recoverable documents via FFI:
        let all_recoverable = document_core_scan_recoverable_documents().expect("scan recoverable");
        assert!(all_recoverable.iter().any(|r| r.document_id == doc_id));

        // Gate Verification: Original disk file is STILL 100% untouched:
        let current_disk = fs::read(&test_pdf).expect("read disk file");
        assert_eq!(compute_sha256_hex(&current_disk), initial_hash);

        // Execute Restore action:
        let restore_res =
            document_core_execute_recovery_action(doc_id.clone(), FfiRecoveryAction::Restore)
                .expect("restore action");
        assert_eq!(restore_res.action, FfiRecoveryAction::Restore);
        assert!(restore_res.session_snapshot.is_some());

        // Gate Verification again: Still untouched on disk after Restore!
        let current_disk_after_restore = fs::read(&test_pdf).expect("read disk file");
        assert_eq!(
            compute_sha256_hex(&current_disk_after_restore),
            initial_hash
        );

        // Clean up session:
        if let Some(snap) = restore_res.session_snapshot {
            let _ = document_core_close_session(snap.session_id);
        }
        let _ = fs::remove_dir_all(&temp_dir);
    }
}
