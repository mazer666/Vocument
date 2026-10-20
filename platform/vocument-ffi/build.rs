//! Build script of the FFI library.
//!
//! The shared library links against the MuPDF of the host. Without a run
//! path the loader cannot find `libmupdf.so.28.5`, and the library can only
//! be loaded with a hand-set `LD_LIBRARY_PATH`. The Kotlin unit tests and
//! the Swift handshake load the library directly, so the run path is baked
//! into the library.
//!
//! For the Android target the run path is deliberately **not** set: the
//! libraries are packaged next to each other and are found by the loader in
//! the native library directory of the application. A run path with a path of
//! the build machine would leak into the package without any benefit.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=MUPDF_DIR");
    println!("cargo:rerun-if-env-changed=MUPDF_ANDROID_DIR");

    let target = std::env::var("TARGET").unwrap_or_default();
    if target.contains("android") {
        return;
    }

    let prefix = std::env::var("MUPDF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/homebrew/opt/mupdf"));
    let lib_dir = prefix.join("lib");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}
