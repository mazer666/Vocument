//! The privacy of the recovery store (`RECOVERY-001`).
//!
//! The store holds the document of the user in plain text. It therefore only
//! writes into a directory that belongs to the user alone.
//!
//! The fourth round of the red team measured two defects:
//!
//! 1. A directory that belonged to a stranger was accepted. The store would
//!    then write the document of the user into a place somebody else owns.
//! 2. The application printed the error of the private root to the standard
//!    error and continued as if the recovery were protected.
//!
//! The tests below hold both: a directory of a stranger is refused, and a
//! store that may not write keeps the document out of the directory instead
//! of handing it to somebody else.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vocument_core::recovery::{RecoveryContainer, RecoveryError, RecoveryStore};

/// A directory of the test that is removed again afterwards.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("voc_priv_{name}_{}_{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

#[test]
fn a_store_that_may_not_write_keeps_the_document_out_of_the_directory() {
    let scratch = Scratch::new("disabled");
    let root = scratch.path.join("vocument_recovery");
    let store = RecoveryStore::disabled(
        root.clone(),
        "the recovery directory belongs to somebody else".to_owned(),
    );

    assert!(store.is_disabled());
    assert_eq!(
        store.disabled_reason(),
        Some("the recovery directory belongs to somebody else")
    );

    let container = store.container_for("doc-1");
    let error = container
        .write_checkpoint(1, b"%PDF-1.4 the secret of the user")
        .expect_err("the checkpoint must not be written into a directory that is not private");
    assert!(
        matches!(error, RecoveryError::NotPrivate { .. }),
        "the error has to name the reason: {error}"
    );
    assert!(error.to_string().contains("not private"), "{error}");

    // The document of the user is nowhere in that directory.
    assert!(
        !root.join("doc-1").exists(),
        "the container must not even be created"
    );
    let leftovers = std::fs::read_dir(&root)
        .map(|entries| entries.flatten().count())
        .unwrap_or(0);
    assert_eq!(leftovers, 0, "nothing may be written into that directory");
}

#[test]
fn a_store_that_may_not_write_refuses_the_journal_as_well() {
    let scratch = Scratch::new("disabled_journal");
    let root = scratch.path.join("vocument_recovery");
    let store = RecoveryStore::disabled(root.clone(), "the directory is not private".to_owned());

    let container = store.container_for("doc-2");
    let journal = vocument_core::transactions::PersistentTransactionJournal {
        schema_version: 1,
        document_id: "doc_test".to_owned(),
        base_pdf_sha256: "0".repeat(64),
        current_revision: 1,
        saved_revision: 1,
        committed_records: Vec::new(),
        redo_records: Vec::new(),
        journal_checksum_sha256: String::new(),
    };
    let error = container
        .write_journal(&journal)
        .expect_err("the journal must not be written either");
    assert!(matches!(error, RecoveryError::NotPrivate { .. }), "{error}");
    assert!(!root.join("doc-2").exists());
}

#[test]
fn a_store_that_may_not_write_finds_no_recoverable_document() {
    let scratch = Scratch::new("disabled_scan");
    let root = scratch.path.join("vocument_recovery");
    let store = RecoveryStore::disabled(root, "the directory is not private".to_owned());
    let broker = vocument_core::file_broker::ScopedLocalFileBroker::new();
    assert!(
        store.scan_recoverable_documents(&broker).is_empty(),
        "a store that writes nothing holds no recovery data"
    );
}

#[test]
fn a_directory_of_a_stranger_is_refused() {
    let scratch = Scratch::new("stranger");
    let root = scratch.path.join("vocument_recovery");
    std::fs::create_dir_all(&root).unwrap();

    // The directory is handed to another user. The test is only meaningful
    // when the process may give a directory away — otherwise it is skipped
    // and the check stays untested on that machine.
    #[cfg(unix)]
    {
        use std::os::unix::fs::chown;
        if chown(&root, Some(1000), None).is_err() {
            eprintln!(
                "the directory could not be given to another user: the check is not exercised"
            );
            return;
        }
    }

    let error = RecoveryContainer::create_private_root(&root)
        .expect_err("a directory of a stranger must be refused");
    let text = error.to_string();
    assert!(
        text.contains("belongs to the user") || text.contains("not private"),
        "the error has to name the problem: {text}"
    );
}

#[test]
fn a_directory_of_the_user_is_accepted() {
    let scratch = Scratch::new("owner");
    let root = scratch.path.join("vocument_recovery");
    RecoveryContainer::create_private_root(&root).expect("the directory of the user is accepted");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&root).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "the directory has to stay private");
    }
}
