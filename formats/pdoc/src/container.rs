//! Package container serialization, ZIP64 layout, and roundtrip processing (`ADR-0020`, `SOURCE-001`, `SOURCE-002`).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::io::{Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use vocument_core::config::PackageLimitsConfig;
use vocument_flow::metadata::DocumentMetadata;
use vocument_flow::model::{Block, FlowDocument, FootnoteDefinition, Section};
use vocument_flow::styles::Stylesheet;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use crate::assets::PdocAssetStore;
use crate::error::PdocError;
use crate::manifest::PdocManifest;
use crate::review::PdocReviewData;
use crate::security::{
    normalize_for_collision_check, read_entry_bounded, validate_compression_method,
    validate_entry_limits, validate_entry_path,
};

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

/// Fingerprint representing the file size, modification timestamp, and SHA-256 digest on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PdocFileFingerprint {
    pub file_size: u64,
    pub mtime_secs: u64,
    pub sha256: String,
}

impl PdocFileFingerprint {
    /// Computes the disk fingerprint of a `.pdoc` file.
    pub fn compute<P: AsRef<Path>>(path: P) -> Result<Self, std::io::Error> {
        let p = path.as_ref();
        let meta = std::fs::metadata(p)?;
        let file_size = meta.len();
        let mtime_secs = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let bytes = std::fs::read(p)?;
        let sha256 = PdocAssetStore::compute_sha256(&bytes);
        Ok(Self {
            file_size,
            mtime_secs,
            sha256,
        })
    }
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

    /// Comprehensive validation of schema invariants, asset integrity, and cross-references.
    pub fn validate(&self) -> Result<(), PdocError> {
        // 1. Validate manifest constraints
        self.manifest.validate()?;

        // 2. Validate FlowDocument schema invariants
        vocument_flow::schema::validate_document(&self.document)?;

        // 3. Validate asset cryptographic integrity against manifest
        self.assets.verify_integrity(&self.manifest.assets)?;

        // 4. Ensure no orphaned/unregistered assets in storage
        for path in self.assets.keys() {
            if !self.manifest.assets.contains_key(path) {
                return Err(PdocError::CorruptContainer {
                    details: format!(
                        "asset '{path}' in assets store is not registered in manifest"
                    ),
                });
            }
        }

        // 5. Cross-validate document references (image asset_refs, styles)
        self.validate_references()?;

        Ok(())
    }

    fn validate_references(&self) -> Result<(), PdocError> {
        for section in &self.document.sections {
            for block in section
                .blocks
                .iter()
                .chain(section.header_blocks.iter())
                .chain(section.footer_blocks.iter())
            {
                self.validate_block_references(block)?;
            }
        }
        for footnote in self.document.footnotes.values() {
            for block in &footnote.blocks {
                self.validate_block_references(block)?;
            }
        }
        Ok(())
    }

