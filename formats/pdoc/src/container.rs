//! Package container serialization, ZIP64 layout, and roundtrip processing (`ADR-0020`, `SOURCE-001`, `SOURCE-002`).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::io::{Cursor, Read, Seek, Write};
use std::path::Path;
use vocument_core::config::PackageLimitsConfig;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{FlowDocument, FootnoteDefinition, Section};
use vocument_flow::styles::Stylesheet;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::assets::PdocAssetStore;
use crate::error::PdocError;
use crate::manifest::{CURRENT_FORMAT_VERSION, PdocManifest};
use crate::review::PdocReviewData;
use crate::security::{validate_entry_limits, validate_entry_path};

const MANIFEST_ENTRY: &str = "manifest.json";
const DOCUMENT_ENTRY: &str = "document.json";
const STYLES_ENTRY: &str = "styles.json";
const METADATA_ENTRY: &str = "metadata.json";
const REVIEW_ENTRY: &str = "review.json";

/// Document content tree serialized into `document.json`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PdocDocumentContent {
    pub schema_version: String,
    pub sections: Vec<Section>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub footnotes: BTreeMap<String, FootnoteDefinition>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}

/// In-memory representation of an open or authored `.pdoc` container package.
#[derive(Debug, Clone, PartialEq)]
pub struct PdocPackage {
    pub manifest: PdocManifest,
    pub document: FlowDocument,
    pub assets: PdocAssetStore,
    pub review: Option<PdocReviewData>,
    pub unmapped_entries: BTreeMap<String, Vec<u8>>,
}

impl PdocPackage {
    /// Constructs a package directly from a `FlowDocument`.
    pub fn from_flow_document(document: FlowDocument) -> Self {
        let manifest = PdocManifest::new(
            document.metadata.id.clone(),
            format!("Vocument {}", env!("CARGO_PKG_VERSION")),
        );
        Self {
            manifest,
            document,
            assets: PdocAssetStore::new(),
            review: None,
            unmapped_entries: BTreeMap::new(),
        }
    }

    /// Converts package into underlying `FlowDocument`.
    pub fn into_flow_document(self) -> FlowDocument {
        self.document
    }

    /// Adds an asset into the package and registers it in the manifest.
    pub fn add_asset(
        &mut self,
        relative_path: &str,
        data: Vec<u8>,
        mime_type: Option<&str>,
    ) -> Result<String, PdocError> {
        let entry = self
            .assets
            .insert_with_path(relative_path, data, mime_type)?;
        let path = entry.path.clone();
        self.manifest.register_asset(entry);
        Ok(path)
    }

    /// Adds an asset using content-addressable storage (`assets/<sha256>.<extension>`).
    pub fn add_content_addressed_asset(
        &mut self,
        extension: &str,
        data: Vec<u8>,
        mime_type: Option<&str>,
    ) -> Result<String, PdocError> {
        let entry = self
            .assets
            .insert_content_addressed(extension, data, mime_type)?;
        let path = entry.path.clone();
        self.manifest.register_asset(entry);
        Ok(path)
    }

    /// Retrieves asset byte content.
    pub fn get_asset(&self, path: &str) -> Option<&[u8]> {
        self.assets.get(path)
    }
}

/// Container serializer and deserializer for `.pdoc` ZIP64 archives (`ADR-0020`).
#[derive(Debug, Clone, Copy, Default)]
pub struct PdocContainer;

