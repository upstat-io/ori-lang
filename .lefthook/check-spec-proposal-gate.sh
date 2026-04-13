#!/usr/bin/env bash
# Pre-commit hook: WARN (not block) when spec/grammar files are staged.
#
# This is the SOFT GATE — it reminds the developer that a Proposal:
# reference is required in the commit message. The HARD GATE is the
# commit-msg hook (check-spec-proposal-msg.sh) which runs after the
# message is written and actually blocks on missing Proposal: lines.
#
# Why this hook does NOT read .git/COMMIT_EDITMSG:
# Pre-commit runs BEFORE the commit message is written. COMMIT_EDITMSG
# may contain a stale message from a prior commit — possibly from a
# different parallel Claude Code session. Reading it would cause false
# rejections when a parallel session's non-proposal commit left a stale
# message file. The commit-msg hook is the only place where the message
# content is reliable.

set -euo pipefail

# Check if any staged files are under the spec directory
SPEC_FILES=$(git diff --cached --name-only --diff-filter=ACDMR | grep -E '^docs/ori_lang/v2026/spec/' || true)

if [[ -z "$SPEC_FILES" ]]; then
    exit 0
fi

# Spec files are staged — remind, don't block
cat <<EOF

================================================================================
  SPEC/GRAMMAR PROPOSAL GATE — REMINDER
================================================================================

  The following spec/grammar files are staged for commit:

$(echo "$SPEC_FILES" | sed 's/^/    /')

  Your commit message MUST contain a "Proposal: <filename>" reference
  to an approved proposal in docs/ori_lang/proposals/approved/.

  The commit-msg hook will enforce this requirement.
================================================================================

EOF

# Always exit 0 — this is a warning only. The commit-msg hook
# (check-spec-proposal-msg.sh) is the hard enforcement gate.
exit 0
