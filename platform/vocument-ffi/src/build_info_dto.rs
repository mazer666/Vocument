//! The identification of the build across the UniFFI boundary.
//!
//! Section 6 of the AGPL-3.0 asks every binary that Vocument hands out to
//! name the source it was built from. The identification therefore travels
//! through the boundary into the application, so that the application can
//! show it in its "about" area and so that the release scripts can refuse
//! a binary that cannot be tied to a revision.

use vocument_core::build_info::BuildInfo;

/// The identification of the build that carries the library.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct FfiBuildInfo {
    /// The name of the product.
    pub product_name: String,
    /// The version of the product.
    pub version: String,
    /// The revision of the source the binary was built from.
    pub source_revision: String,
    /// Whether the tree was clean when the build was made.
    ///
    /// `false` means: the binary cannot be tied to the revision alone.
    pub source_tree_clean: bool,
    /// The address under which the source of the revision can be obtained.
    pub source_url: String,
    /// The licence of the product.
    pub licence: String,
    /// Why the binary may not be published — empty when it may.
    pub publication_blocker: String,
}

impl From<BuildInfo> for FfiBuildInfo {
    fn from(info: BuildInfo) -> Self {
        let blocker = info.publication_blocker().unwrap_or_default();
        Self {
            product_name: info.product_name,
            version: info.version,
            source_revision: info.source_revision,
            source_tree_clean: info.source_tree_clean,
            source_url: info.source_url,
            licence: info.licence,
            publication_blocker: blocker,
        }
    }
}

/// Returns the identification of the build that carries the library.
#[uniffi::export]
pub fn document_core_build_info() -> FfiBuildInfo {
    BuildInfo::current().into()
}
