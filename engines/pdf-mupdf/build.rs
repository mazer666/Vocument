use std::path::{Path, PathBuf};

/// Directory that contains the MuPDF installation for the current target.
///
/// The value is resolved per target so that a host build and an Android build can
/// use different MuPDF installations. `MUPDF_DIR` overrides the default.
fn mupdf_prefix() -> PathBuf {
    std::env::var("MUPDF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/homebrew/opt/mupdf"))
}

/// Directory that contains the Android MuPDF installation.
///
/// Android links against the MuPDF build produced by
/// `scripts/prepare-android-mupdf.sh`, which cross-compiles MuPDF 1.28.5 for
/// `aarch64-linux-android` with the Android NDK. The directory is overridable
/// through `MUPDF_ANDROID_DIR` so that CI and developer machines can place the
/// installation anywhere.
fn android_mupdf_prefix() -> PathBuf {
    std::env::var("MUPDF_ANDROID_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("target/mupdf-android"))
}

/// Emits the link directives for a MuPDF installation.
fn link_mupdf(prefix: &Path, library_name: &str) {
    let lib_dir = prefix.join("lib");
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib={library_name}");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}

/// Reads the MuPDF version of the installation that is being linked.
///
/// The version is taken from the MuPDF header, so the runtime engine identifier
/// always matches the library that is actually linked.
fn mupdf_version(prefix: &Path) -> String {
    let header_path = prefix.join("include/mupdf/fitz/version.h");
    if let Ok(content) = std::fs::read_to_string(&header_path)
        && let Some(index) = content.find("FZ_VERSION")
    {
        let remainder = &content[index..];
        if let Some(start) = remainder.find('"')
            && let Some(end) = remainder[start + 1..].find('"')
        {
            return remainder[start + 1..start + 1 + end].to_owned();
        }
    }
    "unknown".to_owned()
}

/// Returns a short label of the build target, used in the runtime engine id.
fn build_target_label() -> String {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    format!("{target_arch}-{target_os}")
}

fn main() {
    println!("cargo:rerun-if-changed=csrc/mupdf_bridge.c");
    println!("cargo:rerun-if-changed=csrc/mupdf_text_bridge.c");
    println!("cargo:rerun-if-changed=csrc/mupdf_mutate_bridge.c");
    println!("cargo:rerun-if-env-changed=MUPDF_DIR");
    println!("cargo:rerun-if-env-changed=MUPDF_ANDROID_DIR");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let prefix = if target_os == "android" {
        android_mupdf_prefix()
    } else {
        mupdf_prefix()
    };

    let include_dir = prefix.join("include");
    let lib_dir = prefix.join("lib");

    if !include_dir.is_dir() {
        panic!(
            "MuPDF headers not found at '{}'.\n\
             Install MuPDF 1.28.5 for this target or set MUPDF_DIR / MUPDF_ANDROID_DIR.\n\
             For Android use scripts/prepare-android-mupdf.sh.",
            include_dir.display()
        );
    }
    if !lib_dir.is_dir() {
        panic!(
            "MuPDF libraries not found at '{}'.\n\
             Install MuPDF 1.28.5 for this target or set MUPDF_DIR / MUPDF_ANDROID_DIR.\n\
             For Android use scripts/prepare-android-mupdf.sh.",
            lib_dir.display()
        );
    }

    let engine_id = format!("{}-{}", mupdf_version(&prefix), build_target_label());
    println!("cargo:rustc-env=VOCRUMENT_MUPDF_ENGINE_ID={engine_id}");

    cc::Build::new()
        .file("csrc/mupdf_bridge.c")
        .file("csrc/mupdf_text_bridge.c")
        .file("csrc/mupdf_mutate_bridge.c")
        .include(&include_dir)
        .define(
            "VOCRUMENT_MUPDF_ENGINE_ID",
            format!("\"{engine_id}\"").as_str(),
        )
        .warnings(true)
        .compile("vocument_mupdf_bridge");

    link_mupdf(&prefix, "mupdf");
}
