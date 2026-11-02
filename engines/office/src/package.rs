//! Safe reading and deterministic writing of ZIP-based Office packages (OPC and ODF).
//!
//! Reading enforces the limits of [`OfficeLimits`] against the *actual* decompressed bytes,
//! never against the sizes the ZIP headers claim, because a hostile file can lie in its headers.

use std::collections::BTreeMap;
use std::io::{Cursor, Read, Write};

use zip::CompressionMethod;
use zip::write::SimpleFileOptions;
use zip::{ZipArchive, ZipWriter};

use crate::error::OfficeError;
use crate::limits::OfficeLimits;

/// Magic number of an OLE compound document (legacy `.doc`, encrypted OOXML).
const OLE_COMPOUND_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// Longest accepted entry name.
const MAX_ENTRY_NAME_LENGTH: usize = 512;

/// An Office package: every entry fully read and validated.
#[derive(Debug, Clone, Default)]
pub struct OfficePackage {
    parts: BTreeMap<String, Vec<u8>>,
}

impl OfficePackage {
    /// Reads and validates a package from bytes.
    pub fn from_bytes(bytes: &[u8], limits: &OfficeLimits) -> Result<Self, OfficeError> {
        if bytes.len() as u64 > limits.max_package_bytes {
            return Err(OfficeError::ResourceLimitExceeded {
                what: "package size in bytes".to_string(),
                limit: limits.max_package_bytes,
            });
        }
        if bytes.starts_with(&OLE_COMPOUND_MAGIC) {
            return Err(OfficeError::PasswordProtectedOrLegacy);
        }
        let mut archive =
            ZipArchive::new(Cursor::new(bytes)).map_err(|error| OfficeError::NotAZipPackage {
                details: error.to_string(),
            })?;
        if archive.len() > limits.max_entries {
            return Err(OfficeError::ResourceLimitExceeded {
                what: "number of package entries".to_string(),
                limit: limits.max_entries as u64,
            });
        }

        let mut parts = BTreeMap::new();
        let mut folded_names = std::collections::BTreeSet::new();
        let mut total_bytes: u64 = 0;

        for index in 0..archive.len() {
            let mut entry = match archive.by_index(index) {
                Ok(entry) => entry,
                Err(zip::result::ZipError::UnsupportedArchive(reason))
                    if reason.to_lowercase().contains("password") =>
                {
                    return Err(OfficeError::EncryptedEntry {
                        name: format!("#{index}"),
                    });
                }
                Err(error) => return Err(error.into()),
            };
            let name = entry.name().to_string();
            if entry.is_dir() {
                validate_entry_name(name.trim_end_matches('/'))?;
                continue;
            }
            validate_entry_name(&name)?;
            if entry.encrypted() {
                return Err(OfficeError::EncryptedEntry { name });
            }
            if !folded_names.insert(name.to_lowercase()) {
                return Err(OfficeError::UnsafeEntryName {
                    name,
                    reason: "duplicate entry (package part names are case-insensitive)".to_string(),
                });
            }

            let remaining_total = limits.max_total_bytes.saturating_sub(total_bytes);
            let allowed = limits.max_entry_bytes.min(remaining_total);
            let mut data = Vec::new();
            let read = (&mut entry)
                .take(allowed + 1)
                .read_to_end(&mut data)
                .map_err(|error| OfficeError::Zip(format!("cannot read '{name}': {error}")))?
                as u64;
            if read > allowed {
                let limit = if allowed == remaining_total {
                    limits.max_total_bytes
                } else {
                    limits.max_entry_bytes
                };
                return Err(OfficeError::ResourceLimitExceeded {
                    what: format!("uncompressed size of '{name}' or of the whole package"),
                    limit,
                });
            }
            let compressed = entry.compressed_size().max(1);
            if read > limits.ratio_check_threshold_bytes
                && read / compressed > limits.max_compression_ratio
            {
                return Err(OfficeError::ResourceLimitExceeded {
                    what: format!("compression ratio of '{name}' (decompression bomb)"),
                    limit: limits.max_compression_ratio,
                });
            }
            total_bytes += read;
            parts.insert(name, data);
        }
        Ok(Self { parts })
    }

    /// Content of a part, if present.
    pub fn part(&self, name: &str) -> Option<&[u8]> {
        self.parts.get(name).map(Vec::as_slice)
    }

    /// Content of a part that must exist.
    pub fn required_part(&self, name: &str) -> Result<&[u8], OfficeError> {
        self.part(name).ok_or_else(|| OfficeError::MissingPart {
            name: name.to_string(),
        })
    }

    /// Names of all parts in sorted order.
    pub fn part_names(&self) -> impl Iterator<Item = &str> {
        self.parts.keys().map(String::as_str)
    }

    /// True when a part exists.
    pub fn contains(&self, name: &str) -> bool {
        self.parts.contains_key(name)
    }
}

/// Rejects entry names that could escape a directory or confuse path handling.
fn validate_entry_name(name: &str) -> Result<(), OfficeError> {
    let reject = |reason: &str| {
        Err(OfficeError::UnsafeEntryName {
            name: name.chars().take(80).collect(),
            reason: reason.to_string(),
        })
    };
    if name.is_empty() {
        return reject("empty name");
    }
    if name.len() > MAX_ENTRY_NAME_LENGTH {
        return reject("name too long");
    }
    if name.starts_with('/') {
        return reject("absolute path");
    }
    if name.contains('\\') {
        return reject("backslash in name");
    }
    if name.chars().any(|c| c == '\0' || c.is_control()) {
        return reject("control character in name");
    }
    if name.split('/').any(|component| component == "..") {
        return reject("parent directory component");
    }
    if name
        .split('/')
        .next()
        .is_some_and(|first| first.contains(':'))
    {
        return reject("drive or scheme prefix");
    }
    Ok(())
}

