#!/usr/bin/env python3
"""The manifest of a release candidate of Vocument.

The manifest is written from the facts that `scripts/release-candidate.sh`
measured: the facts, the checks and the artefacts. The machine readable
manifest (`manifest.json`) and the readable one (`MANIFEST.md`) come out of
the same facts, so that they cannot say different things.

The manifest draws the difference between the artefacts that are proven to be
bit for bit the same (the source archive) and the applications, whose
binaries are **not** claimed to be equal. What is equal instead — the
checksums of the sources, the version, the checks that were run — is named.

Usage: scripts/release-candidate-manifest.py --facts <file> --checks <file>
                                             --artifacts <file> --output <directory>
"""

import argparse
import json
import os

SCHEMA = "vocument.release-candidate/1"

PUBLICATION_NOTICE = (
    "This release candidate is NOT published. No upload, no release, no tag, "
    "no store and no announcement are part of it: the legal compliance review "
    "(E5) of the licensing decision is open, and only a person can give it "
    "(B-1 of docs/releasing/RELEASE-BLOCKERS.md). The candidate stays in "
    "target/release-candidate and in the repository document "
    "docs/releasing/RELEASE-CANDIDATE.md."
)

APPLICATION_REASON = (
    "The applications are not proven to be bit-identical. They hang on the "
    "versions of the tools, on the stamps in the bundle and on the signature. "
    "The candidate measures what is equal and names it; it does not claim that "
    "the binaries themselves are equal."
)

EQUAL_INSTEAD = [
    "the checksum of the sources of the target — the archive of the revision",
    "the version of the product",
    "the checks that were run — the gates, the audit of the licences, the tests",
]

ARCHIVE_NOTE = (
    "The archive carries the tree of the revision and nothing else except the "
    "identification of the revision. The compression is written with `gzip -n`, "
    "so the header carries neither the time nor the name of the build: two "
    "builds of the same revision are bit for bit the same."
)


def assign(record, key, value):
    """Put a value into a nested record — `a.b.c` becomes a nested dict."""
    parts = key.split(".")
    node = record
    for part in parts[:-1]:
        node = node.setdefault(part, {})
    node[parts[-1]] = value


def read_facts(path):
    facts = {}
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line:
                continue
            key, _, value = line.partition("\t")
            facts[key] = value
    return facts


def read_rows(path, columns):
    """Read the tab separated rows of a file into a list of dicts."""
    rows = []
    with open(path, encoding="utf-8") as handle:
        for line in handle:
            line = line.rstrip("\n")
            if not line:
                continue
            fields = line.split("\t")
            fields += [""] * (len(columns) - len(fields))
            rows.append(dict(zip(columns, fields[: len(columns)])))
    return rows


def build_manifest(facts, checks, artifacts):
    record = {}
    for key, value in facts.items():
        assign(record, key, value)

    working_tree = record.setdefault("working_tree", {})
    working_tree["uncommitted_entries"] = int(
        working_tree.get("uncommitted_entries", 0) or 0
    )
    record["revision_short"] = record.get("revision", "")[:12]
    record["revision_is_head"] = record.get("revision_is_head", "") == "true"

    record["schema"] = SCHEMA
    record["product"] = {
        "name": "Vocument",
        "version": facts.get("version", ""),
        "licence": "AGPL-3.0-or-later",
    }
    record["publication"] = {
        "state": "internal-release-candidate",
        "published": False,
        "legal_clearance_e5": "missing",
        "blockers": ["B-1"],
        "notice": PUBLICATION_NOTICE,
    }
    record["artifacts"] = artifacts
    record["checks"] = checks

    archive_first = facts.get("source_archive.sha256", "not-available")
    archive_second = facts.get("source_archive.sha256_second", "not-available")
    library_state = facts.get("library.state", "NOT RUN")
    library_digest = facts.get("library.sha256", "not-available")

    record["reproducibility"] = {
        "source_archive": {
            "bit_identical": archive_first != "not-available"
            and archive_first == archive_second,
            "builds": 2,
            "sha256_first": archive_first,
            "sha256_second": archive_second,
            "note": ARCHIVE_NOTE,
        },
        "applications": {
            "bit_identical": "not-claimed",
            "reason": APPLICATION_REASON,
            "equal_instead": EQUAL_INSTEAD,
            "targets": [
                {
                    "name": "The Rust library (libvocument_ffi.so)",
                    "state": library_state,
                    "sha256": library_digest if library_digest != "not-available" else None,
                    "source_digest": facts.get("source_digest.rust", ""),
                },
                {
                    "name": "The macOS application bundle (Vocument.app)",
                    "state": "NOT RUN",
                    "sha256": None,
                    "reason": (
                        "No macOS host and therefore no macOS SDK in this "
                        "environment; the bundle also needs an Apple identity "
                        "and a notarisation (B-2)."
                    ),
                    "source_digest": facts.get("source_digest.macos", ""),
                },
                {
                    "name": "The Android package (APK/AAB)",
                    "state": "NOT RUN",
                    "sha256": None,
                    "reason": (
                        "No JDK and no Android SDK in this environment; the "
                        "signed package also needs the release key (B-3)."
                    ),
                    "source_digest": facts.get("source_digest.android", ""),
                },
            ],
        },
    }
    return record


