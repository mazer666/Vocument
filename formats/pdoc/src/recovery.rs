//! Crash recovery store and autosave checkpoint manager for `.pdoc` documents (`ADR-0020`, `RECOVERY-001`).

use std::path::{Path, PathBuf};
use vocument_core::config::PackageLimitsConfig;

use crate::container::{PdocContainer, PdocPackage};
use crate::error::PdocError;

/// Manages crash-safe autosave checkpoints and crash recovery for `.pdoc` packages.
#[derive(Debug, Clone)]
pub struct PdocRecoveryStore {
    recovery_dir: PathBuf,
}

impl PdocRecoveryStore {
    /// Creates a recovery store anchored at the specified directory.
    pub fn new<P: AsRef<Path>>(recovery_dir: P) -> Result<Self, PdocError> {
        let path = recovery_dir.as_ref().to_path_buf();
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
        Ok(Self { recovery_dir: path })
    }

    /// Returns the canonical path of a recovery checkpoint for the given document identifier.
    pub fn checkpoint_path(&self, document_id: &str) -> PathBuf {
        let safe_id = document_id.replace(['/', '\\', ':', '\0'], "_");
        self.recovery_dir
            .join(format!("{}.checkpoint.pdoc", safe_id))
    }

    /// Saves an autosave recovery checkpoint atomically with `fsync`.
    pub fn save_checkpoint(&self, package: &PdocPackage) -> Result<PathBuf, PdocError> {
        let path = self.checkpoint_path(&package.manifest.document_id);
        PdocContainer::save_with_conflict_check(package, &path, None)?;
        Ok(path)
    }

    /// Checks if a recovery checkpoint exists for the given document identifier.
    pub fn has_checkpoint(&self, document_id: &str) -> bool {
        self.checkpoint_path(document_id).exists()
    }

    /// Loads a recovery checkpoint for the given document if one exists.
    pub fn load_checkpoint(
        &self,
        document_id: &str,
        limits: &PackageLimitsConfig,
    ) -> Result<Option<PdocPackage>, PdocError> {
        let path = self.checkpoint_path(document_id);
        if !path.exists() {
            return Ok(None);
        }
        let package = PdocContainer::open(&path, limits)?;
        Ok(Some(package))
    }

    /// Removes the recovery checkpoint after a clean, successful save.
    pub fn remove_checkpoint(&self, document_id: &str) -> Result<bool, PdocError> {
        let path = self.checkpoint_path(document_id);
        if path.exists() {
            std::fs::remove_file(path)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Cleans up abandoned `.tmp_*` files left behind by ungraceful crashes in a directory.
    pub fn cleanup_stale_temp_files<P: AsRef<Path>>(directory: P) -> Result<usize, PdocError> {
        let mut count = 0;
        let read_dir = match std::fs::read_dir(directory) {
            Ok(entries) => entries,
            Err(_) => return Ok(0),
        };

        for entry in read_dir.flatten() {
            let path = entry.path();
            let is_stale_temp = path
                .file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|name| name.starts_with(".tmp_") && name.ends_with(".pdoc"));
            if is_stale_temp && std::fs::remove_file(&path).is_ok() {
                count += 1;
            }
        }
        Ok(count)
    }
}
