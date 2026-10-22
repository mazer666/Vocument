//! The helpers that the end-to-end tests of the FFI layer share.
//!
//! Two things have to be true for a test of the FFI layer, and neither of
//! them is true by accident:
//!
//! 1. **The tests do not write into the data directory of the user.** The
//!    recovery store holds the document of the user in plain text, and the
//!    rail of the profession is a setting of the user. A test run that wrote
//!    into `~/Library/Application Support/Vocument` (macOS) would overwrite
//!    the rail of the user and leave the documents of the test in the
//!    recovery store of the user. The directory of the tests is therefore a
//!    temporary one, and it is set **before** the first document is opened:
//!    the registry of the sessions is created once, and it takes the
//!    directory that is configured at that moment.
//!
//! 2. **The tests that read or write the rail of the profession do not run
//!    at the same time.** The rail lives in the registry of the sessions, and
//!    the registry is shared by the tests of a binary. A test that replaces
//!    the rail would otherwise change it under a test that is reading it —
//!    the result would be a failure that appears and disappears at random.
//!    The lock of this module is what keeps them apart.

use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, OnceLock};

use vocument_ffi::annotation_dto::{
    document_core_configure_data_directory, document_core_default_jurist_palette,
    document_core_set_jurist_palette,
};

/// The lock that keeps the tests of the rail apart from each other.
///
/// Not every test binary of the FFI layer has a test of the rail. The helper
/// is therefore part of every binary — the directory of the data belongs to
/// every test — and the items of the rail are unused in the binaries that do
/// not need them. That is why they carry the `dead_code` exemption: it says
/// "this is not used here", and not "this is not needed".
#[allow(dead_code)]
fn rail_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// The rail of the profession, held for the duration of a test.
///
/// The rail lives in the registry of the sessions, and the registry is shared
/// by the tests of a binary. Two things are therefore done here:
///
/// * the tests are kept apart from each other, so that a test that replaces
///   the rail does not do it under a test that is reading it;
/// * the standard rail is put back when the test ends — also when the test
///   failed, and also before the lock is released, so that the next test does
///   not find the rail of the test before it. Without that, the order of the
///   tests decides whether they pass, and the result of a run would change
///   from run to run.
#[allow(dead_code)]
pub struct RailGuard {
    _lock: MutexGuard<'static, ()>,
}

impl Drop for RailGuard {
    fn drop(&mut self) {
        let _ = document_core_set_jurist_palette(document_core_default_jurist_palette());
    }
}

/// Takes the rail of the profession for the duration of a test.
#[allow(dead_code)]
pub fn the_rail_of_the_user() -> RailGuard {
    // A test that failed while it held the lock must not lock the others out:
    // the lock is taken again, and the reason of the failure stays in the
    // output of the test that failed.
    RailGuard {
        _lock: rail_lock()
            .lock()
            .unwrap_or_else(|error| error.into_inner()),
    }
}

/// Points the data directory of the FFI layer at a temporary directory.
///
/// The function has to be called by **every** test of a binary before it
/// touches the registry of the sessions: the registry is created once, and it
/// takes the directory that is configured at that moment. A test that
/// reaches the registry before this function has run would write into the
/// directory of the user — and the call of this function would then fail with
/// a name that says so, instead of the breach passing unnoticed.
pub fn use_a_temporary_data_directory() {
    static CONFIGURED: OnceLock<()> = OnceLock::new();
    CONFIGURED.get_or_init(|| {
        let directory = temporary_data_directory();
        // The directory of the last run is removed first: the tests of a
        // binary leave their documents behind, and a run must not find the
        // documents of the run before it.
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory)
            .expect("the temporary data directory of the tests must be creatable");
        document_core_configure_data_directory(directory.to_string_lossy().into_owned())
            .expect(
                "the data directory of the tests must be configurable before the first document is opened",
            );
    });
}

/// The directory that the tests of one binary use.
///
/// The name carries the process id, so that two test binaries that run at the
/// same time do not share one directory.
fn temporary_data_directory() -> PathBuf {
    std::env::temp_dir().join(format!("vocument-ffi-tests-{}", std::process::id()))
}
