//! UniFFI DTOs and exported functions for M0.5 Safe Save (`ARCHITECTURE.md` §16, `ROADMAP.md` M0.5).
//!
//! Exposes:
//! - Safe save with atomic staging and post-save validation (`SAVEATOMIC-001`)
//! - 'Save As' and collision-free safe copy (`SAVE-002`, `FILELOCK-004`)
//! - Multi-signal external modification detection (`FILELOCK-002`)
//! - Non-silent conflict UI contract and resolution options (`FILELOCK-003`, `FILELOCK-004`)
//! - **Gate: Never silently overwrite externally changed content.**

use crate::session_dto::FfiFileIdentity;
use crate::{DocumentCoreError, session_registry};
use std::path::Path;
use vocument_core::save::{
    ExternalChangeCheckResult, SaveConflict, SaveConflictResolution, SaveResult,
    SaveSuccessSnapshot, generate_collision_free_path,
};

/// Conflict contract crossing UniFFI when external change is detected on disk (`FILELOCK-003`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSaveConflict {
    pub session_id: String,
    pub target_token: String,
    pub baseline_identity: FfiFileIdentity,
    pub observed_external_identity: FfiFileIdentity,
    pub comparison_summary: String,
    pub suggested_safe_copy_path: String,
}

impl From<SaveConflict> for FfiSaveConflict {
    fn from(c: SaveConflict) -> Self {
        Self {
            session_id: c.session_id.as_str().to_owned(),
            target_token: c.target_token,
            baseline_identity: c.baseline_identity.into(),
            observed_external_identity: c.observed_external_identity.into(),
            comparison_summary: format!("{:?}", c.comparison),
            suggested_safe_copy_path: c.suggested_safe_copy_path,
        }
    }
}

/// Conflict resolution choices crossing UniFFI (`ARCHITECTURE.md` §16, `FILELOCK-004`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSaveConflictResolution {
    /// Save local dirty state to a collision-free safe copy path, leaving external file untouched.
    SaveLocalAsCollisionFreeCopy,
    /// Explicitly overwrites external file after user confirmation.
    OverwriteExternalExplicitly,
    /// Discards local dirty edits and reloads external version from disk.
    ReloadExternalVersion,
    /// Cancels save and keeps local dirty edits in session.
    Cancel,
}

impl From<FfiSaveConflictResolution> for SaveConflictResolution {
    fn from(r: FfiSaveConflictResolution) -> Self {
        match r {
            FfiSaveConflictResolution::SaveLocalAsCollisionFreeCopy => {
                Self::SaveLocalAsCollisionFreeCopy
            }
            FfiSaveConflictResolution::OverwriteExternalExplicitly => {
                Self::OverwriteExternalExplicitly
            }
            FfiSaveConflictResolution::ReloadExternalVersion => Self::ReloadExternalVersion,
            FfiSaveConflictResolution::Cancel => Self::Cancel,
        }
    }
}

/// Snapshot describing a successfully completed safe save crossing UniFFI.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiSaveSuccessSnapshot {
    pub session_id: String,
    pub target_token: String,
    pub new_file_identity: FfiFileIdentity,
    pub saved_revision: u64,
    pub bytes_written: u64,
}

impl From<SaveSuccessSnapshot> for FfiSaveSuccessSnapshot {
    fn from(s: SaveSuccessSnapshot) -> Self {
        Self {
            session_id: s.session_id.as_str().to_owned(),
            target_token: s.target_token,
            new_file_identity: s.new_file_identity.into(),
            saved_revision: s.saved_revision,
            bytes_written: s.bytes_written,
        }
    }
}

/// Safe save outcome crossing UniFFI: either success or conflict (`ROADMAP.md` M0.5 Gate).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Enum)]
pub enum FfiSaveOutcome {
    Success { snapshot: FfiSaveSuccessSnapshot },
    Conflict { conflict: FfiSaveConflict },
}

impl From<SaveResult> for FfiSaveOutcome {
    fn from(res: SaveResult) -> Self {
        match res {
            SaveResult::Success(succ) => Self::Success {
                snapshot: succ.into(),
            },
            SaveResult::Conflict(conf) => Self::Conflict {
                conflict: conf.into(),
            },
        }
    }
}

/// External change inspection result crossing UniFFI (`FILELOCK-002`).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiExternalChangeCheckResult {
    pub session_id: String,
    pub source_token: String,
    pub has_external_changes: bool,
    pub comparison_summary: String,
    pub observed_identity: Option<FfiFileIdentity>,
}

impl From<ExternalChangeCheckResult> for FfiExternalChangeCheckResult {
    fn from(c: ExternalChangeCheckResult) -> Self {
        Self {
            session_id: c.session_id.as_str().to_owned(),
            source_token: c.source_token,
            has_external_changes: c.has_external_changes,
            comparison_summary: format!("{:?}", c.comparison),
            observed_identity: c.observed_identity.map(Into::into),
        }
    }
}

/// Safe save of active `session_id` back to its original file (`ROADMAP.md` M0.5, `SAVE-001`).
#[uniffi::export]
pub fn document_core_save_session(session_id: String) -> Result<FfiSaveOutcome, DocumentCoreError> {
    let outcome = session_registry().save_session(&session_id)?;
    Ok(outcome.into())
}