/// Builds a package with deterministic metadata: same input, same bytes.
#[derive(Debug)]
pub struct PackageWriter {
    writer: ZipWriter<Cursor<Vec<u8>>>,
}

impl Default for PackageWriter {
    fn default() -> Self {
        Self::new()
    }
}

impl PackageWriter {
    pub fn new() -> Self {
        Self {
            writer: ZipWriter::new(Cursor::new(Vec::new())),
        }
    }

    /// Adds a compressed entry.
    pub fn add(&mut self, name: &str, data: &[u8]) -> Result<(), OfficeError> {
        self.add_with(name, data, CompressionMethod::Deflated)
    }

    /// Adds an uncompressed entry (ODF requires `mimetype` stored and first).
    pub fn add_stored(&mut self, name: &str, data: &[u8]) -> Result<(), OfficeError> {
        self.add_with(name, data, CompressionMethod::Stored)
    }

    fn add_with(
        &mut self,
        name: &str,
        data: &[u8],
        method: CompressionMethod,
    ) -> Result<(), OfficeError> {
        validate_entry_name(name)?;
        // `SimpleFileOptions::default()` carries the fixed 1980-01-01 timestamp without the
        // `time` feature, which keeps the output reproducible.
        let options = SimpleFileOptions::default().compression_method(method);
        self.writer.start_file(name, options)?;
        self.writer.write_all(data)?;
        Ok(())
    }

    /// Finishes the archive and returns its bytes.
    pub fn finish(self) -> Result<Vec<u8>, OfficeError> {
        let cursor = self.writer.finish()?;
        Ok(cursor.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut writer = PackageWriter::new();
        for (name, data) in entries {
            writer.add(name, data).expect("add");
        }
        writer.finish().expect("finish")
    }

    #[test]
    fn roundtrips_and_is_deterministic() {
        let bytes = package_with(&[("a.xml", b"<a/>"), ("dir/b.bin", &[1, 2, 3])]);
        assert_eq!(
            bytes,
            package_with(&[("a.xml", b"<a/>"), ("dir/b.bin", &[1, 2, 3])])
        );
        let package = OfficePackage::from_bytes(&bytes, &OfficeLimits::default()).expect("read");
        assert_eq!(package.part("a.xml"), Some(&b"<a/>"[..]));
        assert_eq!(package.part_names().count(), 2);
    }

    #[test]
    fn refuses_non_zip_and_ole_input() {
        assert!(matches!(
            OfficePackage::from_bytes(b"plain text", &OfficeLimits::default()),
            Err(OfficeError::NotAZipPackage { .. })
        ));
        let mut ole = OLE_COMPOUND_MAGIC.to_vec();
        ole.extend_from_slice(&[0; 512]);
        assert!(matches!(
            OfficePackage::from_bytes(&ole, &OfficeLimits::default()),
            Err(OfficeError::PasswordProtectedOrLegacy)
        ));
    }

    #[test]
    fn refuses_path_traversal_entry_names() {
        for name in ["../evil", "a/../../evil", "/abs", "C:evil", "a\\b"] {
            // The writer refuses these too, so craft the archive bytes through the zip crate
            // directly to prove the reader's own check.
            let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file(name, SimpleFileOptions::default())
                .expect("start");
            zip.write_all(b"x").expect("write");
            let bytes = zip.finish().expect("finish").into_inner();
            assert!(
                matches!(
                    OfficePackage::from_bytes(&bytes, &OfficeLimits::default()),
                    Err(OfficeError::UnsafeEntryName { .. })
                ),
                "{name} must be refused"
            );
            assert!(PackageWriter::new().add(name, b"x").is_err());
        }
    }

    #[test]
    fn refuses_decompression_bombs() {
        // 40 MiB of zeros deflates to ~40 KiB: ratio ~1000 and above the entry limit below.
        let bomb = vec![0u8; 40 * 1024 * 1024];
        let bytes = package_with(&[("bomb.xml", &bomb)]);
        let limits = OfficeLimits {
            max_entry_bytes: 8 * 1024 * 1024,
            ..OfficeLimits::default()
        };
        assert!(matches!(
            OfficePackage::from_bytes(&bytes, &limits),
            Err(OfficeError::ResourceLimitExceeded { .. })
        ));

        let ratio_limits = OfficeLimits {
            max_compression_ratio: 50,
            ..OfficeLimits::default()
        };
        assert!(matches!(
            OfficePackage::from_bytes(&bytes, &ratio_limits),
            Err(OfficeError::ResourceLimitExceeded { .. })
        ));

        let total_limits = OfficeLimits {
            max_total_bytes: 1024,
            ..OfficeLimits::default()
        };
        assert!(
            OfficePackage::from_bytes(&package_with(&[("x", &[0u8; 4096])]), &total_limits)
                .is_err()
        );
    }

    #[test]
    fn refuses_too_many_entries_and_duplicate_names() {
        let limits = OfficeLimits {
            max_entries: 2,
            ..OfficeLimits::default()
        };
        let bytes = package_with(&[("a", b"1"), ("b", b"2"), ("c", b"3")]);
        assert!(OfficePackage::from_bytes(&bytes, &limits).is_err());

        let duplicate = package_with(&[("Word/Doc.xml", b"1"), ("word/doc.xml", b"2")]);
        assert!(OfficePackage::from_bytes(&duplicate, &OfficeLimits::default()).is_err());
    }
}
