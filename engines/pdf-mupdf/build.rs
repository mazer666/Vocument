use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/mupdf_bridge.c");
    println!("cargo:rerun-if-changed=csrc/mupdf_text_bridge.c");
    println!("cargo:rerun-if-env-changed=MUPDF_DIR");

    let mupdf_prefix = std::env::var("MUPDF_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/opt/homebrew/opt/mupdf"));

    let include_dir = mupdf_prefix.join("include");
    let lib_dir = mupdf_prefix.join("lib");

    cc::Build::new()
        .file("csrc/mupdf_bridge.c")
        .file("csrc/mupdf_text_bridge.c")
        .include(&include_dir)
        .warnings(true)
        .compile("vocument_mupdf_bridge");

    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    println!("cargo:rustc-link-lib=dylib=mupdf");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
}
