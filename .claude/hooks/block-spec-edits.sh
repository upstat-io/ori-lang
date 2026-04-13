#!/usr/bin/env bash
# PreToolUse hook: block direct edits to spec/grammar files.
#
# Spec and grammar files under docs/ori_lang/v2026/spec/ are protected
# by the proposal gate. They MUST NOT be modified without an approved
# proposal (via /create-draft-proposal -> /review-draft-proposal).
#
# This hook fires on Edit and Write tool calls. If the target file_path
# is under the spec directory, the tool call is DENIED unless a bypass
# is active.
#
# Bypass methods (either works):
#   1. Environment variable: ORI_SPEC_PROPOSAL=<proposal-filename>
#      Works for CLI usage: ORI_SPEC_PROPOSAL=foo.md ori ...
#   2. Marker file: .claude/.spec-proposal-active
#      Contains a single line: the proposal filename.
#      Works for Claude Code sessions where env vars don't persist
#      across tool calls. Created by /review-draft-proposal during
#      the approval workflow, cleaned up by /commit-push or manually.
#
# Both methods verify the proposal exists in approved/ before allowing.
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
    APPROVED_DIR="docs/ori_lang/proposals/approved"
    PROPOSAL=""

    # Method 1: Check environment variable
    if [[ -n "${ORI_SPEC_PROPOSAL:-}" ]]; then
        PROPOSAL="$ORI_SPEC_PROPOSAL"
    fi

    # Method 2: Check marker file (fallback for Claude Code sessions)
    MARKER_FILE=".claude/.spec-proposal-active"
    if [[ -z "$PROPOSAL" ]] && [[ -f "$MARKER_FILE" ]]; then
        PROPOSAL=$(head -1 "$MARKER_FILE" | tr -d '[:space:]')
    fi

    if [[ -n "$PROPOSAL" ]]; then
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
            + 'Set ORI_SPEC_PROPOSAL=<filename> or write the filename to '
            + '.claude/.spec-proposal-active after approval to bypass. '
            + 'See .claude/rules/spec.md for details.'
        )
    }
}))
" "$BASENAME"
    exit 0
fi

# Not a spec file — allow
exit 0
