#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

mkdir -p "$HOOKS_DIR"

cat > "$HOOKS_DIR/commit-msg" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

COMMIT_MSG_FILE="$1"
if ! grep -qE '^Signed-off-by: .+ <.+@.+>' "$COMMIT_MSG_FILE"; then
  echo "ERROR: Commit message is missing a DCO Signed-off-by line." >&2
  echo "Use 'git commit -s' to sign off your commit (see CONTRIBUTING.md)." >&2
  exit 1
fi
EOF

cat > "$HOOKS_DIR/pre-push" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

REPO_ROOT="$(git rev-parse --show-toplevel)"
cd "$REPO_ROOT"

echo "==> Running pre-push local CI checks..."
./scripts/local-ci.sh
EOF

chmod +x "$HOOKS_DIR/commit-msg" "$HOOKS_DIR/pre-push"
echo "✓ Installed Git hooks (commit-msg DCO check & pre-push local-ci check)."
