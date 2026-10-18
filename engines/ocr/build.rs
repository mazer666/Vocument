use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=csrc/tesseract_bridge.h");
    println!("cargo:rerun-if-changed=csrc/tesseract_bridge.c");
    println!("cargo:rerun-if-env-changed=TESSERACT_DIR");

    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "android" {
        let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap_or_default());
        let android_lib_dir = manifest_dir.join("android-libs").join("arm64-v8a");
        let include_dir = PathBuf::from("/opt/homebrew/include");

        cc::Build::new()
            .file("csrc/tesseract_bridge.c")
            .include(&include_dir)
            .include("csrc")
            .warnings(true)
            .compile("vocument_tesseract_bridge");

        println!(
            "cargo:rustc-link-search=native={}",
            android_lib_dir.display()
        );
        println!("cargo:rustc-link-lib=dylib=tesseract");
        println!("cargo:rustc-link-lib=dylib=leptonica");
    } else {
        let tesseract_prefix = std::env::var("TESSERACT_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let brew_arm = PathBuf::from("/opt/homebrew/opt/tesseract");
                if brew_arm.exists() {
                    brew_arm
                } else {
                    PathBuf::from("/usr/local/opt/tesseract")
                }
            });

        let include_dir = tesseract_prefix.join("include");
        let lib_dir = tesseract_prefix.join("lib");

        cc::Build::new()
            .file("csrc/tesseract_bridge.c")
            .include(&include_dir)
            .warnings(true)
            .compile("vocument_tesseract_bridge");

        println!("cargo:rustc-link-search=native={}", lib_dir.display());
        println!("cargo:rustc-link-lib=dylib=tesseract");
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir.display());
    }
}
