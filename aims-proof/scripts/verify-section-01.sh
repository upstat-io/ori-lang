#!/usr/bin/env bash
#
# verify-section-01.sh — sec-01 mathematical foundation orchestrator gate.
#
# Per the mathematical foundation
# Implementation Items 11.12 + 11.14 + FAIL-branch closed exit_reason enum
# at lines 357-361.
#
# Pipeline (3 phases, distinct FAIL-branch routing per phase):
# (a) Per-artifact checker invocation (14 .proof files under
# proofs/01-*/) — any parser-rejection routes through
# calculus_parse_failed (exit 2);
# any type-error on a DEFINITION routes through
# calculus_typecheck_failed (exit 2).
# (b) Theorem-inventory verification on Theorems.proof:
# per-category count must match the inventory table at section
# line 100-110: L=10, CN=8, TF=24, DP=10, IC=9, IA=9, PL=13,
# RL=38, VF=8 (total 129 = 124 active + 5 removal entries).
# Missing entries route through theorem_inventory_incomplete
# (exit 2).
# (c) Composition-inventory verification on Composition.proof:
# CH category must have >= 5 theorems (CH-1 through CH-5
# mandatory; up to CH-10 optional). Missing routes through
# theorem_inventory_incomplete (exit 2).
#
# Exit codes match scripts/plan_corpus/exit_reasons.py
# EXIT_REASON_ROUTING:
# 0 = foundation_authored_clean (PASS)
# 2 = calculus_parse_failed | calculus_typecheck_failed |
# theorem_inventory_incomplete (engine-test-style failures)
#
# Cwd contract per IT 123 (§00 + §01 share the same script anchor
# discipline): script anchors to aims-proof/ via cd "$(dirname "$0")/.."
# at entry; every subsequent path is relative to aims-proof/, NOT the
# repo-root cwd of the caller.

set -e

cd "$(dirname "$0")/.."

mkdir -p test-results

# ---------------------------------------------------------------------------
# Per-artifact verification commands per section IT 11.12 — 14 invocations
# ---------------------------------------------------------------------------
ARTIFACTS=(
  "proofs/01-lattice/Lattice.proof"
  "proofs/01-lattice/Product.proof"
  "proofs/01-lattice/Effect.proof"
  "proofs/01-side-tables/SideTables.proof"
  "proofs/01-contracts/Contract.proof"
  "proofs/01-transfers/Transfer.proof"
  "proofs/01-decisions/Decision.proof"
  "proofs/01-intraprocedural/Intraprocedural.proof"
  "proofs/01-interprocedural/Interprocedural.proof"
  "proofs/01-pipeline/Pipeline.proof"
  "proofs/01-realization/Realization.proof"
  "proofs/01-verification/Verification.proof"
  "proofs/01-theorems/Theorems.proof"
  "proofs/01-theorems/Composition.proof"
)

# Phase (a) — checker invocation on each artifact.
# parse_failure exit_reason routes through calculus_parse_failed (exit 2).
# Any other non-zero exit routes through calculus_typecheck_failed (exit 2).
echo "Phase (a) — checker invocations on 14 artifacts"
failed_artifact=""
fail_reason=""
for artifact in "${ARTIFACTS[@]}"; do
  out=$(./target/release/aims-proof-checker check "$artifact" 2>&1)
  rc=$?
  if [ $rc -ne 0 ]; then
    failed_artifact="$artifact"
    if echo "$out" | grep -q "parse_failure"; then
      fail_reason="calculus_parse_failed"
    else
      fail_reason="calculus_typecheck_failed"
    fi
    echo "FAIL: $artifact (exit $rc)"
    echo "$out" | head -5
    echo "{\"status\": \"fail\", \"exit_reason\": \"$fail_reason\", \"reason\": \"$artifact: exit $rc; see test-results/section-01-${fail_reason}.log\"}" > test-results/section-01-${fail_reason}.log
    exit 2
  fi
  echo " OK: $artifact"
done

# ---------------------------------------------------------------------------
# Phase (b) — theorem-inventory verification on Theorems.proof per section
# inventory table (lines 100-110)
# ---------------------------------------------------------------------------
echo "Phase (b) — theorem-inventory verification on Theorems.proof"

THEOREMS_FILE="proofs/01-theorems/Theorems.proof"

