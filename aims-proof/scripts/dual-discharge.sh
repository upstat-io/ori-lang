#!/usr/bin/env bash
#
# dual-discharge.sh — dual-prover gate (.proof checker <-> Lean kernel).
#
# Asserts the Ori `.proof` checker's verdicts agree with Lean's trusted kernel.
# Validates the proof CHECKER, NOT the shipped compiler (a .proof/.lean pair
# proves a calculus rule sound over the model, not that emitted ori_arc obeys
# it — that binding is owned elsewhere).
#
# Sequence:
#   1. statement-parity prelude (lean-statement-parity.sh, via statement_parity.py)
#      runs FIRST over the full corpus. ANY parity failure aborts with exit 1,
#      dual_discharge_divergence + parity class + theorem ID.
#   2. ONLY on full-corpus parity PASS: compute the Ori-checker verdict AND the
#      Lean verdict (lake build of the mapped modules); compare per theorem.
#      AGREE -> exit 0 (dual_discharge_agree). DISAGREE -> exit 1
#      (dual_discharge_divergence) citing the divergent theorem + both verdicts.
#      A Lean-reject + Ori-valid (or vice versa) is a HARD failure, NOT a
#      `::warning`.
#
# Exit codes:
#   0 — full-corpus parity PASS AND every mapped theorem's verdicts agree
#       (emits dual_discharge_agree).
#   1 — a parity failure OR a verdict disagreement (emits
#       dual_discharge_divergence). HARD failure.
#
# Delegates the parity + verdict-comparison logic to dual_discharge.py (the .sh
# keeps only the cwd anchor + delegation, mirroring check-proofs.sh's shell-out).

set -u

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

python3 "${SCRIPT_DIR}/dual_discharge.py" "$@"
