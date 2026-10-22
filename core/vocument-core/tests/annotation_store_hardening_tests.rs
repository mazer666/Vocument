//! The rail of the profession on disk against the attacks of the red team
//! (`ANNOT-001`).
//!
//! The first round of the red team on this file found three ways to attack
//! the write of the rail:
//!
//! 1. The staging file had a fixed name, so a stranger who prepared that name
//!    as a link had their own file written to.
//! 2. Two applications that save the rail at the same time wrote into the
//!    same staging file; one of them renamed a half-written rail over the
//!    rail of the user.
//! 3. A path with `..`, or a link in place of the file, could carry the write
//!    out of the directory of the user.
//!
//! Every test below measures the file system, not a mock.

#![allow(clippy::unwrap_used)]

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use vocument_core::annotations::{JuristPalette, JuristPaletteEntry};
use vocument_core::annotations_store::JuristPaletteStore;

struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("voc_pal_{name}_{}_{id}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self { path }
    }

    fn palette_path(&self) -> PathBuf {
        self.path
            .join(vocument_core::annotations_store::JURIST_PALETTE_FILE_NAME)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn a_palette() -> JuristPalette {
    JuristPalette::new(vec![
        JuristPaletteEntry::new("house", "Hausfarbe", "Die Farbe des Hauses.", [12, 34, 56]),
        JuristPaletteEntry::new("note", "Hinweis", "Ein Hinweis.", [255, 230, 80]),
    ])
    .expect("the rail")
}

#[test]
fn a_link_of_a_stranger_in_the_place_of_the_staging_file_is_not_written_through() {
    let scratch = Scratch::new("symlink_staging");
    let foreign = scratch.path.join("the_file_of_the_stranger.txt");
    std::fs::write(&foreign, b"the content of the stranger").expect("the foreign file");

    // The stranger prepares the staging file as a link to their own file.
    // The name of the staging file carries the identity of the process, so
    // the stranger has to prepare every name they can think of. The test
    // prepares the name of this process.
    let staging = scratch.path.join(format!(
        ".jurist_palette.json.{}.{}.tmp",
        std::process::id(),
        0
    ));
    #[cfg(unix)]
    std::os::unix::fs::symlink(&foreign, &staging).expect("the link of the stranger");

    let store = JuristPaletteStore::new(scratch.palette_path());
    store.save(&a_palette()).expect("the rail must be written");

    let content = std::fs::read(&foreign).expect("the foreign file");
    assert_eq!(
        content, b"the content of the stranger",
        "the file of the stranger must not have been written to"
    );

    // The rail of the user is there and is complete.
    let loaded = store
        .load()
        .expect("the store must be readable")
        .expect("the rail must be there");
    assert_eq!(loaded, a_palette());
}

#[test]
fn two_applications_that_save_at_the_same_time_leave_a_complete_rail() {
    let scratch = Scratch::new("concurrent");
    let path = scratch.palette_path();
    let palette = a_palette();

    let handles: Vec<_> = (0..8)
        .map(|_| {
            let path = path.clone();
            let palette = palette.clone();
            std::thread::spawn(move || {
                let store = JuristPaletteStore::new(&path);
                for _ in 0..25 {
                    store.save(&palette).expect("the rail must be written");
                }
            })
        })
        .collect();
    for handle in handles {
        handle.join().expect("the writer");
    }

    // The rail is a complete file, not a mixture of two of them.
    let store = JuristPaletteStore::new(&path);
    let loaded = store
        .load()
        .expect("the store must be readable")
        .expect("the rail must be there");
    assert_eq!(loaded, palette, "the rail comes back unchanged");

    // No staging file is left over that holds the rail.
    let leftovers: Vec<_> = std::fs::read_dir(&scratch.path)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.contains(".tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "the staging files have to be renamed away: {leftovers:?}"
    );
}

/// The name of the file decides whether the rail can leave its directory.
///
/// A name can never be `..` — so a file that is named `jurist_palette.json`
/// cannot leave the directory it was given. The *directory* may carry a
/// `..`: that is the part the **platform** chose, and a path like
/// `/a/b/../c` means the same directory as `/a/c`. The second round of the
/// red team measured that the old rule refused the whole path and bricked
/// the application at the start for a path that is not an attack.
#[test]
fn a_file_that_is_not_the_rail_cannot_leave_its_directory() {
    let scratch = Scratch::new("parent_dir");
    // The name of the file is `..` — it points at the directory itself.
    let path = scratch.path.join("sub").join("..");
    let store = JuristPaletteStore::new(&path);
    let error = store
        .save(&a_palette())
        .expect_err("the path has to be refused");
    assert!(error.to_string().contains("jurist_palette.json"), "{error}");
}

/// A `..` in the **directory** the platform chose is not an attack: the rail
/// lands in the directory the path names, and the check of the owner still
/// applies.
#[test]
fn a_directory_with_a_dot_dot_is_accepted_and_lands_in_the_named_directory() {
    let scratch = Scratch::new("parent_dir_allowed");
    std::fs::create_dir_all(scratch.path.join("sub")).expect("the sub directory");
    let path = scratch
        .path
        .join("sub")
        .join("..")
        .join("jurist_palette.json");
    let store = JuristPaletteStore::new(&path);
    store
        .save(&a_palette())
        .expect("the path of the platform may not be refused");
    assert!(
        scratch.path.join("jurist_palette.json").exists(),
        "the rail has to land in the directory the path names"
    );
    assert!(
        !scratch
            .path
            .join("sub")
            .join("jurist_palette.json")
            .exists(),
        "the rail may not land in another directory"
    );
}

/// A link that points **nowhere** is refused like every other link.
///
/// The old rule asked `Path::exists()`, which follows a link. A link that
/// points nowhere therefore looked like "no rail at all", and the
/// application carried on with the standard rail — **silently**. The notes
/// of the reviewer would then mean something else than the user thinks.
#[test]
fn a_link_that_points_nowhere_is_refused_and_not_taken_for_no_rail() {
    let scratch = Scratch::new("dangling_link");
    #[cfg(unix)]
    std::os::unix::fs::symlink(
        scratch.path.join("there_is_nothing_there"),
        scratch.palette_path(),
    )
    .expect("the link");

    let store = JuristPaletteStore::new(scratch.palette_path());
    let error = store
        .load()
        .expect_err("a link that points nowhere is not a missing rail");
    let text = error.to_string();
    assert!(
        text.contains("a link") || text.contains("not a plain file"),
        "{text}"
    );
}

/// The rail of the user may only be read where the user owns it.
///
/// The write path has always checked that. The read path did not, so a rail
/// that belonged to a stranger was **used**: the notes of the reviewer were
/// written with the colours of the stranger, and the meaning of the document
/// changed silently. The second round of the red team measured exactly
/// that.
#[test]
fn a_rail_of_a_stranger_is_not_read() {
    let scratch = Scratch::new("stranger_rail");
    let store = JuristPaletteStore::new(scratch.palette_path());
    store.save(&a_palette()).expect("the rail must be written");

    // The rail now belongs to somebody else. A process that may give a file
    // to another user (root) can build the situation and must refuse it all
    // the same. A process that may **not** (a normal user, the runner of the
    // continuous integration) cannot build the situation at all: the proof is
    // then NOT RUN on that machine, and it says so instead of pretending.
    #[cfg(unix)]
    {
        use std::os::unix::fs::chown;
        let uid = 1000;
        let gid = 1000;
        if chown(scratch.palette_path(), Some(uid), Some(gid)).is_err() {
            eprintln!(
                "NOT RUN: the rail could not be given to another user (the process has no \
                 privilege to do so). The rail of a stranger was not examined here."
            );
            return;
        }
        chown(&scratch.path, Some(uid), Some(gid)).expect("the directory of the stranger");

        let error = store
            .load()
            .expect_err("the rail of a stranger may not be read");
        let text = error.to_string();
        assert!(
            text.contains("belongs to the user"),
            "the refusal has to name the reason: {text}"
        );
    }
}

#[test]
fn a_file_with_another_name_is_refused() {
    let scratch = Scratch::new("other_name");
    let path = scratch.path.join("something_else.json");
    let store = JuristPaletteStore::new(&path);
    let error = store
        .save(&a_palette())
        .expect_err("the name has to be refused");
    assert!(error.to_string().contains("jurist_palette.json"), "{error}");
    assert!(!path.exists(), "nothing may be written");
}

#[test]
fn a_link_in_the_place_of_the_rail_is_not_read() {
    let scratch = Scratch::new("symlink_rail");
    let foreign = scratch.path.join("the_file_of_the_stranger.json");
    std::fs::write(&foreign, r#"{"format_version": 1, "entries": []}"#).expect("the foreign file");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&foreign, scratch.palette_path()).expect("the link");

    let store = JuristPaletteStore::new(scratch.palette_path());
    let error = store
        .load()
        .expect_err("a link is not the rail of the user");
    assert!(error.to_string().contains("not a plain file"), "{error}");
}

#[test]
fn the_directory_of_the_rail_belongs_to_the_user_alone() {
    let scratch = Scratch::new("directory_mode");
    let store = JuristPaletteStore::new(scratch.palette_path());
    store.save(&a_palette()).expect("the rail must be written");

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&scratch.path)
            .expect("the directory")
            .permissions()
            .mode();
        assert_eq!(
            mode & 0o077,
            0,
            "the directory of the rail must not be reachable for others: {mode:o}"
        );
        let file_mode = std::fs::metadata(scratch.palette_path())
            .expect("the file")
            .permissions()
            .mode();
        assert_eq!(file_mode & 0o077, 0, "the file: {file_mode:o}");
    }
}

#[test]
fn a_rail_that_was_cut_in_half_is_an_error_and_not_a_silent_standard_rail() {
    let scratch = Scratch::new("half_written");
    let path = scratch.palette_path();
    let store = JuristPaletteStore::new(&path);
    store.save(&a_palette()).expect("the rail must be written");

    // A crash in the middle of the write leaves the file of the user either
    // complete or untouched. A file that is cut in half is an error, never a
    // silent fall back to the standard rail.
    std::fs::write(&path, b"{\"format_version\": 1, \"entries\": [").expect("the cut file");
    let error = store.load().expect_err("a broken rail is an error");
    let text = error.to_string();
    assert!(
        text.contains("could not be read") || text.contains("is not valid"),
        "{text}"
    );
}
