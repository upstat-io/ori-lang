#!/usr/bin/env bash
#
# run-section-04-proofs.sh — §04 transfer-function proofs orchestrator gate.
#
# Per Annex E §AIMS §4
# success_criteria: every TF-N + composition + IA-5 step (1) has a
# machine-checked proof artifact in proofs/04-transfers/<artifact>.proof.
#
# Two-phase orchestration:
#
# (a) cargo build --release -p aims-proof-checker — pre-run build.
# Failure routes through `transfer_infrastructure_failed` (exit 3).
# (b) For each of the 27 proof artifacts: invoke
# `./target/release/aims-proof-checker check
# proofs/04-transfers/<artifact>.proof --json >
# test-results/section-04-<artifact>.json`. Every proof MUST
# discharge GREEN (status: valid). Fail or UnimplementedEngineShape
# routes through a fail-branch exit_reason (exit 2).
# (c) After 27 proofs discharge GREEN, invoke
# `check-section-04-compiler-conformance.sh` (§04-IM-G); divergence
# routes through `transfer_compiler_spec_divergence` (exit 2).
#
# Exit codes + exit_reasons (per this checker's documented exit-code
# contract — the 6 semantic names below plus proof_search_cap_reached):
# exit 0 + transfer_proofs_passed — 27/27 green + cross-walk pass
# exit 2 + transfer_proof_gap_unprovable — default fail
# exit 2 + transfer_rule_reformulation_required — DP-3 case 3
# exit 2 + transfer_rule_evolution_surfaced — DP-3 case 2/4
# exit 2 + transfer_compiler_spec_divergence — TF-N ↔ impl
# exit 2 + proof_search_cap_reached — DP-3 Cure 3 cap
# exit 3 + transfer_infrastructure_failed — pre-test infra
#
# TRANSFER_FAILURE_CLASSIFICATION env-var (optional, on fail-paths):
# one of {transfer_proof_gap_unprovable,
# transfer_rule_reformulation_required,
# transfer_rule_evolution_surfaced,
# transfer_compiler_spec_divergence, proof_search_cap_reached}.
# Unset / empty → defaults to transfer_proof_gap_unprovable.
# Invalid values route through transfer_infrastructure_failed (exit 3).
#
# Proof artifact roster (27 total per §04-IM-E composition):
# 19 forward TF: TF-1, TF-2, TF-2a, TF-3, TF-4, TF-5, TF-5a, TF-6,
# TF-6a, TF-6b, TF-6c, TF-7, TF-8, TF-9, TF-9a, TF-10,
# TF-10a, TF-15, TF-15a
# 5 backward TF: TF-11, TF-11a, TF-12, TF-13, TF-14
# 1 confirmation: TF-N-A
# 1 composition: Composition
# 1 intraprocedural alias-transfer: IA-5-step-1
#
# Cwd contract: anchors to aims-proof/ via cd "$(dirname "$0")/.." at
# entry.

set -e
cd "$(dirname "$0")/.."

mkdir -p test-results

readonly VALID_FAIL_CLASSIFICATIONS=(
    "transfer_proof_gap_unprovable"
    "transfer_rule_reformulation_required"
    "transfer_rule_evolution_surfaced"
    "transfer_compiler_spec_divergence"
    "proof_search_cap_reached"
)

readonly PROOF_ARTIFACTS=(
    "TF-1"
    "TF-2"
    "TF-2a"
    "TF-3"
    "TF-4"
    "TF-5"
    "TF-5a"
    "TF-6"
    "TF-6a"
    "TF-6b"
    "TF-6c"
    "TF-7"
    "TF-8"
    "TF-9"
    "TF-9a"
    "TF-10"
    "TF-10a"
    "TF-15"
    "TF-15a"
    "TF-11"
    "TF-11a"
    "TF-12"
    "TF-13"
    "TF-14"
    "TF-N-A"
    "Composition"
    "IA-5-step-1"
)

fail_classification="${TRANSFER_FAILURE_CLASSIFICATION:-transfer_proof_gap_unprovable}"
classification_valid=0
for valid in "${VALID_FAIL_CLASSIFICATIONS[@]}"; do
    if [[ "$fail_classification" == "$valid" ]]; then
        classification_valid=1
        break
    fi
done
if [[ "$classification_valid" -ne 1 ]]; then
    cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_infrastructure_failed", "phase": "preflight", "reason": "TRANSFER_FAILURE_CLASSIFICATION='${fail_classification}' not in {${VALID_FAIL_CLASSIFICATIONS[*]}}"}
EOF
    exit 3
fi

# Phase (a) — build.
if ! cargo build --release -p aims-proof-checker > test-results/section-04-build.log 2>&1; then
    cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_infrastructure_failed", "phase": "build", "reason": "cargo build --release -p aims-proof-checker failed; see test-results/section-04-build.log"}
EOF
    exit 3
fi

# Phase (b) — per-proof discharge.
proofs_passed=0
proofs_failed=()

for artifact in "${PROOF_ARTIFACTS[@]}"; do
    proof_path="proofs/04-transfers/${artifact}.proof"
    out_path="test-results/section-04-${artifact}.json"

    if [[ ! -f "$proof_path" ]]; then
        cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_infrastructure_failed", "phase": "discharge", "failing_proof": "${artifact}", "reason": "proof file missing: ${proof_path}"}
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
    cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "${fail_classification}", "proofs_passed": ${proofs_passed}, "proofs_failed": "${failing_list}"}
EOF
    exit 2
fi

# Phase (c) — compiler-conformance cross-walk (§04-IM-G).
conformance_script="scripts/check-section-04-compiler-conformance.sh"
if [[ ! -x "$conformance_script" ]]; then
    cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_infrastructure_failed", "phase": "conformance", "reason": "conformance script missing or not executable: ${conformance_script}"}
EOF
    exit 3
fi

if ! bash "$conformance_script" > test-results/section-04-conformance.log 2>&1; then
    conformance_exit=$?
    if [[ "$conformance_exit" -eq 2 ]]; then
        cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_compiler_spec_divergence", "proofs_passed": ${proofs_passed}, "reason": "compiler conformance cross-walk surfaced divergence; see test-results/section-04-conformance.log"}
EOF
        exit 2
    fi
    cat > test-results/section-04-result.json <<EOF
{"status": "fail", "exit_reason": "transfer_infrastructure_failed", "phase": "conformance", "reason": "conformance script exited ${conformance_exit}; see test-results/section-04-conformance.log"}
EOF
    exit 3
fi

cat > test-results/section-04-result.json <<EOF
{"status": "pass", "exit_reason": "transfer_proofs_passed", "proofs_passed": ${proofs_passed}}
EOF
exit 0
