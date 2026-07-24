#!/usr/bin/env bash
#
# lean-statement-parity.sh — statement-parity prelude for dual-discharge.sh.
#
# For every .proof under aims-proof/proofs/, resolves its mapped Lean theorem
# via proof-lean-map.json and asserts the Lean theorem STATEMENT matches the
# .proof obligation. Runs for ALL mapped theorems BEFORE any verdict is
# computed; a weakened-but-building Lean module can never reach verdict
# comparison.
#
# Rejects (exit 1, dual_discharge_divergence + parity-class string):
#   missing_mapping   — a .proof with no proof-lean-map.json row.
#   wrong_theorem_id  — a row's lean_theorem absent from its lean_module.
#   weaker_statement  — resolved Lean decl vacuous-True OR mentions none of the
#                       row's parity_tokens (carrier/operator evidence).
#
# Exit codes:
#   0 — full-corpus parity PASS (emits dual_discharge_agree, prelude scope).
#   1 — at least one parity failure (emits dual_discharge_divergence).
#
# Delegates the JSON/regex parity logic to statement_parity.py (the .sh keeps
# only the cwd anchor + delegation, mirroring check-proofs.sh's python shell-out).

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

python3 "${SCRIPT_DIR}/statement_parity.py" "$@"