impl PdocContainer {
    /// Reads and validates a `.pdoc` container from a seekable reader under central policy limits.
    pub fn read_from_reader<R: Read + Seek>(
        reader: R,
        limits: &PackageLimitsConfig,
    ) -> Result<PdocPackage, PdocError> {
        let mut archive = ZipArchive::new(reader)?;
        let entry_count = archive.len();

        let mut raw_entries: BTreeMap<String, Vec<u8>> = BTreeMap::new();
        let mut accumulated_bytes: u64 = 0;

        for i in 0..entry_count {
            let mut file = archive.by_index(i)?;
            let raw_name = file.name().to_string();

            // Directory entries are skipped after validation
            let is_dir = file.is_dir() || raw_name.ends_with('/') || raw_name.ends_with('\\');
            let safe_path = validate_entry_path(&raw_name)?;
            let normalized_name = safe_path.to_string_lossy().to_string();

            if is_dir {
                continue;
            }

            let compressed_size = file.compressed_size();
            let uncompressed_size = file.size();

            validate_entry_limits(
                i,
                &normalized_name,
                compressed_size,
                uncompressed_size,
                &mut accumulated_bytes,
                limits,
            )?;

            let mut buffer = Vec::with_capacity(uncompressed_size.min(1024 * 1024) as usize);
            file.read_to_end(&mut buffer)?;

            raw_entries.insert(normalized_name, buffer);
        }

        // 1. Parse manifest.json
        let manifest_bytes =
            raw_entries
                .remove(MANIFEST_ENTRY)
                .ok_or_else(|| PdocError::MissingRequiredEntry {
                    entry_name: MANIFEST_ENTRY.to_string(),
                })?;
        let manifest: PdocManifest = serde_json::from_slice(&manifest_bytes)?;

        if manifest.format_version != CURRENT_FORMAT_VERSION {
            return Err(PdocError::UnsupportedFormatVersion {
                version: manifest.format_version.clone(),
                minimum_supported: CURRENT_FORMAT_VERSION.to_string(),
            });
        }

        // 2. Parse metadata.json
        let metadata: DocumentMetadata =
            if let Some(meta_bytes) = raw_entries.remove(METADATA_ENTRY) {
                serde_json::from_slice(&meta_bytes)?
            } else {
                DocumentMetadata::new("doc_untitled", "Untitled")
            };

        // 3. Parse styles.json
        let stylesheet: Stylesheet = if let Some(style_bytes) = raw_entries.remove(STYLES_ENTRY) {
            serde_json::from_slice(&style_bytes)?
        } else {
            Stylesheet::standard_legal()
        };

        // 4. Parse document.json
        let doc_bytes =
            raw_entries
                .remove(DOCUMENT_ENTRY)
                .ok_or_else(|| PdocError::MissingRequiredEntry {
                    entry_name: DOCUMENT_ENTRY.to_string(),
                })?;

        // Support both partitioned document content and combined FlowDocument serialization
        let flow_doc =
            if let Ok(content) = serde_json::from_slice::<PdocDocumentContent>(&doc_bytes) {
                FlowDocument {
                    schema_version: content.schema_version,
                    metadata,
                    stylesheet,
                    sections: content.sections,
                    footnotes: content.footnotes,
                    extra: content.extra,
                }
            } else {
                let mut combined: FlowDocument = serde_json::from_slice(&doc_bytes)?;
                combined.metadata = metadata;
                combined.stylesheet = stylesheet;
                combined
            };

        // 5. Parse review.json (optional)
        let review = if let Some(review_bytes) = raw_entries.remove(REVIEW_ENTRY) {
            let rev: PdocReviewData = serde_json::from_slice(&review_bytes)?;
            Some(rev)
        } else {
            None
        };

        // 6. Partition remaining entries into assets/ and unmapped forward-compatible files
        let mut asset_store = PdocAssetStore::new();
        let mut unmapped = BTreeMap::new();

        for (path, bytes) in raw_entries {
            if path.starts_with("assets/") || path.starts_with("assets\\") {
                asset_store.insert_with_path(&path, bytes, None)?;
            } else {
                unmapped.insert(path, bytes);
            }
        }

        // 7. Verify asset cryptographic SHA-256 integrity against manifest
        asset_store.verify_integrity(&manifest.assets)?;

        Ok(PdocPackage {
            manifest,
            document: flow_doc,
            assets: asset_store,
            review,
            unmapped_entries: unmapped,
        })
    }

