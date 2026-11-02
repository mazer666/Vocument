//! Resource limits applied to every foreign Office file (`SEC` threat model: ZIP/XML bombs).

/// Upper bounds for reading an untrusted Office package.
///
/// The defaults are generous for real documents (a 100-page scan-heavy report is well inside
/// them) and small enough that a hostile file cannot exhaust the memory of a phone.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OfficeLimits {
    /// Largest accepted package file.
    pub max_package_bytes: u64,
    /// Largest accepted number of ZIP entries.
    pub max_entries: usize,
    /// Largest accepted uncompressed size of a single entry.
    pub max_entry_bytes: u64,
    /// Largest accepted sum of all uncompressed entries.
    pub max_total_bytes: u64,
    /// Largest accepted uncompressed size of one XML part.
    pub max_xml_part_bytes: u64,
    /// Compression ratio above which a large entry is treated as a decompression bomb.
    pub max_compression_ratio: u64,
    /// Entries below this size are exempt from the ratio check (tiny files compress extremely).
    pub ratio_check_threshold_bytes: u64,
    /// Largest accepted XML element nesting depth.
    pub max_xml_depth: usize,
    /// Largest accepted number of XML nodes (elements and text) in one part.
    pub max_xml_nodes: usize,
    /// Largest accepted number of attributes on one element.
    pub max_xml_attributes: usize,
}

impl Default for OfficeLimits {
    fn default() -> Self {
        const MIB: u64 = 1024 * 1024;
        Self {
            max_package_bytes: 256 * MIB,
            max_entries: 20_000,
            max_entry_bytes: 128 * MIB,
            max_total_bytes: 512 * MIB,
            max_xml_part_bytes: 96 * MIB,
            max_compression_ratio: 1000,
            ratio_check_threshold_bytes: MIB,
            max_xml_depth: 200,
            max_xml_nodes: 8_000_000,
            max_xml_attributes: 256,
        }
    }
}
