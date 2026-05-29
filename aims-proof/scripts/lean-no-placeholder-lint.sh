#!/usr/bin/env bash
#
# lean-no-placeholder-lint.sh — Lean proof no-placeholder gate.
#
# Greps every .lean under aims-proof/ for banned proof-escape hatches that let
# a non-proof masquerade as a discharged theorem:
#   - \bsorry\b
#   - \badmit\b
#   - ": True := by trivial"   (vacuous-True theorem body)
#   - ":= by trivial"          (trivial-discharged body)
#   - standalone "axiom " on a theorem (asserted, not proven)
#
# Exit codes:
#   0 — corpus clean (emits lean_no_placeholder_clean).
#   1 — at least one banned hit (emits lean_placeholder_found), each cited file:line.
#
# Cwd contract: anchors to aims-proof/ (this script's parent dir's parent),
# then scans every .lean under aims-proof/.

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AIMS_PROOF_DIR="$(dirname "$SCRIPT_DIR")"

# --- Banned-pattern set (extended regex). ---
declare -a PATTERNS=(
    '\bsorry\b'
    '\badmit\b'
    ': True := by trivial'
    ':= by trivial'
    '^[[:space:]]*axiom '
)

declare -a hits=()

while IFS= read -r lean_file; do
    for pat in "${PATTERNS[@]}"; do
        while IFS= read -r match; do
            [[ -n "$match" ]] || continue
            rel="${lean_file#"${AIMS_PROOF_DIR}/"}"
            lineno="${match%%:*}"
            text="${match#*:}"
            hits+=("${rel}:${lineno}: ${text}")
        done < <(grep -nE "$pat" "$lean_file" 2>/dev/null || true)
    done
done < <(find "$AIMS_PROOF_DIR" -name '*.lean' | sort)

if [[ ${#hits[@]} -gt 0 ]]; then
    echo "lean-no-placeholder-lint: lean_placeholder_found — ${#hits[@]} banned hit(s):" >&2
    for h in "${hits[@]}"; do
        echo "  - ${h}" >&2
    done
    exit 1
fi

echo "lean-no-placeholder-lint: lean_no_placeholder_clean — no banned escape hatches."
exit 0
