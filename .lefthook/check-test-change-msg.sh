#!/usr/bin/env bash
set -euo pipefail

# When a commit's diff touches BOTH existing test files (tests/spec/**) AND
# production code (compiler/**/*.rs), require the commit body to contain a
# "Test-Change: <justification>" line. Hook enforces presence only, not
# quality — review pipelines judge quality.
#
# Project policy: tests pin spec-conformance surface; weakening tests to
# make a hook pass is BANNED. The Test-Change line forces the author to
# write down why the test surface changed, citing a spec clause / approved
# proposal / concrete plan item.

MSG_FILE="$1"
MSG=$(cat "$MSG_FILE")

# Allow merge / revert commits — same exemption as conventional-commit hook.
if [[ "$MSG" =~ ^Merge\  ]]; then
    exit 0
fi
if [[ "$MSG" =~ ^Revert\  ]]; then
    exit 0
fi

# Staged diff file list. At commit-msg time the index already holds what's
# about to be committed; git diff --cached --name-only is authoritative.
STAGED_FILES=$(git diff --cached --name-only 2>/dev/null || echo "")

if [[ -z "$STAGED_FILES" ]]; then
    exit 0
fi

# Touches existing test scope?
TOUCHES_TESTS=$(echo "$STAGED_FILES" | grep -E '^tests/spec/' || true)

# Touches production Rust code? (compiler/**/*.rs)
TOUCHES_PROD=$(echo "$STAGED_FILES" | grep -E '^compiler/.*\.rs$' || true)

# Rule applies only when BOTH are present (diff-pair gate).
if [[ -z "$TOUCHES_TESTS" ]] || [[ -z "$TOUCHES_PROD" ]]; then
    exit 0
fi

# Both present — require a Test-Change: line with non-empty payload in the
# commit body.
if echo "$MSG" | grep -qE '^Test-Change:[[:space:]]*[^[:space:]]'; then
    exit 0
fi

cat >&2 <<'EOF'

ERROR: This commit modifies BOTH existing test files AND production code,
       but the commit body lacks a Test-Change: justification line.

When a commit simultaneously touches tests/spec/** AND compiler/**/*.rs,
the commit body must declare WHY the test surface changed.

Add a line to the commit body:

    Test-Change: <one-line justification — what surface was removed
                  and a citation to a spec clause / approved proposal /
                  concrete plan item that authorizes the change>

Examples:
    Test-Change: removed turbofish from take_n test per approved
                 method-generics-grammar-alignment-proposal.md
    Test-Change: added #compile_fail per §03 negative-pin coverage
                 requirement

The hook accepts ANY non-empty justification. Review pipelines judge
quality — weak fillers ("updated tests", "simplified", "tracked
elsewhere", "cleanup") will be flagged downstream.

If the test edit weakens a positive test (removes assertions, narrows
return types, replaces a return-value oracle with a panic, adds #skip
without a tracked anchor): DO NOT add a Test-Change: line. Revert the
test edit, fix the production code, then commit. Tests pin spec
conformance; weakening them to make a hook pass is banned by project
testing discipline.
EOF

exit 1
