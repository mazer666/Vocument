#!/usr/bin/env bash
# The gate of the distribution: the licence, the notices, the offer of the
# source, the local-first rules and the privacy policy.
#
# The decision of the project owner (E1-E5) asks for the AGPL-3.0 to be
# carried through the product: the licence text, the notices, the
# identification of the source in the binary and the offer of the source
# with every artefact that is handed out.
#
# The gate is a script of its own so that it stays readable: `validate-repo.sh`
# checks the structure of the repository, this one checks what a released
# binary has to carry.
#
# Usage: scripts/validate-distribution.sh
# Exit code 0: everything a released binary needs is in place.

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

# --- The gate of the distribution: the licence, the notices, the source ---
# The decision of the project owner (E1-E5) asks for the AGPL-3.0 to be
# carried through the product: the licence text, the notices, the
# identification of the source in the binary and the offer of the source
# with every artefact that is handed out.

for required_file in \
    LICENSE \
    LICENSES/AGPL-3.0-or-later.txt \
    REUSE.toml \
    THIRD_PARTY.md \
    PRIVACY.md \
    docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md \
    docs/releasing/BUILD-FROM-SOURCE.md \
    docs/releasing/RELEASE-PROCESS.md \
    apps/macos/Resources/Info.plist \
    apps/macos/Resources/VocumentMac.entitlements \
    scripts/audit-licenses.sh \
    scripts/generate-third-party-notices.sh \
    scripts/release-macos.sh \
    scripts/release-android.sh \
    scripts/release-source-archive.sh \
    docs/audit/store-requirements.md; do
  if [[ ! -f "$required_file" ]]; then
    echo "ERROR (DIST Gate): the file '$required_file' is missing!" >&2
    exit 1
  fi
done

# The decision itself has to be written down (E1-E5), with the answers.
for decision in "E1" "E2" "E3" "E4" "E5"; do
  if ! grep -q "\\*\\*$decision\\*\\*" docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md; then
    echo "ERROR (DIST Gate): the decision $decision is not written down in the licensing decision!" >&2
    exit 1
  fi
done
if ! grep -qi "agent cannot give a legal clearance" docs/decisions/LICENSING-AND-DISTRIBUTION-DECISION.md; then
  echo "ERROR (DIST Gate): the decision has to state that an agent cannot give the legal clearance (E5)!" >&2
  exit 1
fi

# The licence of every component has to be known, and the inventory in the
# repository has to be the one the metadata of the components produces.
if ! grep -q "BEGIN OF THE GENERATED LICENCE INVENTORY" THIRD_PARTY.md; then
  echo "ERROR (DIST Gate): the inventory of the licences is missing in THIRD_PARTY.md!" >&2
  exit 1
fi
if ! bash scripts/audit-licenses.sh >/dev/null; then
  echo "ERROR (DIST Gate): the licence audit did not pass — a component carries an unknown licence!" >&2
  exit 1
fi

# The binary has to name its source (AGPL-3.0 section 6 and 13).
if ! grep -q "source_revision" core/vocument-core/src/build_info.rs; then
  echo "ERROR (DIST Gate): the core has to carry the revision of the source!" >&2
  exit 1
fi
if ! grep -q "publication_blocker" core/vocument-core/src/build_info.rs; then
  echo "ERROR (DIST Gate): the core has to refuse a build that cannot be tied to a revision!" >&2
  exit 1
fi
if ! grep -q "document_core_build_info" platform/vocument-ffi/src/build_info_dto.rs; then
  echo "ERROR (DIST Gate): the identification of the build has to cross the FFI boundary!" >&2
  exit 1
fi
if ! grep -q "buildInfo" apps/macos/Sources/VocumentMacKit/DocumentCoreClient.swift; then
  echo "ERROR (DIST Gate): the macOS client has to reach the identification of the build!" >&2
  exit 1
fi
if ! grep -q "buildInfo" apps/android/app/src/main/java/org/vocument/app/DocumentCoreBridge.kt; then
  echo "ERROR (DIST Gate): the Android bridge has to reach the identification of the build!" >&2
  exit 1
fi
if ! grep -q "the tree carries uncommitted changes" scripts/release-macos.sh; then
  echo "ERROR (DIST Gate): the release of macOS has to refuse a build from a dirty tree!" >&2
  exit 1
fi
if ! grep -q "the tree carries uncommitted changes" scripts/release-android.sh; then
  echo "ERROR (DIST Gate): the release of Android has to refuse a build from a dirty tree!" >&2
  exit 1
fi
if ! grep -q "SOURCE.txt" scripts/release-macos.sh || ! grep -q "SOURCE.txt" scripts/release-android.sh; then
  echo "ERROR (DIST Gate): every artefact has to carry the offer of the source!" >&2
  exit 1
fi

# The product is local-first (E4): no network, no accounts, no telemetry.
if grep -q '<uses-permission android:name="android.permission.INTERNET"' apps/android/app/src/main/AndroidManifest.xml; then
  echo "ERROR (DIST Gate): the Android manifest asks for the network — the product is local-first!" >&2
  exit 1
fi
if grep -q "network.client\|network.server" apps/macos/Resources/VocumentMac.entitlements; then
  echo "ERROR (DIST Gate): the macOS entitlements ask for the network — the product is local-first!" >&2
  exit 1
