//! The build script of the core.
//!
//! It leaves the revision of the source and the state of the tree in the
//! environment of the build, so that the binary can carry the identification
//! that section 6 of the AGPL-3.0 asks for.
//!
//! The revision can also be given from the outside, with the environment
//! variable `VOCUMENT_SOURCE_REVISION` and `VOCUMENT_SOURCE_TREE_CLEAN`.
//! The release scripts use that: they set the revision of the tag that is
//! being released, so that the binary names the revision even when the
//! build happens outside of the checkout.

use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=VOCUMENT_SOURCE_REVISION");
    println!("cargo:rerun-if-env-changed=VOCUMENT_SOURCE_TREE_CLEAN");

    let revision = std::env::var("VOCUMENT_SOURCE_REVISION")
        .ok()
        .unwrap_or_else(|| {
            git(&["rev-parse", "HEAD"])
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "unknown".to_string())
        });

    let clean = std::env::var("VOCUMENT_SOURCE_TREE_CLEAN")
        .ok()
        .map(|value| value == "true")
        .unwrap_or_else(|| {
            git(&["status", "--porcelain"])
                .map(|value| value.trim().is_empty())
                .unwrap_or(false)
        });

    println!("cargo:rustc-env=VOCUMENT_SOURCE_REVISION={revision}");
    println!("cargo:rustc-env=VOCUMENT_SOURCE_TREE_CLEAN={clean}");
}

/// Runs a git command in the root of the workspace and returns its output.
fn git(arguments: &[&str]) -> Option<String> {
    let output = Command::new("git").args(arguments).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}