    /// Writes a `.pdoc` package into a seekable writer formatted as standard ZIP64.
    pub fn write_to_writer<W: Write + Seek>(
        writer: W,
        package: &PdocPackage,
    ) -> Result<(), PdocError> {
        let mut zip = ZipWriter::new(writer);

        let default_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .large_file(true);

        let stored_options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Stored)
            .large_file(true);

        // 1. Serialize and write manifest.json
        let manifest_json = serde_json::to_vec_pretty(&package.manifest)?;
        zip.start_file(MANIFEST_ENTRY, default_options)?;
        zip.write_all(&manifest_json)?;

        // 2. Serialize and write metadata.json
        let metadata_json = serde_json::to_vec_pretty(&package.document.metadata)?;
        zip.start_file(METADATA_ENTRY, default_options)?;
        zip.write_all(&metadata_json)?;

        // 3. Serialize and write styles.json
        let styles_json = serde_json::to_vec_pretty(&package.document.stylesheet)?;
        zip.start_file(STYLES_ENTRY, default_options)?;
        zip.write_all(&styles_json)?;

        // 4. Serialize and write document.json
        let content = PdocDocumentContent {
            schema_version: package.document.schema_version.clone(),
            sections: package.document.sections.clone(),
            footnotes: package.document.footnotes.clone(),
            extra: package.document.extra.clone(),
        };
        let doc_json = serde_json::to_vec_pretty(&content)?;
        zip.start_file(DOCUMENT_ENTRY, default_options)?;
        zip.write_all(&doc_json)?;

        // 5. Serialize review.json if present
        if let Some(rev) = package.review.as_ref().filter(|r| !r.is_empty()) {
            let review_json = serde_json::to_vec_pretty(rev)?;
            zip.start_file(REVIEW_ENTRY, default_options)?;
            zip.write_all(&review_json)?;
        }

        // 6. Write embedded assets
        for (path, bytes) in package.assets.iter() {
            // Already compressed image formats can use Stored compression
            let is_precompressed = path.ends_with(".png")
                || path.ends_with(".jpg")
                || path.ends_with(".jpeg")
                || path.ends_with(".webp");
            let opts = if is_precompressed {
                stored_options
            } else {
                default_options
            };
            zip.start_file(path, opts)?;
            zip.write_all(bytes)?;
        }

        // 7. Write forward-compatible unmapped entries
        for (path, bytes) in &package.unmapped_entries {
            zip.start_file(path, default_options)?;
            zip.write_all(bytes)?;
        }

        zip.finish()?;
        Ok(())
    }

    /// Deserializes a `.pdoc` package from in-memory bytes.
    pub fn read_from_bytes(
        bytes: &[u8],
        limits: &PackageLimitsConfig,
    ) -> Result<PdocPackage, PdocError> {
        let cursor = Cursor::new(bytes);
        Self::read_from_reader(cursor, limits)
    }

    /// Serializes a `.pdoc` package into an in-memory byte buffer.
    pub fn write_to_bytes(package: &PdocPackage) -> Result<Vec<u8>, PdocError> {
        let mut buffer = Cursor::new(Vec::new());
        Self::write_to_writer(&mut buffer, package)?;
        Ok(buffer.into_inner())
    }

    /// Reads a `.pdoc` file from disk under central limits.
    pub fn open<P: AsRef<Path>>(
        path: P,
        limits: &PackageLimitsConfig,
    ) -> Result<PdocPackage, PdocError> {
        let file = std::fs::File::open(path)?;
        Self::read_from_reader(file, limits)
    }

    /// Saves a `.pdoc` package to disk atomically.
    pub fn save<P: AsRef<Path>>(package: &PdocPackage, path: P) -> Result<(), PdocError> {
        let p = path.as_ref();
        let bytes = Self::write_to_bytes(package)?;
        // Temporary file in parent directory for atomic replacement
        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        let temp_path = parent.join(format!(
            ".tmp_{}_{}",
            std::process::id(),
            p.file_name().and_then(|n| n.to_str()).unwrap_or("doc.pdoc")
        ));
        std::fs::write(&temp_path, &bytes)?;
        std::fs::rename(&temp_path, p)?;
        Ok(())
    }
}