fi
if ! grep -q "com.apple.security.app-sandbox" apps/macos/Resources/VocumentMac.entitlements; then
  echo "ERROR (DIST Gate): the macOS app has to run in the sandbox — the stores ask for it!" >&2
  exit 1
fi
if ! grep -q "does not collect" PRIVACY.md; then
  echo "ERROR (DIST Gate): the privacy policy has to say what the product does with the data!" >&2
  exit 1
fi

# The conflict between the terms of Apple and the AGPL has to be named as a
# risk — it must not be smoothed over.
if ! grep -q "RISK-041" docs/project/RISKS.md; then
  echo "ERROR (DIST Gate): the conflict between the Apple terms and the AGPL has to be a named risk!" >&2
  exit 1
fi

# The texts of the licences that travel with a binary have to be in the
# repository: a notice whose text has to be fetched from the network at the
# moment of the release is a notice that is missing when the network is down.
for licence_text in \
    MPL-2.0.txt \
    Apache-2.0.txt \
    BSD-2-Clause.txt \
    BSD-3-Clause.txt \
    ISC.txt \
    MIT.txt \
    Zlib.txt \
    OFL-1.1.txt \
    Unicode-3.0.txt \
    FTL.txt \
    IJG.txt; do
  if [[ ! -s "LICENSES/$licence_text" ]]; then
    echo "ERROR (DIST Gate): the licence text 'LICENSES/$licence_text' is missing!" >&2
    exit 1
  fi
done

# The notice of the third-party components has to be there and has to be the
# one that the generator produces right now.
if [[ ! -s LICENSES/THIRD-PARTY-NOTICES.txt ]]; then
  echo "ERROR (DIST Gate): the notice of the third-party components is missing!" >&2
  exit 1
fi
mkdir -p "$REPO_ROOT/target"
if ! bash scripts/generate-third-party-notices.sh --output "$REPO_ROOT/target/third-party-notices.check.txt" > /dev/null 2>&1; then
  echo "ERROR (DIST Gate): the notice of the third-party components could not be assembled!" >&2
  exit 1
fi
if ! diff -q LICENSES/THIRD-PARTY-NOTICES.txt "$REPO_ROOT/target/third-party-notices.check.txt" > /dev/null; then
  echo "ERROR (DIST Gate): the notice of the third-party components is out of date." >&2
  echo "       Run 'scripts/generate-third-party-notices.sh' to bring it back in line." >&2
  exit 1
fi

# Every binary has to carry the notice of the third-party components.
for release_script in scripts/release-macos.sh scripts/release-android.sh; do
  if ! grep -q "THIRD-PARTY-NOTICES.txt" "$release_script"; then
    echo "ERROR (DIST Gate): '$release_script' has to put the notice of the third-party components into the artefact!" >&2
    exit 1
  fi
done

# The configuration of cargo-deny has to be loadable — and whether it can be
# loaded at all depends on the **version** of the tool: the spellings of the
# GNU licences are not the same in every version (see the head of `deny.toml`).
# The version of the tool is therefore part of the configuration of the
# project, and a machine that carries another version is told that the check
# was **NOT RUN** — a check that was not carried out is never reported as a
# passed one.
CARGO_DENY_REQUIRED_VERSION="0.20.2"
CARGO_DENY_BIN=""
if command -v cargo-deny > /dev/null 2>&1; then
  CARGO_DENY_BIN="$(command -v cargo-deny)"
elif [[ -x "${HOME}/.cargo/bin/cargo-deny" ]]; then
  CARGO_DENY_BIN="${HOME}/.cargo/bin/cargo-deny"
fi
if [[ -n "$CARGO_DENY_BIN" ]]; then
  # cargo-deny calls cargo itself: it has to find it.
  CARGO_HOME_BIN="$(dirname "$CARGO_DENY_BIN")"
  CARGO_DENY_VERSION="$("$CARGO_DENY_BIN" --version 2>/dev/null | grep -oE '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1)"
  if [[ "$CARGO_DENY_VERSION" != "$CARGO_DENY_REQUIRED_VERSION" ]]; then
    echo "NOT RUN (DIST Gate): cargo-deny ${CARGO_DENY_VERSION:-of an unknown version} is installed, the project needs ${CARGO_DENY_REQUIRED_VERSION} — the licence configuration of deny.toml was not checked here." >&2
    echo "         The spellings of the GNU licences differ between the versions of the tool; see the head of deny.toml." >&2
  elif ! PATH="$CARGO_HOME_BIN:$PATH" "$CARGO_DENY_BIN" check licenses > /dev/null 2>&1; then
    echo "ERROR (DIST Gate): cargo-deny ${CARGO_DENY_REQUIRED_VERSION} does not accept the licence configuration of deny.toml!" >&2
    exit 1
  else
    echo "✓ (DIST Gate): the licence configuration of deny.toml was checked with cargo-deny ${CARGO_DENY_VERSION}."
  fi
else
  echo "NOTE (DIST Gate): cargo-deny is not installed — the licence configuration was not checked here." >&2
fi

echo "✓ DIST Gate verified: the licence, the notices, the source offer and the local-first rules are in place."
