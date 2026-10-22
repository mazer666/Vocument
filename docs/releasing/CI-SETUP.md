# The continuous integration — what a runner needs and what the state is

**Date:** 2026-10-22 · **Branch:** `main` · **Owner:** the project owner ·
**Related:** [`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md) (B‑9),
[`RELEASE-PROCESS.md`](RELEASE-PROCESS.md), [`local-ci.sh`](../../scripts/local-ci.sh),
[`github-actions-platform-builds.md`](../audit/github-actions-platform-builds.md)

The repository runs its continuous integration on the **GitHub-hosted runners**
of GitHub. Three workflows do it, one per platform:

| The workflow | The job | The runner | The subject |
|---|---|---|---|
| [`ci.yml`](../../.github/workflows/ci.yml) | `linux-core` | `ubuntu-latest` | the whole gate of the project (`scripts/local-ci.sh`) |
| [`ci-macos.yml`](../../.github/workflows/ci-macos.yml) | `macos-swift` | `macos-15` | the Swift build, the tests of the kit, the handshake |
| [`ci-android.yml`](../../.github/workflows/ci-android.yml) | `android` | `ubuntu-latest` | the native libraries, the packages, the unit tests |

None of them uses a secret, none of them signs anything, and none of them
uploads or publishes anything: the legal clearance (E5) is open, and only a
person can give it.

---

## 1. The state, measured

### 1.1 Before the change (2026-10-22, read from the API of the repository)

| What | The measurement |
|---|---|
| The runs of the workflow since 2026-10-17 | **47** |
| Of them: **cancelled** | **38** |
| Of them: **waiting** (`queued`) | **9** |
| Of them: **run** (green or red) | **0** |
| The runners registered for the repository | **0** |

The CI had never run once. The workflow asked for `runs-on: self-hosted`, and
the repository has no runner: the runs either waited forever or were
cancelled by hand. The gate of the project was only ever run by hand, in the
environment of the team — a release asked for a green gate, and it got one,
but never a *second, independent* one.

### 1.2 The decision, and what it changed

The decision of the project owner (2026-10-22):

> "Use GitHub Actions with the required native SDKs where possible."

The workflow of 2026-10-17 was replaced. The runner of the project is not
needed any more: the hosted runners of GitHub carry the SDKs, and the
workflows install the native parts themselves — the **host MuPDF 1.28.5** is
built from the pinned source
([`prepare-host-mupdf.sh`](../../scripts/prepare-host-mupdf.sh)), the
**Android SDK**, the **NDK r27c** and the **JDK 17** are installed by the
job of Android, and the **Swift compiler** is part of the macOS image.

The details of the three jobs — what they prove, what stays `NOT RUN`, the
errors that were found while the commands were executed, and the time they
need — are in
[`docs/audit/github-actions-platform-builds.md`](../audit/github-actions-platform-builds.md).

---

## 2. What a runner needs — and what the workflows do instead

The runner of the project would need the following:

| What the runner needs | Why | What the workflows do instead |
|---|---|---|
| **Linux or macOS** | the gate is a shell script | the hosted runners are Linux and macOS |
| **Rust** (cargo, rustc) | the workspace is built and tested | the toolchain is installed explicitly (`stable`, with `rustfmt` and `clippy`) |
| **The host MuPDF** | the engine is built against it and the tests link against it | the pinned version 1.28.5 is built from the source by the job itself; `MUPDF_DIR` and `LD_LIBRARY_PATH` (macOS: `DYLD_LIBRARY_PATH`) point at the installation |
| **Tesseract and Leptonica** | the OCR engine is linked against them | `apt-get install libtesseract-dev libleptonica-dev` (Linux), `brew install tesseract` (macOS); `TESSERACT_DIR` names the installation |
| **Python 3** | the gates, the licence audit, the proof of the Swift sources | the image of the runner has it |
| **Node** (with npm) | the proof of the Swift sources parses the sources with the grammar of Swift | the image of the runner has it; the check installs `tree-sitter` and `tree-sitter-swift` into `~/.cache/vocument-swift-check` — it needs the network once |
| **A C toolchain** | the native part of the PDF engine | `build-essential` |
| **The Android SDK, the NDK, a JDK 17** | the packages of Android | the job of Android installs them (the command line tools 12.0 with its SHA-256, `platform-tools`, `platforms;android-35`, `build-tools;35.0.0`, `ndk;27.2.12479018`, OpenJDK 17) |
| *Optional:* **QEMU** | the proof of the engine on a real ARM64 target | the job of Android states `NOT RUN` with the reason; the gate of the Linux job does the same |
| *Optional:* **a device or an emulator** | the proof of the OCR on a device | the job of Android states `NOT RUN` with the reason |

---

## 3. The queue of the old runs

Nine runs of the old workflow are waiting for a runner that does not exist.
They should be cancelled — they are runs of revisions that have long moved
on:

```bash
gh run list --workflow ci.yml --status queued
gh run cancel <id>
```

(Without the `gh` CLI, the page of the workflow offers the same thing: the
three dots next to a run → *Cancel workflow*.)

---

## 4. What the gate proves — and what it does not

The gate of the repository (`scripts/local-ci.sh`) runs:

1. the validation of the repository (the rules `CODE-001` and their kin);
2. the gate of the distribution (the licence, the notices, the offer of the
   source, the local-first rules);
3. the audit of the licences of every component;
4. the structural check of the Swift sources;
5. the static proof of the Swift sources (the grammar of Swift, the names
   against the generated bindings, the self-test of the check);
6. `cargo fmt`, `cargo clippy -D warnings` and the tests of the workspace;
7. the proofs of the redaction, of the inspections and of the workflow;
8. the generation of the bindings and their comparison with the ones of the
   repository;
9. the native libraries of Android (the closure, the SONAME, the 16 KiB
   pages) and the proofs of Android, as far as the machine allows;
10. the **build record** of the run (`target/build-record/`).

Steps that the machine cannot do say `NOT RUN` **out loud** and do not fail
the gate — the ARM64 proof without QEMU, the Android runtime without the
SDK, the Swift application without the macOS SDK. The list of what was not
run is in the build record of every run.

The job of macOS adds the proof that the gate cannot make: the **application**
of the project, compiled against the real macOS SDK, with the tests of the
kit and the handshake from Swift through UniFFI into the Rust core and
MuPDF.

**What the CI does not prove:** the behaviour of the application on a device,
the signature and the notarisation, the publication, and the legal clearance
(E5). Those are the entries of the blocker list, with their owner.

---

## 5. The check of a machine

Before a machine is used for a release by hand, it can be checked with the
gate itself — that is the whole point of the script:

```bash
git clone https://github.com/mazer666/Vocument.git
cd Vocument
./scripts/prepare-host-mupdf.sh          # the host MuPDF 1.28.5
export MUPDF_DIR="$PWD/target/mupdf-host"
export LD_LIBRARY_PATH="$MUPDF_DIR/lib"  # macOS: DYLD_LIBRARY_PATH
export TESSERACT_DIR=/usr                # macOS: the prefix of Homebrew
export TESSDATA_PREFIX="$PWD/apps/android/app/src/main/assets/tessdata"
bash scripts/local-ci.sh
```

The last line of the output must be `==> Local CI completed successfully.`,
and `target/build-record/build-record.md` must name the revision, the
tools, the counts of the tests and the gates.
