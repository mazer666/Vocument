//! The identification of the build.
//!
//! Section 6 of the AGPL-3.0 ties every binary that Vocument hands out to
//! the source it was built from. The identification therefore belongs into
//! the binary itself: the product, the version, the revision of the source
//! and — this is the important part — whether the tree was clean when the
//! build was made. A binary from a dirty tree cannot be tied to a revision,
//! because the revision does not contain the changes that were in the tree.
//!
//! The release scripts refuse to publish such a binary
//! (`scripts/release-macos.sh`, `scripts/release-android.sh`).

use serde::{Deserialize, Serialize};

/// The identification of the build, as it is carried by the binary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BuildInfo {
    /// The name of the product.
    pub product_name: String,
    /// The version of the product, from the manifest of the workspace.
    pub version: String,
    /// The revision of the source the binary was built from.
    ///
    /// `unknown` when the build did not happen in a git checkout, for
    /// example when the source was unpacked from an archive.
    pub source_revision: String,
    /// Whether the tree was clean when the build was made.
    ///
    /// `false` means: the binary cannot be tied to the revision, because the
    /// revision does not contain the changes of the tree. Such a binary must
    /// not be published.
    pub source_tree_clean: bool,
    /// The address under which the source of the revision can be obtained.
    pub source_url: String,
    /// The licence of the product.
    pub licence: String,
}

impl BuildInfo {
    /// Reads the identification out of the values that the build script left.
    pub fn current() -> Self {
        let revision = env!("VOCUMENT_SOURCE_REVISION").to_string();
        let clean = env!("VOCUMENT_SOURCE_TREE_CLEAN") == "true";
        Self {
            product_name: "Vocument".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            source_revision: revision,
            source_tree_clean: clean,
            source_url: "https://github.com/mazer666/Vocument".to_string(),
            licence: "AGPL-3.0-or-later".to_string(),
        }
    }

    /// Whether the binary may be published under the AGPL-3.0.
    ///
    /// The reason is written down, so that the release script can put it
    /// into the log of the refusal.
    pub fn publication_blocker(&self) -> Option<String> {
        if !self.source_tree_clean {
            return Some(
                "the tree was not clean when the binary was built: the revision alone does not \
                 describe the source of the binary"
                    .to_string(),
            );
        }
        if self.source_revision == "unknown" || self.source_revision.is_empty() {
            return Some(
                "the revision of the source is unknown: the source of the binary cannot be named"
                    .to_string(),
            );
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::BuildInfo;

    fn info(revision: &str, clean: bool) -> BuildInfo {
        BuildInfo {
            product_name: "Vocument".to_string(),
            version: "0.1.0".to_string(),
            source_revision: revision.to_string(),
            source_tree_clean: clean,
            source_url: "https://github.com/mazer666/Vocument".to_string(),
            licence: "AGPL-3.0-or-later".to_string(),
        }
    }

    #[test]
    fn a_clean_tree_with_a_revision_may_be_published() {
        assert_eq!(info("abc123", true).publication_blocker(), None);
    }

    #[test]
    fn a_dirty_tree_may_not_be_published() {
        let blocker = info("abc123", false).publication_blocker();
        assert!(blocker.is_some());
        assert!(
            blocker.clone().unwrap_or_default().contains("not clean"),
            "{blocker:?}"
        );
    }

    #[test]
    fn an_unknown_revision_may_not_be_published() {
        let blocker = info("unknown", true).publication_blocker();
        assert!(blocker.is_some());
        assert!(
            blocker.clone().unwrap_or_default().contains("unknown"),
            "{blocker:?}"
        );
    }

    #[test]
    fn the_build_of_this_tree_carries_an_identification() {
        let current = BuildInfo::current();
        assert_eq!(current.product_name, "Vocument");
        assert_eq!(current.licence, "AGPL-3.0-or-later");
        assert!(!current.version.is_empty());
    }
}