def write_markdown(manifest, path):
    lines = []
    lines.append("# The release candidate of Vocument")
    lines.append("")
    lines.append("> **This candidate is NOT published.**")
    lines.append(">")
    lines.append("> " + manifest["publication"]["notice"])
    lines.append("")
    lines.append("| What | Value |")
    lines.append("|---|---|")
    lines.append(f"| **The state** | **{manifest.get('state', 'PASS')}** |")
    lines.append(f"| The product | Vocument {manifest['product']['version']} |")
    lines.append(f"| The revision | `{manifest.get('revision', '')}` |")
    lines.append(f"| The branch | `{manifest.get('branch', '')}` |")
    lines.append(
        f"| The revision is the HEAD of the checkout | "
        f"{'yes' if manifest.get('revision_is_head') else 'no'} |"
    )
    working_tree = manifest.get("working_tree", {})
    lines.append(
        f"| The state of the working tree | {working_tree.get('state', '')} "
        f"({working_tree.get('uncommitted_entries', 0)} uncommitted entries) |"
    )
    lines.append(f"| The date of the run | {manifest.get('generated_at', '')} |")
    lines.append("| The licence | AGPL-3.0-or-later |")
    lines.append("| The legal clearance (E5) | **missing** — the candidate is internal |")
    lines.append("")
    lines.append(working_tree.get("note", ""))
    lines.append("")
    lines.append("## The tools of the run")
    lines.append("")
    lines.append("| Tool | Version |")
    lines.append("|---|---|")
    for label, key in [
        ("rustc", "rustc"),
        ("cargo", "cargo"),
        ("rustfmt", "rustfmt"),
        ("clippy", "clippy"),
        ("UniFFI", "uniffi"),
        ("MuPDF", "mupdf"),
        ("Tesseract", "tesseract"),
        ("Swift", "swift"),
        ("Java", "java"),
        ("The Android SDK", "android_sdk"),
        ("The host", "host"),
    ]:
        lines.append(f"| {label} | `{manifest.get('tools', {}).get(key, '')}` |")
    lines.append("")
    lines.append("## The artefacts of the candidate")
    lines.append("")
    lines.append("| Artefact | The file | SHA-256 | Bytes | Reproducibility |")
    lines.append("|---|---|---|---|---|")
    for artifact in manifest.get("artifacts", []):
        lines.append(
            f"| {artifact['name']} | `{artifact['path']}` | `{artifact['sha256']}` | "
            f"{artifact['bytes']} | {artifact['reproducibility']} |"
        )
    lines.append("")
    lines.append("## The checks of the candidate")
    lines.append("")
    lines.append("| Check | Command | Result | The reason / the evidence |")
    lines.append("|---|---|---|---|")
    for check in manifest.get("checks", []):
        detail = check.get("reason") or check.get("evidence") or ""
        lines.append(
            f"| {check['name']} | `{check['command']}` | **{check['result']}** | {detail} |"
        )
    lines.append("")
    lines.append("## What is reproducible — and what is not")
    lines.append("")
    lines.append("### The source archive: bit for bit the same")
    lines.append("")
    lines.append(
        "The archive of the source is built **twice** in this run, and the two "
        "builds are compared byte by byte."
    )
    lines.append("")
    archive = manifest["reproducibility"]["source_archive"]
    lines.append("| What | Value |")
    lines.append("|---|---|")
    lines.append(f"| The first build | `{archive['sha256_first']}` |")
    lines.append(f"| The second build | `{archive['sha256_second']}` |")
    lines.append(
        f"| Bit for bit the same | {'**yes**' if archive['bit_identical'] else '**no**'} |"
    )
    lines.append("")
    lines.append(archive["note"])
    lines.append("")
    lines.append("### The applications: not claimed to be bit-identical")
    lines.append("")
    lines.append(manifest["reproducibility"]["applications"]["reason"])
    lines.append("")
    lines.append("What is equal instead:")
    lines.append("")
    for item in manifest["reproducibility"]["applications"]["equal_instead"]:
        lines.append(f"* {item}")
    lines.append("")
    lines.append(
        "| Target | The state | The SHA-256 of the binary | The digest of the sources |"
    )
    lines.append("|---|---|---|---|")
    for target in manifest["reproducibility"]["applications"]["targets"]:
        digest = f"`{target['sha256']}`" if target["sha256"] else "—"
        lines.append(
            f"| {target['name']} | **{target['state']}** | {digest} | "
            f"`{target['source_digest']}` |"
        )
    lines.append("")
    lines.append("## The publication")
    lines.append("")
    lines.append(f"* **Published:** no — {manifest['publication']['notice']}")
    lines.append(
        "* **The legal clearance (E5):** missing — the review is the work of the "
        "project owner (B-1)."
    )
    lines.append(
        "* **The channels of the distribution:** none was used. The candidate holds "
        "the source, the record and the licences — no package of a store and no "
        "signed bundle."
    )
    lines.append("")

    with open(path, "w", encoding="utf-8") as handle:
        handle.write("\n".join(lines))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--facts", required=True)
    parser.add_argument("--checks", required=True)
    parser.add_argument("--artifacts", required=True)
    parser.add_argument("--output", required=True)
    arguments = parser.parse_args()

    facts = read_facts(arguments.facts)
    checks = read_rows(
        arguments.checks, ["name", "command", "result", "reason", "evidence"]
    )
    artifacts = read_rows(
        arguments.artifacts,
        ["name", "path", "sha256", "bytes", "kind", "reproducibility", "evidence"],
    )
    for artifact in artifacts:
        artifact["bytes"] = int(artifact["bytes"]) if artifact["bytes"].isdigit() else 0

    manifest = build_manifest(facts, checks, artifacts)
    states = [check["result"] for check in manifest["checks"]]
    manifest["state"] = "FAIL" if "FAIL" in states else "PASS"

    os.makedirs(arguments.output, exist_ok=True)
    json_path = os.path.join(arguments.output, "manifest.json")
    markdown_path = os.path.join(arguments.output, "MANIFEST.md")
    with open(json_path, "w", encoding="utf-8") as handle:
        json.dump(manifest, handle, indent=2, ensure_ascii=False)
        handle.write("\n")
    write_markdown(manifest, markdown_path)


if __name__ == "__main__":
    main()
