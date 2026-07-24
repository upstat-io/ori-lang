#!/usr/bin/env bash
#
# run-section-05-proofs.sh — §05 decision-predicate proofs orchestrator gate.
#
# Per the decision-predicate proofs
# success_criteria: every DP-N + DP-10-REMOVED has a machine-checked proof
# artifact in proofs/05-decisions/<artifact>.proof.
#
# Two-phase orchestration:
#
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure routes through `decision_predicate_infrastructure_failed`
# (exit 3).
# (b) For each of the 10 proof artifacts: invoke
# `./target/release/aims-proof-checker check
# proofs/05-decisions/<artifact>.proof --json >
# test-results/section-05-<artifact>.json`. Every proof MUST
# discharge GREEN (status: valid). Fail or UnimplementedEngineShape
# routes through a fail-branch exit_reason (exit 2).
# (c) After 10 proofs discharge GREEN, invoke
# `check-section-05-compiler-conformance.sh` (the section-05 compiler-conformance cross-walk); divergence
# routes through `decision_predicate_compiler_spec_divergence`
# (exit 2).
#
# Exit codes + exit_reasons (per this checker's documented exit-code
# contract — the 6 semantic names below plus proof_search_cap_reached):
# exit 0 + decision_predicates_proven — 10/10 green + cross-walk pass
# exit 2 + decision_predicate_proof_gap_unprovable — default fail
# exit 2 + decision_predicate_rule_reformulation_required — DP-3 case 3
# exit 2 + decision_predicate_rule_evolution_surfaced — DP-3 case 2/4
# exit 2 + decision_predicate_compiler_spec_divergence — DP-N ↔ impl
# exit 2 + proof_search_cap_reached — DP-3 Cure 3 cap
# exit 3 + decision_predicate_infrastructure_failed — pre-test infra
#
# DECISION_PREDICATE_FAILURE_CLASSIFICATION env-var (optional, on
# fail-paths): one of
# {decision_predicate_proof_gap_unprovable,
# decision_predicate_rule_reformulation_required,
# decision_predicate_rule_evolution_surfaced,
# decision_predicate_compiler_spec_divergence,
# proof_search_cap_reached}.
# Unset / empty → defaults to decision_predicate_proof_gap_unprovable.
# Invalid values route through decision_predicate_infrastructure_failed
# (exit 3).
#
# Proof artifact roster (10 total per the section-05 composition):
# 9 active DPs: DP-1, DP-2, DP-3, DP-4, DP-5, DP-6, DP-7, DP-8, DP-9
# 1 confirmation: DP-10-REMOVED
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "decision_predicate_proof_gap_unprovable"
    "decision_predicate_rule_reformulation_required"
    "decision_predicate_rule_evolution_surfaced"
    "decision_predicate_compiler_spec_divergence"
    "proof_search_cap_reached"
)

readonly PROOF_ARTIFACTS=(
    "DP-1"
    "DP-2"
    "DP-3"
    "DP-4"
    "DP-5"
    "DP-6"
    "DP-7"
    "DP-8"
    "DP-9"
    "DP-10-REMOVED"
)

fail_classification="${DECISION_PREDICATE_FAILURE_CLASSIFICATION:-decision_predicate_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_infrastructure_failed", "phase": "preflight", "reason": "DECISION_PREDICATE_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-05-build.log 2>&1; then
    cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-05-build.log"}
EOF
    exit 3
fi

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for artifact in "${PROOF_ARTIFACTS[@]}"; do
    proof_path="proofs/05-decisions/${artifact}.proof"
    out_path="test-results/section-05-${artifact}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_infrastructure_failed", "phase": "discharge", "failing_proof": "${artifact}", "reason": "proof file missing: ${proof_path}"}
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
    cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

# Phase (c) — compiler-conformance cross-walk (the section-05 compiler-conformance cross-walk).
conformance_script="scripts/check-section-05-compiler-conformance.sh"
if [[ ! -x "$conformance_script" ]]; then
    cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_infrastructure_failed", "phase": "conformance", "reason": "conformance script missing or not executable: ${conformance_script}"}
EOF
    exit 3
fi

if ! bash "$conformance_script" > test-results/section-05-conformance.log 2>&1; then
    conformance_exit=$?
    if [[ "$conformance_exit" -eq 2 ]]; then
        cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_compiler_spec_divergence", "proofs_passed": ${proofs_passed}, "reason": "compiler conformance cross-walk surfaced divergence; see test-results/section-05-conformance.log"}
EOF
        exit 2
    fi
    cat > test-results/section-05-result.json <<EOF
{"status": "fail", "exit_reason": "decision_predicate_infrastructure_failed", "phase": "conformance", "reason": "conformance script exited ${conformance_exit}; see test-results/section-05-conformance.log"}
EOF
    exit 3
fi

cat > test-results/section-05-result.json <<EOF
{"status": "pass", "exit_reason": "decision_predicates_proven", "proofs_passed": ${proofs_passed}}
EOF
exit 0
