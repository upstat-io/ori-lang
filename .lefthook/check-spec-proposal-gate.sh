#!/usr/bin/env bash
# Pre-commit hook: block spec/grammar changes that lack an approved proposal.
#
# Protected paths: docs/ori_lang/v2026/spec/**
# This includes grammar.ebnf, operator-rules.md, and all clause files.
#
# Bypass: the commit message (read from .git/COMMIT_EDITMSG) must contain
# "Proposal:" followed by a proposal filename, OR the commit must be a
# merge/revert (handled by the commit-msg hook already).
#
# This is a HARD GATE — no exceptions, not even for typo fixes.
# See .claude/rules/spec.md §Proposal Gate for the full rationale.

set -euo pipefail

# Check if any staged files are under the spec directory
SPEC_FILES=$(git diff --cached --name-only --diff-filter=ACDMR | grep -E '^docs/ori_lang/v2026/spec/' || true)

if [[ -z "$SPEC_FILES" ]]; then
    # No spec/grammar files staged — nothing to check
    exit 0
fi

# Spec files are staged — check commit message for Proposal: reference
MSG_FILE=".git/COMMIT_EDITMSG"

if [[ ! -f "$MSG_FILE" ]]; then
    # No commit message file yet (pre-commit runs before commit-msg).
    # We can't check the message here, so we'll rely on the commit-msg
    # hook variant. But we CAN warn loudly.
    cat <<EOF

================================================================================
  SPEC/GRAMMAR PROPOSAL GATE — WARNING
================================================================================

  The following spec/grammar files are staged for commit:

$(echo "$SPEC_FILES" | sed 's/^/    /')

  These files are protected by the proposal gate. Your commit message
  MUST contain a "Proposal: <filename>" reference to an approved proposal
  in docs/ori_lang/proposals/approved/.

  Workflow:
    1. /create-draft-proposal  — create the proposal
    2. /review-draft-proposal  — get it reviewed and approved
    3. Only THEN modify spec/grammar files

  The commit-msg hook will enforce the Proposal: reference.
================================================================================

EOF
    # Don't block here — let the commit-msg hook do the hard enforcement.
    # This warning ensures the developer sees it early.
    exit 0
fi

# If we DO have a commit message (e.g., amending), check it
MSG=$(cat "$MSG_FILE")

# Allow merge and revert commits
if [[ "$MSG" =~ ^Merge\  ]] || [[ "$MSG" =~ ^Revert\  ]]; then
    exit 0
fi

# Check for Proposal: reference
if ! echo "$MSG" | grep -qiE 'Proposal:\s*\S+'; then
    cat <<EOF

================================================================================
  SPEC/GRAMMAR PROPOSAL GATE — BLOCKED
================================================================================

  The following spec/grammar files are staged:

$(echo "$SPEC_FILES" | sed 's/^/    /')

  But the commit message does NOT contain a "Proposal:" reference.

  Spec and grammar files MUST NOT be modified without an approved proposal.
  Add "Proposal: <proposal-filename>" to your commit message body.

  Example:
    docs(spec): update Clause 14 expression semantics

    Proposal: expression-semantics-proposal.md

  If no approved proposal exists, run:
    1. /create-draft-proposal
    2. /review-draft-proposal
    3. Then commit with the Proposal: reference

  NO EXCEPTIONS — not even for typos, formatting, or "obvious" fixes.
  See .claude/rules/spec.md §Proposal Gate for the full rationale.
================================================================================

EOF
    exit 1
fi

# Proposal reference found — verify the proposal file exists in approved/
PROPOSAL_REF=$(echo "$MSG" | grep -oiE 'Proposal:\s*\S+' | head -1 | sed 's/[Pp]roposal:\s*//')

APPROVED_DIR="docs/ori_lang/proposals/approved"
if [[ -f "$APPROVED_DIR/$PROPOSAL_REF" ]]; then
    exit 0
fi

# Try without extension
if [[ -f "$APPROVED_DIR/${PROPOSAL_REF}.md" ]]; then
    exit 0
fi

# Try finding by basename anywhere in approved/
FOUND=$(find "$APPROVED_DIR" -name "$PROPOSAL_REF" -o -name "${PROPOSAL_REF}.md" 2>/dev/null | head -1 || true)
if [[ -n "$FOUND" ]]; then
    exit 0
fi

cat <<EOF

================================================================================
  SPEC/GRAMMAR PROPOSAL GATE — PROPOSAL NOT FOUND
================================================================================

  Commit references: Proposal: $PROPOSAL_REF
  But no matching file was found in: $APPROVED_DIR/

  Ensure the proposal has been approved and exists in the approved/ directory.
  Run /review-draft-proposal to approve a draft proposal.
================================================================================

EOF
exit 1
