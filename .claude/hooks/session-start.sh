#!/usr/bin/env bash
# SessionStart hook: install LLVM 21 so the full test suite can run.
# Mirrors the CI install step exactly (see .github/workflows/ci.yml).
# Idempotent — skips if LLVM 21 is already present.
# Only runs in remote (cloud) Claude Code sessions.

set -euo pipefail

if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

echo '{"async": true, "asyncTimeout": 300000}'

# Skip if already installed
if [ -f /usr/lib/llvm-21/bin/llvm-config ] && /usr/lib/llvm-21/bin/llvm-config --version | grep -q "^21"; then
  echo "LLVM 21 already installed, skipping."
  echo "LLVM_SYS_211_PREFIX=/usr/lib/llvm-21" >> "$CLAUDE_ENV_FILE"
  exit 0
fi

echo "Installing LLVM 21..."

DEBIAN_FRONTEND=noninteractive

# Add apt.llvm.org repo (same as CI)
curl -fsSL https://apt.llvm.org/llvm-snapshot.gpg.key \
  | gpg --dearmor -o /usr/share/keyrings/llvm-archive-keyring.gpg

CODENAME=$(lsb_release -cs)
echo "deb [signed-by=/usr/share/keyrings/llvm-archive-keyring.gpg] \
http://apt.llvm.org/${CODENAME}/ llvm-toolchain-${CODENAME}-21 main" \
  | tee /etc/apt/sources.list.d/llvm.list

apt-get update -qq
apt-get install -y --no-install-recommends \
  llvm-21 llvm-21-dev libpolly-21-dev clang-21 lld-21

echo "LLVM 21 installed successfully."

# Persist for the rest of the session
echo "LLVM_SYS_211_PREFIX=/usr/lib/llvm-21" >> "$CLAUDE_ENV_FILE"