    fn validate_block_references(&self, block: &Block) -> Result<(), PdocError> {
        match block {
            Block::Image(img) => {
                let path = if img.asset_ref.starts_with("assets/")
                    || img.asset_ref.starts_with("assets\\")
                {
                    img.asset_ref.clone()
                } else {
                    format!("assets/{}", img.asset_ref.trim_start_matches('/'))
                };
                if !self.assets.contains(&path) && !self.assets.contains(&img.asset_ref) {
                    return Err(PdocError::MissingReferencedAsset {
                        asset_ref: img.asset_ref.clone(),
                        referrer: format!("ImageBlock(id='{}')", img.id),
                    });
                }
            }
            Block::Paragraph(para) => {
                if let Some(ref sid) = para.style_id
                    && !self.document.stylesheet.paragraph_styles.contains_key(sid)
                {
                    return Err(PdocError::UndefinedStyleReference {
                        style_id: sid.clone(),
                        element_id: para.id.clone(),
                    });
                }
                for span in &para.spans {
                    if let Some(ref fn_id) = span.footnote_ref
                        && !self.document.footnotes.contains_key(fn_id)
                    {
                        return Err(PdocError::CorruptContainer {
                            details: format!(
                                "span references undefined footnote '{fn_id}' in paragraph '{}'",
                                para.id
                            ),
                        });
                    }
                }
            }
            Block::Heading(h) => {
                for span in &h.spans {
                    if let Some(ref fn_id) = span.footnote_ref
                        && !self.document.footnotes.contains_key(fn_id)
                    {
                        return Err(PdocError::CorruptContainer {
                            details: format!(
                                "span references undefined footnote '{fn_id}' in heading '{}'",
                                h.id
                            ),
                        });
                    }
                }
            }
            Block::Table(tbl) => {
                if let Some(ref sid) = tbl.style_id
                    && !self.document.stylesheet.table_styles.contains_key(sid)
                {
                    return Err(PdocError::UndefinedStyleReference {
                        style_id: sid.clone(),
                        element_id: tbl.id.clone(),
                    });
                }
                for row in &tbl.rows {
                    for cell in &row.cells {
                        for cell_block in &cell.blocks {
                            self.validate_block_references(cell_block)?;
                        }
                    }
                }
            }
            Block::List(list) => {
                if let Some(ref sid) = list.style_id
                    && !self.document.stylesheet.list_styles.contains_key(sid)
                {
                    return Err(PdocError::UndefinedStyleReference {
                        style_id: sid.clone(),
                        element_id: list.id.clone(),
                    });
                }
                for item in &list.items {
                    if let Some(ref nested) = item.nested_list {
                        let nested_block = Block::List(*nested.clone());
                        self.validate_block_references(&nested_block)?;
                    }
                }
            }
            Block::Callout(callout) => {
                for c_block in &callout.blocks {
                    self.validate_block_references(c_block)?;
                }
            }
            _ => {}
        }
        Ok(())
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
        let mut seen_collision_keys = HashSet::new();
        let mut accumulated_bytes: u64 = 0;
        let max_archive_bytes = limits.max_pdoc_archive_size_mb.saturating_mul(1024 * 1024);

        for i in 0..entry_count {
            let mut file = archive.by_index(i)?;
            let raw_name = file.name().to_string();

            // Validate compression method: only Stored and Deflated allowed
            validate_compression_method(&raw_name, file.compression())?;

            // Directory entries are skipped after validation
            let is_dir = file.is_dir() || raw_name.ends_with('/') || raw_name.ends_with('\\');
            let safe_path = validate_entry_path(&raw_name)?;
            let normalized_name = safe_path.to_string_lossy().to_string();

            // Detect duplicate or colliding entries (e.g. case differences or path variations)
            let collision_key = normalize_for_collision_check(&raw_name);
            if !seen_collision_keys.insert(collision_key.clone()) {
                return Err(PdocError::DuplicateEntry {
                    path: raw_name,
                    details: format!("collision with existing entry under key '{collision_key}'"),
                });
            }

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

            let remaining_quota = max_archive_bytes.saturating_sub(accumulated_bytes);
            let buffer = read_entry_bounded(
                &mut file,
                &normalized_name,
                compressed_size,
                uncompressed_size,
                remaining_quota,
                limits,
            )?;

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

        let package = PdocPackage {
            manifest,
            document: flow_doc,
            assets: asset_store,
            review,
            unmapped_entries: unmapped,
        };

        // 7. Full validation of package invariants and cross-references
        package.validate()?;

        Ok(package)
    }

    /// Writes a `.pdoc` package into a seekable writer formatted as standard ZIP64.
    pub fn write_to_writer<W: Write + Seek>(
        writer: W,
        package: &PdocPackage,
    ) -> Result<(), PdocError> {
        // Enforce invariants prior to write
        package.validate()?;

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

    /// Saves a `.pdoc` package to disk atomically, guarding against external modification conflicts.
    pub fn save_with_conflict_check<P: AsRef<Path>>(
        package: &PdocPackage,
        path: P,
        expected_prior_fingerprint: Option<&PdocFileFingerprint>,
    ) -> Result<PdocFileFingerprint, PdocError> {
        let p = path.as_ref();

        // Conflict check: if file exists and expected prior fingerprint was provided, verify it has not changed
        if p.exists()
            && let Some(expected) = expected_prior_fingerprint
        {
            let actual = PdocFileFingerprint::compute(p).map_err(|e| {
                PdocError::ExternalConflictDetected {
                    path: p.display().to_string(),
                    details: format!("could not inspect current file fingerprint: {e}"),
                }
            })?;
            if actual != *expected {
                return Err(PdocError::ExternalConflictDetected {
                    path: p.display().to_string(),
                    details: format!(
                        "file on disk was modified by an external process (expected sha256={}, current sha256={})",
                        expected.sha256, actual.sha256
                    ),
                });
            }
        }

        let bytes = Self::write_to_bytes(package)?;

        let parent = p.parent().unwrap_or_else(|| Path::new("."));
        let temp_path = parent.join(format!(
            ".tmp_{}_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0),
            p.file_name().and_then(|n| n.to_str()).unwrap_or("doc.pdoc")
        ));

        // Write and fsync temporary file
        {
            let mut file = std::fs::File::create(&temp_path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }

        // Atomic rename into target path
        std::fs::rename(&temp_path, p)?;

        // Directory sync on Unix systems to ensure directory entry durability
        #[cfg(unix)]
        if let Ok(dir) = std::fs::File::open(parent) {
            let _ = dir.sync_all();
        }

        PdocFileFingerprint::compute(p).map_err(PdocError::Io)
    }

    /// Saves a collision-free conflict copy of the document so user edits are never lost.
    pub fn save_conflict_copy<P: AsRef<Path>>(
        package: &PdocPackage,
        original_path: P,
    ) -> Result<PathBuf, PdocError> {
        let orig = original_path.as_ref();
        let parent = orig.parent().unwrap_or_else(|| Path::new("."));
        let stem = orig
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("document");
        let ts = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let conflict_path = parent.join(format!("{stem}.conflict.{ts}.pdoc"));
        Self::save_with_conflict_check(package, &conflict_path, None)?;
        Ok(conflict_path)
    }

    /// Saves a `.pdoc` package to disk atomically.
    pub fn save<P: AsRef<Path>>(package: &PdocPackage, path: P) -> Result<(), PdocError> {
        Self::save_with_conflict_check(package, path, None).map(|_| ())
    }
}
