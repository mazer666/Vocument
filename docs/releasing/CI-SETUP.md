# The continuous integration — what a runner needs and what the state is

**Date:** 2026-10-22 · **Branch:** `main` · **Owner:** the project owner ·
**Related:** [`RELEASE-BLOCKERS.md`](RELEASE-BLOCKERS.md) (B‑9),
[`RELEASE-PROCESS.md`](RELEASE-PROCESS.md), [`local-ci.sh`](../../scripts/local-ci.sh)

The workflow of the repository (`.github/workflows/ci.yml`) runs **one** step:
`./scripts/local-ci.sh` — the gate of the project. There is no second
workflow, no matrix, no secret.

---

## 1. The state, measured

The measurement of 2026-10-22, read from the API of the repository:

| What | The measurement |
|---|---|
| The runs of the workflow since 2026-10-17 | **47** |
| Of them: **cancelled** | **38** |
| Of them: **waiting** (`queued`) | **9** |
| Of them: **run** (green or red) | **0** |
| The runners registered for the repository | **0** |

**The CI has never run once.** The workflow asks for `runs-on: self-hosted`,
and the repository has no runner. The runs either wait forever or are
cancelled by hand.

That is not a defect of the workflow: the workflow does exactly what it
should. It is a **missing machine**.

**The consequence for the release:** the gate of the project has always been
run by hand, in one environment (the sandbox of the team). A release asks
for a green gate — and it gets one, but never a *second, independent* one.
The statement "the CI is green" must not be made; the honest statement is
"the local gate is green, the CI has not run".

---

## 2. The two ways out — both are the decision of the owner

### 2.1 A self-hosted runner (the workflow stays as it is)

A machine of the owner with the toolchain of the project, registered for the
repository. The workflow does not change.

| What the runner needs | Why | How it is checked |
|---|---|---|
| **Linux or macOS** | the gate is a shell script | — |
| **Rust** (cargo, rustc) | the workspace is built and tested | `cargo --version` |
| **The host MuPDF** | the PDF engine is built against it and the tests link against it | `MUPDF_DIR` must point at an installation with `lib/libmupdf.so`; `LD_LIBRARY_PATH=$MUPDF_DIR/lib` |
| **Python 3** | the gates, the licence audit, the Swift proof | `python3 --version` |
| **Node** (with npm) | the Swift proof parses the sources with the grammar of Swift | the check installs `tree-sitter` and `tree-sitter-swift` into `~/.cache/vocument-swift-check` — it needs the network once |
| **A C toolchain** | the native part of the PDF engine | `cc --version` |
| **The network** | the packages of npm, the crates of Cargo | — |
| *Optional:* the **Android SDK** and a **JDK** | the Android proofs | without them the gate says `NOT RUN` and stays green — the same convention as for the QEMU proof |

The registration is the part that needs the owner: the page
*Settings → Actions → Runners → New self-hosted runner* asks for a token,
and the machine has to run the runner package with it. The token belongs to
the owner and must not be written into the repository.

### 2.2 GitHub-hosted runners (the workflow has to change)

`runs-on: ubuntu-latest` needs no machine of the owner. The price: the
runner has no MuPDF, no Android SDK and no Swift toolchain — the gate would
run there with those proofs in `NOT RUN` (which the gate says out loud), and
the **real** proofs would stay in the local environment. The workflow file
has to be changed for it, and **the team does not change the workflow
files** — that decision and that change belong to the owner.

### 2.3 The third way — no runner at all

The gate is a script. It runs anywhere the toolchain is:

```bash
bash scripts/local-ci.sh
```

A second machine of the owner — a Mac, a Linux box — running the gate by
hand is the same proof as a runner: the same script, the same steps, the
same record (`target/build-record/`). The difference is that it is not
automatic; the difference is also that it needs no identity and no account.

---

## 3. The queue

Nine runs are waiting for a runner that does not exist. If a runner appears,
they would all start at once — on the revisions of the last two days, long
after the code moved on. They should be cancelled **before** a runner is
registered:

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

**What the gate does not prove:** the behaviour of the application on a
device, the compilation of the macOS application, the signing, and the legal
clearance (E5). Those are the entries of the blocker list, with their owner.

---

## 5. The check of the runner

Before a runner is registered, the machine can be checked with the gate
itself — that is the whole point of the script:

```bash
git clone https://github.com/mazer666/Vocument.git
cd Vocument
export MUPDF_DIR="$PWD/target/mupdf-host"     # or an installation of MuPDF
export LD_LIBRARY_PATH="$MUPDF_DIR/lib"
bash scripts/local-ci.sh
```

The last line of the output must be `==> Local CI completed successfully.`,
and `target/build-record/build-record.md` must name the revision, the
tools, the counts of the tests and the gates.
