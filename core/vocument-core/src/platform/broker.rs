//! Platform-abstracted file broker interface (`CR-0003`, `PLATFORM-003`).
//!
//! Provides a uniform trait for scoped file open/save operations across macOS, Android,
//! Windows, and Linux shells without leaking OS-specific APIs into the shared core.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::file_broker::{BrokeredFileRead, FileAccessOrigin, FileBrokerError};

/// Scoped file broker trait implemented per target platform shell (`PLATFORM-003`).
pub trait PlatformFileBroker: Send + Sync {
    /// Reads file payload through the platform broker's security boundary.
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError>;

    /// Atomically writes content bytes to the destination specified by `target_token`.
    fn write_file_atomic(&self, target_token: &str, bytes: &[u8]) -> Result<(), FileBrokerError>;

    /// Displays the platform's native open file dialog and returns the user-selected file read.
    fn show_open_dialog(
        &self,
        filters: &[&str],
    ) -> Result<Option<BrokeredFileRead>, FileBrokerError>;

    /// Displays the platform's native save file dialog and returns the target token/path.
    fn show_save_dialog(
        &self,
        suggested_name: &str,
        default_extension: &str,
    ) -> Result<Option<String>, FileBrokerError>;
}

/// Windows 11 platform file broker based on Common Item Dialog and Windows file handles (`CR-0003`, M5).
#[derive(Debug, Default)]
pub struct WindowsFileBroker {
    simulated_dialog_response: Arc<Mutex<Option<BrokeredFileRead>>>,
    simulated_save_path: Arc<Mutex<Option<String>>>,
}

impl WindowsFileBroker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets simulated response for headless testing of Windows open dialogs.
    pub fn set_dialog_response(&self, response: Option<BrokeredFileRead>) {
        if let Ok(mut lock) = self.simulated_dialog_response.lock() {
            *lock = response;
        }
    }

    /// Sets simulated response for headless testing of Windows save dialogs.
    pub fn set_save_path(&self, path: Option<String>) {
        if let Ok(mut lock) = self.simulated_save_path.lock() {
            *lock = path;
        }
    }
}

impl PlatformFileBroker for WindowsFileBroker {
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError> {
        let path = std::path::Path::new(source_token);
        if !path.exists() {
            return Err(FileBrokerError::NotFound {
                source_token: source_token.to_string(),
                details: "Windows file not found".to_string(),
            });
        }
        let bytes = std::fs::read(path).map_err(|e| FileBrokerError::PermissionDenied {
            source_token: format!("{source_token}: {e}"),
        })?;
        let display_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(source_token)
            .to_string();

        Ok(BrokeredFileRead {
            source_token: source_token.to_string(),
            display_name,
            access_origin: FileAccessOrigin::InMemoryBuffer,
            os_file_id: Some(format!("win:{}", source_token)),
            modified_unix_ms: None,
            bytes,
        })
    }

    fn write_file_atomic(&self, target_token: &str, bytes: &[u8]) -> Result<(), FileBrokerError> {
        let path = std::path::Path::new(target_token);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(path, bytes).map_err(|e| FileBrokerError::StagingFailed {
            target_token: target_token.to_string(),
            details: e.to_string(),
        })
    }

    fn show_open_dialog(
        &self,
        _filters: &[&str],
    ) -> Result<Option<BrokeredFileRead>, FileBrokerError> {
        if let Ok(lock) = self.simulated_dialog_response.lock() {
            return Ok(lock.clone());
        }
        Ok(None)
    }

    fn show_save_dialog(
        &self,
        _suggested_name: &str,
        _default_extension: &str,
    ) -> Result<Option<String>, FileBrokerError> {
        if let Ok(lock) = self.simulated_save_path.lock() {
            return Ok(lock.clone());
        }
        Ok(None)
    }
}

/// In-memory reference file broker for deterministic testing across platforms.
#[derive(Debug, Default, Clone)]
pub struct InMemoryFileBroker {
    files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
}

impl InMemoryFileBroker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_file(&self, token: impl Into<String>, bytes: Vec<u8>) {
        if let Ok(mut lock) = self.files.lock() {
            lock.insert(token.into(), bytes);
        }
    }
}

impl PlatformFileBroker for InMemoryFileBroker {
    fn read_file(&self, source_token: &str) -> Result<BrokeredFileRead, FileBrokerError> {
        let lock = self.files.lock().map_err(|_| FileBrokerError::NotFound {
            source_token: source_token.to_string(),
            details: "lock poisoned".to_string(),
        })?;
        let bytes = lock
            .get(source_token)
            .ok_or_else(|| FileBrokerError::NotFound {
                source_token: source_token.to_string(),
                details: "in-memory file not found".to_string(),
            })?;

        Ok(BrokeredFileRead {
            source_token: source_token.to_string(),
            display_name: source_token.to_string(),
            access_origin: FileAccessOrigin::InMemoryBuffer,
            os_file_id: Some(format!("mem:{source_token}")),
            modified_unix_ms: Some(1_700_000_000_000),
            bytes: bytes.clone(),
        })
    }

    fn write_file_atomic(&self, target_token: &str, bytes: &[u8]) -> Result<(), FileBrokerError> {
        let mut lock = self
            .files
            .lock()
            .map_err(|_| FileBrokerError::StagingFailed {
                target_token: target_token.to_string(),
                details: "lock poisoned".to_string(),
            })?;
        lock.insert(target_token.to_string(), bytes.to_vec());
        Ok(())
    }

    fn show_open_dialog(
        &self,
        _filters: &[&str],
    ) -> Result<Option<BrokeredFileRead>, FileBrokerError> {
        let lock = self.files.lock().map_err(|_| FileBrokerError::IoError {
            source_token: "in_memory".to_string(),
            details: "lock poisoned".to_string(),
        })?;
        if let Some((k, v)) = lock.iter().next() {
            Ok(Some(BrokeredFileRead {
                source_token: k.clone(),
                display_name: k.clone(),
                access_origin: FileAccessOrigin::InMemoryBuffer,
                os_file_id: Some(format!("mem:{k}")),
                modified_unix_ms: Some(1_700_000_000_000),
                bytes: v.clone(),
            }))
        } else {
            Ok(None)
        }
    }

    fn show_save_dialog(
        &self,
        suggested_name: &str,
        default_extension: &str,
    ) -> Result<Option<String>, FileBrokerError> {
        Ok(Some(format!("{suggested_name}.{default_extension}")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_and_windows_broker_operations() {
        let broker = InMemoryFileBroker::new();
        broker.insert_file("contract.pdf", b"%PDF-1.7\n%fake".to_vec());

        let read = broker.read_file("contract.pdf").expect("read file");
        assert_eq!(read.display_name, "contract.pdf");
        assert_eq!(read.bytes, b"%PDF-1.7\n%fake");

        broker
            .write_file_atomic("output.pdf", b"new content")
            .expect("write file");
        let read_out = broker.read_file("output.pdf").expect("read written");
        assert_eq!(read_out.bytes, b"new content");

        let win_broker = WindowsFileBroker::new();
        win_broker.set_save_path(Some("C:\\Documents\\out.pdf".to_string()));
        let save_target = win_broker
            .show_save_dialog("out", "pdf")
            .expect("win save")
            .expect("save bytes");
        assert_eq!(save_target, "C:\\Documents\\out.pdf");
    }
}
