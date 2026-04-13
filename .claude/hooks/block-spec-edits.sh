#!/usr/bin/env bash
# PreToolUse hook: block direct edits to spec/grammar files.
#
# Spec and grammar files under docs/ori_lang/v2026/spec/ are protected
# by the proposal gate. They MUST NOT be modified without an approved
# proposal (via /create-draft-proposal -> /review-draft-proposal).
#
# This hook fires on Edit and Write tool calls. If the target file_path
# is under the spec directory, the tool call is DENIED unless the
# ORI_SPEC_PROPOSAL environment variable is set to an approved proposal
# filename.
#
# Bypass: set ORI_SPEC_PROPOSAL=<proposal-filename> when using /sync-spec
# or /sync-grammar after a proposal has been approved.
#
# Reading spec files is always allowed (this hook only fires on Edit/Write).

set -euo pipefail

INPUT=$(cat)

# Extract the file_path from the tool input
FILE_PATH=$(printf '%s' "$INPUT" | python3 -c "
import sys, json
data = json.load(sys.stdin)
ti = data.get('tool_input', {})
print(ti.get('file_path', ''))
")

# Check if the file is under the protected spec directory
SPEC_PATTERN="docs/ori_lang/v2026/spec/"

if [[ "$FILE_PATH" == *"$SPEC_PATTERN"* ]]; then
    # Check for approved proposal bypass
    if [[ -n "${ORI_SPEC_PROPOSAL:-}" ]]; then
        APPROVED_DIR="docs/ori_lang/proposals/approved"
        PROPOSAL="$ORI_SPEC_PROPOSAL"

        # Verify the proposal actually exists in approved/
        if [[ -f "$APPROVED_DIR/$PROPOSAL" ]] || \
           [[ -f "$APPROVED_DIR/${PROPOSAL}.md" ]] || \
           [[ -n "$(find "$APPROVED_DIR" -name "$PROPOSAL" -o -name "${PROPOSAL}.md" 2>/dev/null | head -1)" ]]; then
            # Approved proposal exists — allow the edit
            exit 0
        fi

        # Proposal referenced but not found in approved/
        BASENAME=$(basename "$FILE_PATH")
        python3 -c "
import json, sys
print(json.dumps({
    'hookSpecificOutput': {
        'hookEventName': 'PreToolUse',
        'permissionDecision': 'deny',
        'permissionDecisionReason': (
            'SPEC/GRAMMAR PROPOSAL GATE: ORI_SPEC_PROPOSAL='
            + sys.argv[1]
            + ' but no matching file found in docs/ori_lang/proposals/approved/. '
            + 'Ensure the proposal has been approved via /review-draft-proposal.'
        )
    }
}))
" "$PROPOSAL"
        exit 0
    fi

    # No bypass — block the edit
    BASENAME=$(basename "$FILE_PATH")
    python3 -c "
import json, sys
print(json.dumps({
    'hookSpecificOutput': {
        'hookEventName': 'PreToolUse',
        'permissionDecision': 'deny',
        'permissionDecisionReason': (
            'SPEC/GRAMMAR PROPOSAL GATE: Cannot modify '
            + sys.argv[1]
            + ' without an approved proposal. '
            + 'Run /create-draft-proposal first, then /review-draft-proposal. '
            + 'Only after approval may spec/grammar files be edited. '
            + 'Set ORI_SPEC_PROPOSAL=<filename> after approval to bypass. '
            + 'See .claude/rules/spec.md for details.'
        )
    }
}))
" "$BASENAME"
    exit 0
fi

# Not a spec file — allow
exit 0
