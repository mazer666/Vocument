//! The identification of the build across the boundary.
//!
//! Section 6 of the AGPL-3.0 asks every binary that Vocument hands out to
//! name the source it was built from. These tests check that the
//! identification really travels — and that a binary that cannot be tied to
//! a revision says so instead of pretending to be releasable.

use vocument_ffi::build_info_dto::document_core_build_info;
mod common;

#[test]
fn the_build_carries_the_identification_of_its_source() {
    common::use_a_temporary_data_directory();
    let info = document_core_build_info();

    assert_eq!(info.product_name, "Vocument");
    assert_eq!(info.licence, "AGPL-3.0-or-later");
    assert!(!info.version.is_empty());
    assert!(!info.source_revision.is_empty());
    assert!(info.source_url.contains("github.com"));
}

#[test]
fn the_build_that_was_built_from_a_dirty_tree_names_the_reason() {
    common::use_a_temporary_data_directory();
    let info = document_core_build_info();

    // The tree of this checkout carries the changes of the working state, so
    // the binary must not be publishable — and it has to say why.
    if !info.source_tree_clean {
        assert!(
            !info.publication_blocker.is_empty(),
            "a build from a dirty tree must name the reason why it may not be published"
        );
        assert!(info.publication_blocker.contains("not clean"));
    } else {
        assert!(
            info.publication_blocker.is_empty(),
            "a clean build with a revision may be published: {0}",
            info.publication_blocker
        );
    }
}