/// 'Save As' of active `session_id` to `target_token` (`SAVE-002`).
#[uniffi::export]
pub fn document_core_save_session_as(
    session_id: String,
    target_token: String,
) -> Result<FfiSaveOutcome, DocumentCoreError> {
    let outcome = session_registry().save_session_as(&session_id, &target_token)?;
    Ok(outcome.into())
}

/// Saves a guaranteed collision-free safe copy of `session_id` (`ARCHITECTURE.md` §16, `FILELOCK-004`).
#[uniffi::export]
pub fn document_core_save_session_collision_free_copy(
    session_id: String,
) -> Result<FfiSaveSuccessSnapshot, DocumentCoreError> {
    let snapshot = session_registry().save_session_collision_free_copy(&session_id)?;
    Ok(snapshot.into())
}

/// Inspects whether on-disk file for `session_id` was modified externally (`FILELOCK-002`).
#[uniffi::export]
pub fn document_core_check_external_changes(
    session_id: String,
) -> Result<FfiExternalChangeCheckResult, DocumentCoreError> {
    let res = session_registry().check_external_changes(&session_id)?;
    Ok(res.into())
}

/// Resolves an external save conflict according to `resolution` (`ARCHITECTURE.md` §16, `FILELOCK-004`).
#[uniffi::export]
pub fn document_core_resolve_save_conflict(
    session_id: String,
    resolution: FfiSaveConflictResolution,
) -> Result<Option<FfiSaveSuccessSnapshot>, DocumentCoreError> {
    let res = session_registry().resolve_save_conflict(&session_id, resolution.into())?;
    Ok(res.map(Into::into))
}

/// Generates a collision-free file path for `base_path` and `suffix` (`FILELOCK-004`).
#[uniffi::export]
pub fn document_core_generate_collision_free_path(base_path: String, suffix: String) -> String {
    let path = generate_collision_free_path(Path::new(&base_path), &suffix);
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mutation_dto::document_core_rotate_pages;
    use crate::{document_core_close_session, document_core_open_macos_scoped_file};

    #[test]
    fn ffi_m0_5_safe_save_and_conflict_handling_pipeline() {
        let sample = Path::new("../../tests/corpus/m0_1_sample_two_pages.pdf");
        assert!(sample.exists(), "sample PDF must exist");
        let sample_bytes = std::fs::read(sample).expect("read sample");

        let temp_dir = std::env::temp_dir().join(format!("ffi_save_test_{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let test_pdf = temp_dir.join("test_ffi_doc.pdf");
        std::fs::write(&test_pdf, &sample_bytes).expect("write test pdf");

        let session = document_core_open_macos_scoped_file(test_pdf.to_string_lossy().into_owned())
            .expect("open session");
        assert!(!session.is_dirty);

        // Mutate:
        let _ = document_core_rotate_pages(session.session_id.clone(), vec![0], 90)
            .expect("rotate page");

        // 1. Safe save dirty document:
        let outcome = document_core_save_session(session.session_id.clone()).expect("save session");
        match outcome {
            FfiSaveOutcome::Success { snapshot } => {
                assert_eq!(snapshot.session_id, session.session_id);
                assert_eq!(snapshot.saved_revision, 2);
            }
            FfiSaveOutcome::Conflict { .. } => panic!("unexpected conflict on clean target"),
        }

        // 2. Simulate external edit:
        std::thread::sleep(std::time::Duration::from_millis(15));
        let mut ext = sample_bytes.clone();
        ext.extend_from_slice(b"\n%external writer change\n");
        std::fs::write(&test_pdf, &ext).expect("write external edit");

        // Check external changes:
        let check = document_core_check_external_changes(session.session_id.clone())
            .expect("check external changes");
        assert!(check.has_external_changes);

        // 3. Mutate again:
        let _ = document_core_rotate_pages(session.session_id.clone(), vec![0], 90)
            .expect("rotate page 2");

        // 4. Save must detect conflict (M0.5 GATE):
        let outcome2 =
            document_core_save_session(session.session_id.clone()).expect("save session 2");
        match outcome2 {
            FfiSaveOutcome::Conflict { conflict } => {
                assert!(conflict.comparison_summary.contains("ModifiedExternally"));
                assert!(
                    conflict
                        .suggested_safe_copy_path
                        .contains("Conflicted Copy")
                );

                // Gate verified: disk file was NOT overwritten:
                let on_disk = std::fs::read(&test_pdf).expect("read disk file");
                assert_eq!(on_disk, ext);
            }
            FfiSaveOutcome::Success { .. } => {
                panic!("FATAL: M0.5 Gate breached across FFI! External file was overwritten.")
            }
        }

        // 5. Resolve via collision-free safe copy:
        let resolved = document_core_resolve_save_conflict(
            session.session_id.clone(),
            FfiSaveConflictResolution::SaveLocalAsCollisionFreeCopy,
        )
        .expect("resolve conflict");
        let copy_snap = resolved.expect("safe copy produced snapshot");
        assert!(copy_snap.target_token.contains("Safe Copy"));
        assert!(Path::new(&copy_snap.target_token).exists());

        // 6. Test collision-free path generator:
        let gen_path = document_core_generate_collision_free_path(
            test_pdf.to_string_lossy().into_owned(),
            "Safe Copy".to_owned(),
        );
        assert!(gen_path.contains("Safe Copy"));

        document_core_close_session(session.session_id).expect("close session");
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}
