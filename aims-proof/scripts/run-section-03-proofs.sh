#!/usr/bin/env bash
#
# run-section-03-proofs.sh — §03 canonicalization-rule proofs orchestrator gate.
#
# Per Annex E §AIMS §5
# success_criteria row 1: "Each of CN-1, CN-2, CN-3, CN-5, CN-6, CN-8 has a
# machine-checked proof artifact ... CN-4-REMOVED and CN-7-REMOVED carry
# confirmation-of-removal artifacts ...".
#
# Two-phase orchestration:
#
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure routes through `canonicalization_infrastructure_failed`
# (exit 3).
# (b) For each of the 11 proof artifacts: invoke
# `./target/release/aims-proof-checker check
# proofs/03-canonicalization/<artifact>.proof --json >
# test-results/section-03-<artifact>.json`. Every proof MUST
# discharge GREEN (status: valid). Fail or UnimplementedEngineShape
# routes through a fail-branch exit_reason (exit 2).
#
# Exit codes + exit_reasons (per this checker's documented exit-code
# contract — the 6 semantic names below plus proof_search_cap_reached):
# exit 0 + canonicalization_rules_proven — 11/11 green
# exit 2 + canonicalization_proof_gap_unprovable — default fail
# exit 2 + canonicalization_rule_reformulation_required — DP-3 case 3
# exit 2 + canonicalization_rule_evolution_surfaced — DP-3 case 2/4
# exit 2 + canonicalization_compiler_spec_divergence — CN-N ↔ impl
# exit 2 + proof_search_cap_reached — DP-3 Cure 3 cap
# exit 3 + canonicalization_infrastructure_failed — pre-test infra
#
# CANONICALIZATION_FAILURE_CLASSIFICATION env-var (optional, on fail-paths):
# one of {canonicalization_proof_gap_unprovable,
# canonicalization_rule_reformulation_required,
# canonicalization_rule_evolution_surfaced,
# canonicalization_compiler_spec_divergence, proof_search_cap_reached}.
# Unset / empty → defaults to canonicalization_proof_gap_unprovable.
# Invalid values route through canonicalization_infrastructure_failed
# (exit 3).
#
# Proof artifact roster (11 total per the section-03 composition):
# 6 active CN proofs: CN-1, CN-2, CN-3, CN-5, CN-6, CN-8
# 2 confirmation proofs: CN-4-REMOVED, CN-7-REMOVED
# 1 reconciliation proof: CN-6-Extraction-Reconciliation
# 1 ordering proof: CN-Ordering
# 1 fixpoint proof: Fixpoint
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "canonicalization_proof_gap_unprovable"
    "canonicalization_rule_reformulation_required"
    "canonicalization_rule_evolution_surfaced"
    "canonicalization_compiler_spec_divergence"
    "proof_search_cap_reached"
)

readonly PROOF_ARTIFACTS=(
    "CN-1"
    "CN-2"
    "CN-3"
    "CN-4-REMOVED"
    "CN-5"
    "CN-6"
    "CN-6-Extraction-Reconciliation"
    "CN-7-REMOVED"
    "CN-8"
    "CN-Ordering"
    "Fixpoint"
)

fail_classification="${CANONICALIZATION_FAILURE_CLASSIFICATION:-canonicalization_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-03-result.json <<EOF
{"status": "fail", "exit_reason": "canonicalization_infrastructure_failed", "phase": "preflight", "reason": "CANONICALIZATION_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-03-build.log 2>&1; then
    cat > test-results/section-03-result.json <<EOF
{"status": "fail", "exit_reason": "canonicalization_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-03-build.log"}
EOF
    exit 3
fi

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for artifact in "${PROOF_ARTIFACTS[@]}"; do
    proof_path="proofs/03-canonicalization/${artifact}.proof"
    out_path="test-results/section-03-${artifact}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-03-result.json <<EOF
{"status": "fail", "exit_reason": "canonicalization_infrastructure_failed", "phase": "discharge", "failing_proof": "${artifact}", "reason": "proof file missing: ${proof_path}"}
EOF
        exit 3
    fi

    ./target/release/aims-proof-checker check "$proof_path" --json > "$out_path" 2>&1 || true
    proof_status=$(python3 -c "import sys,json; print(json.loads(open('${out_path}').read()).get('status','PARSE_ERR'))" 2>/dev/null || echo "PARSE_ERR")

    if [[ "$proof_status" == "valid" ]]; then
        proofs_passed=$((proofs_passed + 1))
    else
        proofs_failed+=("${artifact}:${proof_status}")
    fi
done

if [[ ${#proofs_failed[@]} -gt 0 ]]; then
    failing_list=$(IFS=,; echo "${proofs_failed[*]}")
    cat > test-results/section-03-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

cat > test-results/section-03-result.json <<EOF
{"status": "pass", "exit_reason": "canonicalization_rules_proven", "proofs_passed": ${proofs_passed}}
EOF
exit 0