# Per-category expected counts (124 active + 5 removal = 129 total)
declare -A EXPECTED_COUNT=(
  [L]=10
  [CN]=8
  [TF]=24
  [DP]=10
  [IC]=9
  [IA]=9
  [PL]=13
  [RL]=38
  [VF]=8
)

inventory_ok=1
for cat in L CN TF DP IC IA PL RL VF; do
  actual=$(grep -c "^Theorem ${cat}-" "$THEOREMS_FILE" || echo 0)
  expected=${EXPECTED_COUNT[$cat]}
  if [ "$actual" -eq "$expected" ]; then
    echo " OK: $cat = $actual (expected $expected)"
  else
    echo " FAIL: $cat = $actual (expected $expected)"
    inventory_ok=0
  fi
done

# Sub-rule presence check (must all be 1)
SUB_RULES=("PL-1a" "PL-4a" "IC-8a" "TF-2a" "TF-5a" "TF-6a" "TF-6b" "TF-6c" \
           "TF-9a" "TF-10a" "TF-15a" "RL-11a" "RL-14a" "RL-15a" "RL-18a")
for sr in "${SUB_RULES[@]}"; do
  n=$(grep -c "^Theorem ${sr}:" "$THEOREMS_FILE" || echo 0)
  if [ "$n" -ne 1 ]; then
    echo " FAIL: sub-rule $sr count = $n (expected 1)"
    inventory_ok=0
  fi
done

# Removal entries presence check (5 expected, encoded as Theorem REMOVED blocks)
for r in CN-4 CN-7 DP-10 IC-8 RL-13; do
  if ! grep -q "^Theorem ${r}: REMOVED" "$THEOREMS_FILE"; then
    echo " FAIL: removal entry $r missing"
    inventory_ok=0
  fi
done

if [ $inventory_ok -ne 1 ]; then
  echo "{\"status\": \"fail\", \"exit_reason\": \"theorem_inventory_incomplete\", \"reason\": \"Theorems.proof per-category or sub-rule or removal-entry inventory mismatch; see stdout\"}" > test-results/section-01-theorem_inventory_incomplete.log
  exit 2
fi

# ---------------------------------------------------------------------------
# Phase (c) — composition-inventory verification on Composition.proof
# ---------------------------------------------------------------------------
echo "Phase (c) — composition-inventory verification on Composition.proof"

COMPOSITION_FILE="proofs/01-theorems/Composition.proof"
ch_count=$(grep -c "^Theorem CH-" "$COMPOSITION_FILE" || echo 0)

if [ "$ch_count" -lt 5 ]; then
  echo " FAIL: CH composition count = $ch_count (expected >= 5)"
  echo "{\"status\": \"fail\", \"exit_reason\": \"theorem_inventory_incomplete\", \"reason\": \"Composition.proof CH count = $ch_count; expected >= 5 (CH-1 through CH-5 mandatory)\"}" > test-results/section-01-theorem_inventory_incomplete.log
  exit 2
fi

# Sub-rule presence: CH-1 through CH-5 mandatory
for ch in CH-1 CH-2 CH-3 CH-4 CH-5; do
  if ! grep -q "^Theorem ${ch}:" "$COMPOSITION_FILE"; then
    echo " FAIL: composition theorem $ch missing"
    echo "{\"status\": \"fail\", \"exit_reason\": \"theorem_inventory_incomplete\", \"reason\": \"Composition.proof missing $ch (CH-1..CH-5 mandatory)\"}" > test-results/section-01-theorem_inventory_incomplete.log
    exit 2
  fi
done
echo " OK: CH = $ch_count (expected >= 5; CH-1..CH-5 present)"

# ---------------------------------------------------------------------------
# Green path — sec-01 PASS gate.
# ---------------------------------------------------------------------------
total_theorems=$(grep -c "^Theorem " "$THEOREMS_FILE")
total_composition=$(grep -c "^Theorem CH-" "$COMPOSITION_FILE")
echo ""
echo "sec-01 PASS — foundation_authored_clean"
echo " 14 artifacts parse + exit 0"
echo " Theorems.proof: $total_theorems total (124 active + 5 removal)"
echo " Composition.proof: $total_composition CH theorems"

cat > test-results/section-01-result.json <<EOF
{"status": "pass", "exit_reason": "foundation_authored_clean", "theorems_total": $total_theorems, "composition_total": $total_composition, "artifacts_verified": ${#ARTIFACTS[@]}}
EOF

exit 0
