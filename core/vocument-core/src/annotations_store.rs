//! The rail of the profession on disk (`ANNOT-001`).
//!
//! The rail of the user is a setting, and a setting has to survive the
//! restart of the application: a reviewer who has chosen the colours of the
//! house expects them to be there the next morning.
//!
//! The file belongs to the user alone: the rail is read by every document the
//! user opens, and a file that anybody on the machine may write would let a
//! stranger change the meaning of the notes of the user.
//!
//! The file is written atomically — a temporary file next to it, then a
//! rename — so that a crash in the middle of the write cannot leave a rail
//! that is half written. A rail that cannot be read is not a reason to lose
//! the notes: the standard rail is used instead, and the file is left alone
//! for the user to look at.
//!
//! The first round of the red team on this file found three ways to attack
//! the write, and all three are closed here:
//!
//! 1. The staging file had a fixed name. A stranger who prepared that name
//!    as a link would have had their own file written to. The staging file
//!    now carries the identity of the process and a counter, and it is
//!    created exclusively: a file that is already there is never written
//!    through.
//! 2. Two applications that save the rail at the same time wrote into the
//!    same staging file. One of them renamed a half-written rail over the
//!    rail of the user. Every writer now has a staging file of its own, and
//!    the rename puts a complete rail over the old one.
//! 3. A path with `..` or a link in place of the file could carry the write
//!    out of the directory of the user. The name of the file is checked, the
//!    directory is made private, and a file that belongs to somebody else is
//!    refused instead of being overwritten.

use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::annotations::{JuristPalette, JuristPaletteEntry, PaletteError};

/// The name of the file that holds the rail of the profession.
pub const JURIST_PALETTE_FILE_NAME: &str = "jurist_palette.json";

/// The version of the format of the file.
///
/// A file of a newer version is not read: the team of the platform does not
/// know what it says, and a rail that is read wrong is a rail whose notes
/// mean the wrong thing.
const JURIST_PALETTE_FORMAT_VERSION: u32 = 1;

/// The rail of the profession as it lies on the disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JuristPaletteFile {
    /// The version of the format.
    pub format_version: u32,
    /// The entries of the rail.
    pub entries: Vec<JuristPaletteEntry>,
    /// The time of the last change, in milliseconds since the epoch.
    pub modified_unix_ms: u64,
}

/// The error of the rail on disk.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum PaletteFileError {
    #[error("the rail of the profession could not be read: {details}")]
    Io { details: String },
    #[error("the rail of the profession is not valid: {details}")]
    Invalid { details: String },
}

/// The file of the rail of the profession.
#[derive(Debug, Clone)]
pub struct JuristPaletteStore {
    path: PathBuf,
}

impl JuristPaletteStore {
    /// The store that keeps the rail in the given file.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// The file the rail is kept in.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Reads the rail of the user.
    ///
    /// A file that does not exist is not an error: the standard rail is used
    /// until the user changes it. A file that exists but cannot be read is
    /// an error: the caller has to tell the user that **their** rail could
    /// not be read — silently using the standard rail would make the notes of
    /// the user mean something else than the user thinks.
    ///
    /// A file that is a link is refused: the rail of the user is a file, and
    /// a link points somewhere the user did not choose.
    pub fn load(&self) -> Result<Option<JuristPalette>, PaletteFileError> {
        self.check_path()?;
        if !self.path.exists() {
            return Ok(None);
        }
        let metadata = fs::symlink_metadata(&self.path).map_err(|error| PaletteFileError::Io {
            details: format!("{}: {error}", self.path.display()),
        })?;
        if !metadata.is_file() {
            return Err(PaletteFileError::Invalid {
                details: format!(
                    "{}: the rail of the profession is not a plain file of the user",
                    self.path.display()
                ),
            });
        }
        let text = fs::read_to_string(&self.path).map_err(|error| PaletteFileError::Io {
            details: format!("{}: {error}", self.path.display()),
        })?;
        let file: JuristPaletteFile =
            serde_json::from_str(&text).map_err(|error| PaletteFileError::Invalid {
                details: format!("{}: {error}", self.path.display()),
            })?;
        if file.format_version != JURIST_PALETTE_FORMAT_VERSION {
            return Err(PaletteFileError::Invalid {
                details: format!(
                    "{}: the format version {} is not known (expected {})",
                    self.path.display(),
                    file.format_version,
                    JURIST_PALETTE_FORMAT_VERSION
                ),
            });
        }
        let palette = JuristPalette::new(file.entries).map_err(|error: PaletteError| {
            PaletteFileError::Invalid {
                details: format!("{}: {error}", self.path.display()),
            }
        })?;
        Ok(Some(palette))
    }

