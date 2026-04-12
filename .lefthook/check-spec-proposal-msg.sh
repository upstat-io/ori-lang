#!/usr/bin/env bash
# Commit-msg hook: enforce Proposal: reference when spec/grammar files are staged.
#
# This is the HARD GATE. The pre-commit variant warns early; this one blocks.
# Runs after the commit message is written but before the commit finalizes.

set -euo pipefail

MSG_FILE="$1"
MSG=$(head -20 "$MSG_FILE")  # Read enough lines to find Proposal: in body

# Allow merge and revert commits
if [[ "$MSG" =~ ^Merge\  ]] || [[ "$MSG" =~ ^Revert\  ]]; then
    exit 0
fi

# Check if any staged files are under the spec directory
SPEC_FILES=$(git diff --cached --name-only --diff-filter=ACDMR | grep -E '^docs/ori_lang/v2026/spec/' || true)

if [[ -z "$SPEC_FILES" ]]; then
    # No spec/grammar files staged — nothing to enforce
    exit 0
fi

# Spec files ARE staged — require Proposal: reference
if echo "$MSG" | grep -qiE 'Proposal:\s*\S+'; then
    # Has a Proposal: reference — allow
    exit 0
fi

cat <<EOF

================================================================================
  SPEC/GRAMMAR PROPOSAL GATE — COMMIT BLOCKED
================================================================================

  The following spec/grammar files are staged:

$(echo "$SPEC_FILES" | sed 's/^/    /')

  But the commit message does NOT contain a "Proposal:" reference.

  Add "Proposal: <proposal-filename>" to your commit message body.

  Example commit message:
    docs(spec): update Clause 14 expression semantics

    Proposal: expression-semantics-proposal.md

  NO EXCEPTIONS. See .claude/rules/spec.md for details.
================================================================================

EOF
exit 1