    /// Writes the rail of the user.
    ///
    /// The write is atomic: the file is written next to the target and then
    /// renamed over it. A crash in the middle cannot leave a half-written
    /// rail behind, and two applications that write at the same time cannot
    /// tear each other's rail apart.
    pub fn save(&self, palette: &JuristPalette) -> Result<(), PaletteFileError> {
        self.check_path()?;
        palette
            .validate()
            .map_err(|error| PaletteFileError::Invalid {
                details: error.to_string(),
            })?;
        let file = JuristPaletteFile {
            format_version: JURIST_PALETTE_FORMAT_VERSION,
            entries: palette.entries().to_vec(),
            modified_unix_ms: current_unix_ms(),
        };
        let text =
            serde_json::to_string_pretty(&file).map_err(|error| PaletteFileError::Invalid {
                details: format!("{}: {error}", self.path.display()),
            })?;
        let parent = self.parent_directory()?;
        fs::create_dir_all(&parent).map_err(|error| PaletteFileError::Io {
            details: format!("{}: {error}", parent.display()),
        })?;
        make_private_directory(&parent)?;
        self.assert_belongs_to_the_user(&parent)?;

        // Every writer has a staging file of its own, and the file is
        // created exclusively: a file that is already there — a link of a
        // stranger, for instance — is never written through.
        let temporary = parent.join(format!(
            ".{JURIST_PALETTE_FILE_NAME}.{}.{}.tmp",
            std::process::id(),
            unique_counter()
        ));
        if let Err(error) = write_new_private_file(&temporary, text.as_bytes()) {
            let _ = fs::remove_file(&temporary);
            return Err(error);
        }
        if let Err(error) = fs::rename(&temporary, &self.path) {
            let _ = fs::remove_file(&temporary);
            return Err(PaletteFileError::Io {
                details: format!(
                    "{} -> {}: {error}",
                    temporary.display(),
                    self.path.display()
                ),
            });
        }
        Ok(())
    }

    /// The directory that holds the rail.
    fn parent_directory(&self) -> Result<PathBuf, PaletteFileError> {
        match self.path.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => Ok(parent.to_path_buf()),
            _ => Err(PaletteFileError::Invalid {
                details: format!(
                    "{}: the rail of the profession has to be kept in a directory",
                    self.path.display()
                ),
            }),
        }
    }

    /// Refuses a path that could carry the write out of the directory of the
    /// user.
    fn check_path(&self) -> Result<(), PaletteFileError> {
        let name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default();
        if name != JURIST_PALETTE_FILE_NAME {
            return Err(PaletteFileError::Invalid {
                details: format!(
                    "{}: the rail of the profession is kept in a file named {JURIST_PALETTE_FILE_NAME}",
                    self.path.display()
                ),
            });
        }
        if self
            .path
            .components()
            .any(|component| matches!(component, std::path::Component::ParentDir))
        {
            return Err(PaletteFileError::Invalid {
                details: format!(
                    "{}: the path of the rail of the profession may not leave its directory",
                    self.path.display()
                ),
            });
        }
        Ok(())
    }

    /// Refuses the write when the file or the directory belongs to somebody
    /// else.
    ///
    /// A stranger who owns the file may replace it, and a stranger who owns
    /// the directory may put a file of their own in place of the rail. The
    /// rail of the user is only written where the user owns it.
    fn assert_belongs_to_the_user(&self, parent: &Path) -> Result<(), PaletteFileError> {
        let user = effective_user(parent)?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            if let Ok(metadata) = fs::metadata(parent)
                && metadata.uid() != user
            {
                return Err(PaletteFileError::Invalid {
                    details: format!(
                        "{}: the directory belongs to the user {} and not to the user of the application ({user}).",
                        parent.display(),
                        metadata.uid()
                    ),
                });
            }
            if let Ok(metadata) = fs::metadata(&self.path)
                && metadata.uid() != user
            {
                return Err(PaletteFileError::Invalid {
                    details: format!(
                        "{}: the rail of the profession belongs to the user {} and not to the user of the application ({user}).",
                        self.path.display(),
                        metadata.uid()
                    ),
                });
            }
        }
        Ok(())
    }
}

/// The time since the epoch, in milliseconds.
fn current_unix_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_millis() as u64)
        .unwrap_or(0)
}

/// A counter that makes the name of the staging file unique within the
/// process.
fn unique_counter() -> u128 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    nanos ^ u128::from(count) << 32
}

/// The user of the process, read out of a file the process writes itself.
///
/// The kernel gives such a file to the effective user of the process. The
/// name carries the identity of the process and a counter, and the file is
/// created exclusively: a file that a stranger prepared is never written
/// through.
fn effective_user(directory: &Path) -> Result<u32, PaletteFileError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let probe = directory.join(format!(
            ".owner_probe.{}.{}",
            std::process::id(),
            unique_counter()
        ));
        let outcome = write_new_private_file(&probe, b"vocument").and_then(|()| {
            fs::metadata(&probe)
                .map(|metadata| metadata.uid())
                .map_err(|error| PaletteFileError::Io {
                    details: format!("{}: {error}", probe.display()),
                })
        });
        let _ = fs::remove_file(&probe);
        outcome
    }
    #[cfg(not(unix))]
    {
        let _ = directory;
        Ok(0)
    }
}

/// Writes a file that is new, private, and never written through.
///
/// The file is created exclusively: if something is already there — a link
/// of a stranger, for instance — the write fails instead of overwriting
/// something that belongs to somebody else.
fn write_new_private_file(path: &Path, bytes: &[u8]) -> Result<(), PaletteFileError> {
    use std::io::Write;
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path).map_err(|error| PaletteFileError::Io {
        details: format!("{}: {error}", path.display()),
    })?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| PaletteFileError::Io {
            details: format!("{}: {error}", path.display()),
        })
}

/// Makes a directory reachable for its owner only.
fn make_private_directory(directory: &Path) -> Result<(), PaletteFileError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(directory)
            .map_err(|error| PaletteFileError::Io {
                details: format!("{}: {error}", directory.display()),
            })?
            .permissions();
        if permissions.mode() & 0o077 != 0 {
            permissions.set_mode(0o700);
            fs::set_permissions(directory, permissions).map_err(|error| PaletteFileError::Io {
                details: format!("{}: {error}", directory.display()),
            })?;
        }
    }
    let _ = directory;
    Ok(())
}
